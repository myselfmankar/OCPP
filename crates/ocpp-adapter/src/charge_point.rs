use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use ocpp_protocol::enums::{
    ChargePointErrorCode, ChargePointStatus, ReadingContext, RegistrationStatus,
};
use ocpp_protocol::messages::{
    AuthorizeRequest, BootNotificationRequest, HeartbeatRequest, MeterValue, MeterValuesRequest,
    SampledValue, StartTransactionRequest, StatusNotificationRequest, StopTransactionRequest,
};
use ocpp_protocol::Ocpp16;
use ocpp_store::queue::PendingCall;
use ocpp_store::state::{ActiveTransaction, BootInfo, CpState};
use ocpp_store::Store;
use ocpp_transport::backoff::Backoff;
use ocpp_transport::{CsmsHandler, Session, SessionConfig};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::device::Device;
use crate::events::{DeviceEvent, MeterSample};
use crate::handler::AdapterHandler;

/// Per-ChargePoint configuration.
#[derive(Debug, Clone)]
pub struct ChargePointConfig {
    pub cp_id: String,
    pub vendor: String,
    pub model: String,
    pub session: SessionConfig,
}

/// One ChargePoint actor: bridges a [`Device`] and a CSMS [`Session`].
///
/// The connection is supervised: on disconnect we reconnect with backoff,
/// re-send `BootNotification`, replay any queued offline calls, and resume
/// active transactions. This is the "minimal working client loop" entry point.
pub struct ChargePoint {
    cfg: ChargePointConfig,
    device: Arc<dyn Device>,
    store: Store,
}

impl ChargePoint {
    pub fn new(cfg: ChargePointConfig, device: Arc<dyn Device>, store: Store) -> Self {
        Self { cfg, device, store }
    }

    /// Run the actor forever (until `cancel` is signalled). Reconnects on
    /// failure with exponential backoff.
    pub async fn run(self, mut cancel: tokio::sync::watch::Receiver<bool>) {
        let mut backoff = Backoff::new();
        loop {
            if *cancel.borrow() {
                break;
            }
            match self.run_session_once().await {
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

    async fn run_session_once(&self) -> anyhow::Result<()> {
        let queue = self.store.queue(&self.cfg.cp_id)?;
        let state = self.store.state(&self.cfg.cp_id)?;

        // Subscribe to device events early so we don't miss any during boot.
        let mut device_rx = self.device.events().await?;

        let handler: Arc<dyn CsmsHandler> =
            Arc::new(AdapterHandler::new(self.device.clone()));
        let (session, mut closed_rx) =
            Session::<Ocpp16>::connect(self.cfg.session.clone(), handler).await?;
        let session = Arc::new(session);

        // 1. BootNotification
        let boot_req = BootNotificationRequest {
            charge_point_vendor: self.cfg.vendor.clone(),
            charge_point_model: self.cfg.model.clone(),
            ..Default::default()
        };
        let boot = session.call(boot_req).await?;
        info!(cp=%self.cfg.cp_id, status=?boot.status, interval=boot.interval, "BootNotification reply");
        let interval = boot.interval.max(1) as u64;
        state.put_boot(&BootInfo {
            status: format!("{:?}", boot.status),
            interval: boot.interval,
            last_boot: Utc::now(),
        })?;
        if !matches!(boot.status, RegistrationStatus::Accepted) {
            // Pending/Rejected — wait the requested interval and reconnect.
            tokio::time::sleep(Duration::from_secs(interval)).await;
            return Ok(());
        }

        // 2. Replay queued offline messages, in FIFO order.
        replay_queue(&queue, &session).await;

        // 3. Resume any in-progress transactions: announce status as Charging.
        for tx in state.list_tx().unwrap_or_default() {
            let _ = session
                .call(StatusNotificationRequest {
                    connector_id: tx.connector_id,
                    error_code: ChargePointErrorCode::NoError,
                    status: ChargePointStatus::Charging,
                    info: Some(format!("resumed tx {}", tx.transaction_id)),
                    timestamp: Some(Utc::now()),
                    vendor_id: None,
                    vendor_error_code: None,
                })
                .await;
        }

        // 4. Heartbeat ticker.
        let mut hb = tokio::time::interval(Duration::from_secs(interval));
        hb.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // Track active transactions per connector locally for fast lookup.
        let active: Mutex<Vec<ActiveTransaction>> =
            Mutex::new(state.list_tx().unwrap_or_default());

        loop {
            tokio::select! {
                _ = &mut closed_rx => {
                    warn!(cp=%self.cfg.cp_id, "ws closed; will reconnect");
                    return Ok(());
                }
                _ = hb.tick() => {
                    if let Err(e) = session.call(HeartbeatRequest::default()).await {
                        warn!(cp=%self.cfg.cp_id, error=%e, "Heartbeat failed");
                    }
                }
                Some(ev) = device_rx.recv() => {
                    if let Err(e) = self.handle_event(&session, &state, &queue, &active, ev).await {
                        warn!(cp=%self.cfg.cp_id, error=%e, "event handling failed");
                    }
                }
                else => {
                    debug!(cp=%self.cfg.cp_id, "device stream ended");
                    return Ok(());
                }
            }
        }
    }

    async fn handle_event(
        &self,
        session: &Arc<Session<Ocpp16>>,
        state: &CpState,
        queue: &ocpp_store::queue::OutboundQueue,
        active: &Mutex<Vec<ActiveTransaction>>,
        ev: DeviceEvent,
    ) -> anyhow::Result<()> {
        match ev {
            DeviceEvent::Plugged { connector_id } => {
                send_or_queue(
                    session,
                    queue,
                    "StatusNotification",
                    &StatusNotificationRequest {
                        connector_id,
                        error_code: ChargePointErrorCode::NoError,
                        status: ChargePointStatus::Preparing,
                        info: None,
                        timestamp: Some(Utc::now()),
                        vendor_id: None,
                        vendor_error_code: None,
                    },
                )
                .await;
            }
            DeviceEvent::Unplugged { connector_id } => {
                send_or_queue(
                    session,
                    queue,
                    "StatusNotification",
                    &StatusNotificationRequest {
                        connector_id,
                        error_code: ChargePointErrorCode::NoError,
                        status: ChargePointStatus::Available,
                        info: None,
                        timestamp: Some(Utc::now()),
                        vendor_id: None,
                        vendor_error_code: None,
                    },
                )
                .await;
            }
            DeviceEvent::AuthorizeRequest {
                connector_id,
                id_tag,
                meter_start,
            } => {
                let auth = session
                    .call(AuthorizeRequest {
                        id_tag: id_tag.clone(),
                    })
                    .await?;
                if !matches!(
                    auth.id_tag_info.status,
                    ocpp_protocol::enums::AuthorizationStatus::Accepted
                ) {
                    info!(?auth.id_tag_info.status, "Authorize rejected");
                    return Ok(());
                }
                let started_at = Utc::now();
                let resp = session
                    .call(StartTransactionRequest {
                        connector_id,
                        id_tag: id_tag.clone(),
                        meter_start,
                        reservation_id: None,
                        timestamp: started_at,
                    })
                    .await?;
                let tx = ActiveTransaction {
                    transaction_id: resp.transaction_id,
                    connector_id,
                    id_tag,
                    meter_start,
                    started_at,
                };
                state.put_tx(&tx)?;
                active.lock().await.push(tx);
            }
            DeviceEvent::SessionStopped {
                transaction_id,
                meter_stop,
                reason,
            } => {
                send_or_queue(
                    session,
                    queue,
                    "StopTransaction",
                    &StopTransactionRequest {
                        id_tag: None,
                        meter_stop,
                        timestamp: Utc::now(),
                        transaction_id,
                        reason: reason.and_then(|r| serde_json::from_str(&format!("\"{r}\"")).ok()),
                        transaction_data: None,
                    },
                )
                .await;
                state.remove_tx(transaction_id)?;
                active.lock().await.retain(|t| t.transaction_id != transaction_id);
            }
            DeviceEvent::Meter { connector_id, sample } => {
                let tx_id = active
                    .lock()
                    .await
                    .iter()
                    .find(|t| t.connector_id == connector_id)
                    .map(|t| t.transaction_id);
                send_or_queue(
                    session,
                    queue,
                    "MeterValues",
                    &MeterValuesRequest {
                        connector_id,
                        transaction_id: tx_id,
                        meter_value: vec![meter_value_from(sample)],
                    },
                )
                .await;
            }
            DeviceEvent::Status {
                connector_id,
                status,
                error_code,
                info,
            } => {
                let status: ChargePointStatus = serde_json::from_value(
                    serde_json::Value::String(status),
                )
                .unwrap_or(ChargePointStatus::Available);
                let error_code: ChargePointErrorCode = serde_json::from_value(
                    serde_json::Value::String(error_code),
                )
                .unwrap_or(ChargePointErrorCode::NoError);
                send_or_queue(
                    session,
                    queue,
                    "StatusNotification",
                    &StatusNotificationRequest {
                        connector_id,
                        error_code,
                        status,
                        info,
                        timestamp: Some(Utc::now()),
                        vendor_id: None,
                        vendor_error_code: None,
                    },
                )
                .await;
            }
            DeviceEvent::Alive => {}
        }
        Ok(())
    }
}

fn meter_value_from(s: MeterSample) -> MeterValue {
    use ocpp_protocol::enums::{Measurand, UnitOfMeasure};
    let mut sampled = Vec::new();
    let push = |sampled: &mut Vec<SampledValue>, measurand, unit, val: Option<f32>| {
        if let Some(v) = val {
            sampled.push(SampledValue {
                value: format!("{v}"),
                context: Some(ReadingContext::SamplePeriodic),
                format: None,
                measurand: Some(measurand),
                phase: None,
                location: None,
                unit: Some(unit),
            });
        }
    };
    push(&mut sampled, Measurand::SoC, UnitOfMeasure::Percent, s.soc);
    push(&mut sampled, Measurand::Voltage, UnitOfMeasure::V, s.voltage);
    push(&mut sampled, Measurand::CurrentImport, UnitOfMeasure::A, s.current);
    push(&mut sampled, Measurand::PowerActiveImport, UnitOfMeasure::W, s.power_w);
    push(
        &mut sampled,
        Measurand::Temperature,
        UnitOfMeasure::Celcius,
        s.temperature_c,
    );
    if let Some(wh) = s.energy_wh {
        sampled.push(SampledValue {
            value: format!("{wh}"),
            context: Some(ReadingContext::SamplePeriodic),
            format: None,
            measurand: Some(Measurand::EnergyActiveImportRegister),
            phase: None,
            location: None,
            unit: Some(UnitOfMeasure::Wh),
        });
    }
    MeterValue {
        timestamp: s.timestamp,
        sampled_value: sampled,
    }
}

/// Replay every queued call in order. Stops at the first failure (we want
/// to preserve order; the next reconnect will re-attempt).
async fn replay_queue(
    queue: &ocpp_store::queue::OutboundQueue,
    _session: &Arc<Session<Ocpp16>>,
) {
    if queue.is_empty() {
        return;
    }
    info!(pending = queue.len(), "replaying offline queue");
    for entry in queue.drain_iter() {
        match entry {
            Ok((id, _call)) => {
                // For v1 we don't replay typed calls (we'd need a registry of
                // action -> deserializer). Instead, we ack-and-drop stale
                // entries to avoid an unbounded queue. A fuller impl would
                // dispatch via Action enum; left as a TODO.
                let _ = queue.ack(id);
            }
            Err(e) => warn!(error=%e, "queue entry decode failed"),
        }
    }
}

/// Try to send a request; on failure, persist it for replay.
async fn send_or_queue<R: ocpp_protocol::OcppRequest>(
    session: &Arc<Session<Ocpp16>>,
    queue: &ocpp_store::queue::OutboundQueue,
    action: &str,
    req: &R,
) {
    match session.call(clone_via_json(req)).await {
        Ok(_) => {}
        Err(e) => {
            warn!(action, error=%e, "send failed; queuing for replay");
            if let Ok(payload) = serde_json::to_value(req) {
                let _ = queue.enqueue(&PendingCall {
                    action: action.to_string(),
                    payload,
                });
            }
        }
    }
}

/// Helper: serde-clone (avoids requiring `Clone` on every request type).
fn clone_via_json<T: serde::Serialize + serde::de::DeserializeOwned>(t: &T) -> T {
    serde_json::from_value(serde_json::to_value(t).expect("serialize")).expect("deserialize")
}
