use std::sync::Arc;
use std::time::Duration;

use ocpp_protocol::Ocpp16;
use ocpp_store::Store;
use ocpp_transport::backoff::Backoff;
use ocpp_transport::{CsmsHandler, Session, SessionConfig};
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use crate::connectivity::ConnectivityManager;
use crate::handler::AdapterHandler;
use crate::metadata::MetadataManager;
use crate::transaction::TransactionManager;
use crate::Device;

/// Per-ChargePoint configuration.
#[derive(Debug, Clone)]
pub struct ChargePointConfig {
    pub cp_id: String,
    pub vendor: String,
    pub model: String,
    pub session: SessionConfig,
}

/// One ChargePoint actor: bridges a [`Device`] and a CSMS [`Session`].
pub struct ChargePoint {
    cfg: ChargePointConfig,
    device: Arc<dyn Device>,
    store: Store,
}

impl ChargePoint {
    pub fn new(cfg: ChargePointConfig, device: Arc<dyn Device>, store: Store) -> Self {
        Self { cfg, device, store }
    }

    pub async fn run(self, mut cancel: tokio::sync::watch::Receiver<bool>) {
        let mut backoff = Backoff::new();

        let connectivity = Arc::new(ConnectivityManager::new(
            self.cfg.cp_id.clone(),
            self.cfg.vendor.clone(),
            self.cfg.model.clone(),
            self.cfg.session.clone(),
        ));

        let config_store = self.store.config(&self.cfg.cp_id).expect("config store");
        let auth_store = self.store.auth(&self.cfg.cp_id).expect("auth store");
        let metadata = Arc::new(MetadataManager::new(
            self.cfg.cp_id.clone(),
            config_store,
            auth_store,
        ));

        let state = self.store.state(&self.cfg.cp_id).expect("state store");
        let profile_store = self.store.profiles(&self.cfg.cp_id).expect("profile store");
        let reservation_store = self.store.reservations(&self.cfg.cp_id).expect("res store");
        let auth_store_tx = self.store.auth(&self.cfg.cp_id).expect("auth store");
        let config_store_tx = self.store.config(&self.cfg.cp_id).expect("config store");

        let active_list = state.list_tx().unwrap_or_default();
        let transactions = Arc::new(TransactionManager::new(
            self.cfg.cp_id.clone(),
            state,
            auth_store_tx,
            config_store_tx,
            profile_store,
            reservation_store,
            active_list,
        ));

        loop {
            if *cancel.borrow() {
                break;
            }

            match self
                .run_session_once(&connectivity, &metadata, &transactions)
                .await
            {
                Ok(()) => {
                    info!(cp = %self.cfg.cp_id, "session ended cleanly; reconnecting");
                    backoff.reset();
                }
                Err(e) => {
                    let delay = backoff.next_delay();
                    error!(cp = %self.cfg.cp_id, error = %e, ?delay, "session failed; backing off");
                    tokio::select! {
                        _ = tokio::time::sleep(delay) => {},
                        _ = cancel.changed() => break,
                    }
                }
            }
        }
        info!(cp = %self.cfg.cp_id, "ChargePoint actor stopping");
    }

    async fn run_session_once(
        &self,
        connectivity: &Arc<ConnectivityManager>,
        metadata: &Arc<MetadataManager>,
        transactions: &Arc<TransactionManager>,
    ) -> anyhow::Result<()> {
        let queue = self.store.queue(&self.cfg.cp_id)?;
        let state = self.store.state(&self.cfg.cp_id)?;

        let mut device_rx = self.device.events().await?;
        let (trigger_tx, mut trigger_rx) = mpsc::channel(8);

        let handler: Arc<dyn CsmsHandler> = Arc::new(AdapterHandler::new(
            self.device.clone(),
            trigger_tx,
            metadata.clone(),
            transactions.clone(),
        ));

        let (session, mut closed_rx) =
            Session::<Ocpp16>::connect(self.cfg.session.clone(), handler).await?;
        let session = Arc::new(session);

        // 1. Boot
        let interval = connectivity.boot(&session, &queue, &state).await?;

        // 2. Replay & Retry
        connectivity.replay_queue(&queue, &session).await;

        transactions
            .retry_pending_stops(|tx_id, ps| {
                let session = session.clone();
                let req = ocpp_protocol::messages::StopTransactionRequest {
                    id_tag: None,
                    meter_stop: ps.meter_stop,
                    timestamp: ps.timestamp,
                    transaction_id: tx_id,
                    reason: ps.reason,
                    transaction_data: None,
                };
                async move {
                    session
                        .call(req)
                        .await
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                }
            })
            .await;

        // 3. Resume
        transactions.resume_transactions(&session, &queue).await;

        // 4. Main loop
        let mut heartbeat_tick = tokio::time::interval(Duration::from_secs(interval));
        heartbeat_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = &mut closed_rx => {
                    warn!(cp = %self.cfg.cp_id, "CSMS connection closed");
                    break;
                }
                _ = heartbeat_tick.tick() => {
                    metadata.heartbeat(&session).await;
                }
                Some(event) = device_rx.recv() => {
                    // Transaction manager handles events too (e.g. Stop on Unplugged)
                    if let Err(e) = transactions.handle_event(&session, &queue, event.clone()).await {
                        warn!(cp = %self.cfg.cp_id, error = %e, "Transaction event error");
                    }
                    if let Err(e) = metadata.handle_event(&session, &queue, event).await {
                        warn!(cp = %self.cfg.cp_id, error = %e, "Metadata event error");
                    }
                }
                Some(trigger) = trigger_rx.recv() => {
                    match trigger {
                        ocpp_protocol::enums::MessageTrigger::BootNotification => {
                            let _ = connectivity.boot(&session, &queue, &state).await;
                        }
                        ocpp_protocol::enums::MessageTrigger::Heartbeat => {
                            metadata.heartbeat(&session).await;
                        }
                        ocpp_protocol::enums::MessageTrigger::FirmwareStatusNotification => {
                            metadata.report_firmware_status(&session, &queue).await;
                        }
                        ocpp_protocol::enums::MessageTrigger::DiagnosticsStatusNotification => {
                            metadata.report_diagnostics_status(&session, &queue).await;
                        }
                        _ => warn!(?trigger, "Trigger not supported"),
                    }
                }
            }
        }

        Ok(())
    }
}
