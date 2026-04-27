use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use ocpp_protocol::enums::{
    ChargePointErrorCode, ChargePointStatus, MessageTrigger, ReadingContext, RegistrationStatus,
    StopReason,
};
use ocpp_protocol::messages::{
    AuthorizeRequest, BootNotificationRequest, DiagnosticsStatusNotificationRequest,
    FirmwareStatusNotificationRequest,
    HeartbeatRequest, MeterValue, MeterValuesRequest,
    SampledValue, StartTransactionRequest, StatusNotificationRequest, StopTransactionRequest,
};
use ocpp_protocol::Ocpp16;
use ocpp_store::queue::PendingCall;
use ocpp_store::state::{ActiveTransaction, BootInfo, CpState, PendingStop};
use ocpp_store::Store;
use ocpp_transport::backoff::Backoff;
use ocpp_transport::{CsmsHandler, Session, SessionConfig};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};

use crate::Device;
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

        let (trigger_tx, mut trigger_rx) = mpsc::channel(8);
        let handler: Arc<dyn CsmsHandler> =
            Arc::new(AdapterHandler::new(self.device.clone(), trigger_tx));
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

        // 2b. Retry any StopTransaction that was issued locally last session
        //     but never confirmed by the CSMS. Active txs whose stop is
        //     successfully delivered are dropped; the rest are kept (they
        //     will retry on the next reconnect).
        let active_list = state.list_tx().unwrap_or_default();
        let active_list = retry_pending_stops_with(&state, active_list, |tx_id, ps| {
            let session = session.clone();
            let req = StopTransactionRequest {
                id_tag: None,
                meter_stop: ps.meter_stop,
                timestamp: ps.timestamp,
                transaction_id: tx_id,
                reason: ps.reason.clone(),
                transaction_data: None,
            };
            async move {
                session.call(req).await.map(|_| ()).map_err(|e| e.to_string())
            }
        })
        .await;

        // 3. Resume any in-progress transactions: announce status as Charging.
        for tx in &active_list {
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
        let active: Mutex<Vec<ActiveTransaction>> = Mutex::new(active_list);

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
                Some(trigger) = trigger_rx.recv() => {
                    self.handle_trigger(&session, &queue, trigger).await;
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
                if !matches!(
                    resp.id_tag_info.status,
                    ocpp_protocol::enums::AuthorizationStatus::Accepted
                ) {
                    warn!(
                        cp=%self.cfg.cp_id,
                        ?resp.id_tag_info.status,
                        transaction_id=resp.transaction_id,
                        "StartTransaction rejected by CSMS; not tracking locally"
                    );
                    return Ok(());
                }
                let tx = ActiveTransaction {
                    transaction_id: resp.transaction_id,
                    connector_id,
                    id_tag,
                    meter_start,
                    started_at,
                    pending_stop: None,
                };
                state.put_tx(&tx)?;
                active.lock().await.push(tx);
            }
            DeviceEvent::SessionStopped {
                transaction_id,
                meter_stop,
                reason,
            } => {
                let timestamp = Utc::now();
                let req = StopTransactionRequest {
                    id_tag: None,
                    meter_stop,
                    timestamp,
                    transaction_id,
                    reason,
                    transaction_data: None,
                };
                let session_for_send = session.clone();
                stop_or_persist(
                    state,
                    active,
                    transaction_id,
                    meter_stop,
                    timestamp,
                    reason,
                    || async move {
                        session_for_send
                            .call(req)
                            .await
                            .map(|_| ())
                            .map_err(|e| e.to_string())
                    },
                )
                .await;
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
            DeviceEvent::FirmwareStatus { status } => {
                // Best-effort: if CSMS is offline the status notification is lost.
                // Per OCPP 1.6 §5.16 there is no queuing requirement for these.
                let _ = session
                    .call(FirmwareStatusNotificationRequest { status })
                    .await;
            }
            DeviceEvent::DiagnosticsStatus { status } => {
                // Best-effort: ditto — OCPP 1.6 §5.10.
                let _ = session
                    .call(DiagnosticsStatusNotificationRequest { status })
                    .await;
            }
        }
        Ok(())
    }

    async fn handle_trigger(
        &self,
        session: &Arc<Session<Ocpp16>>,
        _queue: &ocpp_store::queue::OutboundQueue,
        trigger: MessageTrigger,
    ) {
        info!(cp=%self.cfg.cp_id, ?trigger, "handling CSMS trigger");
        match trigger {
            MessageTrigger::BootNotification => {
                let _ = session
                    .call(BootNotificationRequest {
                        charge_point_vendor: self.cfg.vendor.clone(),
                        charge_point_model: self.cfg.model.clone(),
                        ..Default::default()
                    })
                    .await;
            }
            MessageTrigger::Heartbeat => {
                let _ = session.call(HeartbeatRequest::default()).await;
            }
            _ => warn!(?trigger, "trigger not yet fully implemented"),
        }
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
        UnitOfMeasure::Celsius,
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

/// Replay every queued call in order. Stops at the first failure to
/// preserve FIFO ordering — the next reconnect will resume from the
/// failed entry.
async fn replay_queue(
    queue: &ocpp_store::queue::OutboundQueue,
    session: &Arc<Session<Ocpp16>>,
) {
    replay_queue_with(queue, |action, payload| {
        let session = session.clone();
        let action = action.to_string();
        async move {
            session
                .call_raw(&action, payload)
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
    })
    .await
}

/// Inner replay loop, parameterised over the dispatch behaviour so it can
/// be unit-tested without a live WebSocket session.
///
/// Semantics:
/// - FIFO order (sled key ordering).
/// - Each entry is **only** acked after `dispatch` returns `Ok`.
/// - On the first failure the loop returns immediately, leaving that entry
///   and any later entries in the queue for the next reconnect.
async fn replay_queue_with<F, Fut>(
    queue: &ocpp_store::queue::OutboundQueue,
    mut dispatch: F,
) where
    F: FnMut(&str, serde_json::Value) -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    if queue.is_empty() {
        return;
    }
    info!(pending = queue.len(), "replaying offline queue");
    // Snapshot the current entries; new enqueues during replay will be picked
    // up on the next reconnect.
    let snapshot: Vec<_> = queue.drain_iter().filter_map(|r| r.ok()).collect();
    for (id, call) in snapshot {
        match dispatch(&call.action, call.payload.clone()).await {
            Ok(()) => {
                if let Err(e) = queue.ack(id) {
                    warn!(id, error=%e, action=%call.action, "queue ack failed; will resend");
                } else {
                    debug!(id, action=%call.action, "replayed");
                }
            }
            Err(e) => {
                warn!(
                    id,
                    action=%call.action,
                    error=%e,
                    "replay failed; halting replay to preserve order"
                );
                return;
            }
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

/// Try to send a `StopTransaction`. On success the active transaction is
/// removed from local state and from `active`. On failure a `pending_stop`
/// flag is recorded on the active transaction and persisted, so the next
/// reconnect can retry it durably.
///
/// The actual network call is supplied as a closure to keep the helper
/// unit-testable without a live `Session`.
async fn stop_or_persist<F, Fut>(
    state: &CpState,
    active: &Mutex<Vec<ActiveTransaction>>,
    transaction_id: i32,
    meter_stop: i32,
    timestamp: chrono::DateTime<Utc>,
    reason: Option<StopReason>,
    send: F,
) -> bool
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    match send().await {
        Ok(()) => {
            if let Err(e) = state.remove_tx(transaction_id) {
                warn!(transaction_id, error=%e, "failed to clear active tx after StopTransaction");
            }
            active.lock().await.retain(|t| t.transaction_id != transaction_id);
            true
        }
        Err(e) => {
            warn!(
                transaction_id,
                error = %e,
                "StopTransaction send failed; persisting pending_stop for retry on reconnect"
            );
            let mut act = active.lock().await;
            if let Some(tx) = act.iter_mut().find(|t| t.transaction_id == transaction_id) {
                tx.pending_stop = Some(PendingStop {
                    meter_stop,
                    timestamp,
                    reason,
                });
                if let Err(e) = state.put_tx(tx) {
                    warn!(transaction_id, error=%e, "failed to persist pending_stop");
                }
            }
            false
        }
    }
}

/// Walk the supplied active transactions, retrying any with `pending_stop`.
/// Successfully-confirmed stops are removed from `state` and dropped from
/// the returned list; failed retries are kept and will be tried again on the
/// next reconnect.
async fn retry_pending_stops_with<F, Fut>(
    state: &CpState,
    txs: Vec<ActiveTransaction>,
    mut send: F,
) -> Vec<ActiveTransaction>
where
    F: FnMut(i32, &PendingStop) -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    let mut keep = Vec::with_capacity(txs.len());
    for tx in txs {
        let Some(ps) = tx.pending_stop.clone() else {
            keep.push(tx);
            continue;
        };
        match send(tx.transaction_id, &ps).await {
            Ok(()) => {
                if let Err(e) = state.remove_tx(tx.transaction_id) {
                    warn!(transaction_id = tx.transaction_id, error=%e, "failed to clear active tx after pending stop retry");
                }
                info!(
                    transaction_id = tx.transaction_id,
                    "pending StopTransaction delivered on reconnect"
                );
            }
            Err(e) => {
                warn!(
                    transaction_id = tx.transaction_id,
                    error = %e,
                    "pending StopTransaction retry still failing; will try again next reconnect"
                );
                keep.push(tx);
            }
        }
    }
    keep
}

#[cfg(test)]
mod tests {
    use super::*;
    use ocpp_store::queue::PendingCall;
    use std::sync::Mutex as StdMutex;

    fn pc(action: &str, n: i32) -> PendingCall {
        PendingCall {
            action: action.to_string(),
            payload: serde_json::json!({ "seq": n }),
        }
    }

    fn open_tmp_store() -> (tempfile::TempDir, ocpp_store::Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = ocpp_store::Store::open(dir.path()).unwrap();
        (dir, store)
    }

    #[tokio::test]
    async fn replay_acks_all_on_success_and_in_fifo_order() {
        let (_d, store) = open_tmp_store();
        let q = store.queue("CP-1").unwrap();
        q.enqueue(&pc("A", 1)).unwrap();
        q.enqueue(&pc("B", 2)).unwrap();
        q.enqueue(&pc("C", 3)).unwrap();

        let seen: StdMutex<Vec<String>> = StdMutex::new(Vec::new());
        replay_queue_with(&q, |action, _payload| {
            seen.lock().unwrap().push(action.to_string());
            async { Ok(()) }
        })
        .await;

        assert_eq!(*seen.lock().unwrap(), vec!["A", "B", "C"]);
        assert!(q.is_empty(), "all entries should be acked on success");
    }

    #[tokio::test]
    async fn replay_halts_on_failure_and_preserves_remaining_entries() {
        let (_d, store) = open_tmp_store();
        let q = store.queue("CP-1").unwrap();
        q.enqueue(&pc("A", 1)).unwrap();
        q.enqueue(&pc("B", 2)).unwrap();
        q.enqueue(&pc("C", 3)).unwrap();

        let calls: StdMutex<u32> = StdMutex::new(0);
        replay_queue_with(&q, |action, _payload| {
            let mut n = calls.lock().unwrap();
            *n += 1;
            let count = *n;
            let action = action.to_string();
            async move {
                // Fail on B (the second call) to simulate an offline CSMS
                // mid-replay.
                if action == "B" {
                    Err(format!("simulated failure at #{count}"))
                } else {
                    Ok(())
                }
            }
        })
        .await;

        assert_eq!(*calls.lock().unwrap(), 2, "must stop at first failure");
        // A was acked; B and C remain in the queue, in order.
        let remaining: Vec<_> = q.drain_iter().collect::<Result<_, _>>().unwrap();
        assert_eq!(remaining.len(), 2);
        assert_eq!(remaining[0].1.action, "B");
        assert_eq!(remaining[1].1.action, "C");
    }

    #[tokio::test]
    async fn replay_on_empty_queue_is_noop() {
        let (_d, store) = open_tmp_store();
        let q = store.queue("CP-1").unwrap();
        let mut called = false;
        replay_queue_with(&q, |_, _| {
            called = true;
            async { Ok(()) }
        })
        .await;
        assert!(!called);
    }

    fn sample_tx(id: i32) -> ActiveTransaction {
        ActiveTransaction {
            transaction_id: id,
            connector_id: 1,
            id_tag: "TAG".into(),
            meter_start: 0,
            started_at: Utc::now(),
            pending_stop: None,
        }
    }

    #[tokio::test]
    async fn failed_stop_persists_pending_and_keeps_active_tx() {
        let (_d, store) = open_tmp_store();
        let state = store.state("CP-1").unwrap();
        let tx = sample_tx(42);
        state.put_tx(&tx).unwrap();
        let active = Mutex::new(vec![tx]);

        let delivered = stop_or_persist(
            &state,
            &active,
            42,
            1234,
            Utc::now(),
            Some(StopReason::Local),
            || async { Err::<(), String>("offline".into()) },
        )
        .await;

        assert!(!delivered, "send was simulated failed");
        // Active tx must still be present, with pending_stop populated.
        let listed = state.list_tx().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].transaction_id, 42);
        let ps = listed[0]
            .pending_stop
            .as_ref()
            .expect("pending_stop should be set");
        assert_eq!(ps.meter_stop, 1234);
        assert_eq!(ps.reason, Some(StopReason::Local));
        // In-memory active list must also still hold the tx (with pending_stop).
        let mem = active.lock().await;
        assert_eq!(mem.len(), 1);
        assert!(mem[0].pending_stop.is_some());
    }

    #[tokio::test]
    async fn successful_stop_clears_active_tx() {
        let (_d, store) = open_tmp_store();
        let state = store.state("CP-1").unwrap();
        let tx = sample_tx(7);
        state.put_tx(&tx).unwrap();
        let active = Mutex::new(vec![tx]);

        let delivered = stop_or_persist(
            &state,
            &active,
            7,
            500,
            Utc::now(),
            None,
            || async { Ok::<(), String>(()) },
        )
        .await;

        assert!(delivered);
        assert!(state.list_tx().unwrap().is_empty());
        assert!(active.lock().await.is_empty());
    }

    #[tokio::test]
    async fn retry_pending_stops_clears_on_success_and_keeps_on_failure() {
        let (_d, store) = open_tmp_store();
        let state = store.state("CP-1").unwrap();

        // tx 1: has pending_stop, retry will succeed.
        let mut t1 = sample_tx(1);
        t1.pending_stop = Some(PendingStop {
            meter_stop: 100,
            timestamp: Utc::now(),
            reason: None,
        });
        state.put_tx(&t1).unwrap();

        // tx 2: has pending_stop, retry will fail.
        let mut t2 = sample_tx(2);
        t2.pending_stop = Some(PendingStop {
            meter_stop: 200,
            timestamp: Utc::now(),
            reason: Some(StopReason::PowerLoss),
        });
        state.put_tx(&t2).unwrap();

        // tx 3: no pending_stop, must be carried through untouched.
        let t3 = sample_tx(3);
        state.put_tx(&t3).unwrap();

        let txs = state.list_tx().unwrap();
        let kept = retry_pending_stops_with(&state, txs, |tx_id, _ps| async move {
            if tx_id == 1 {
                Ok(())
            } else {
                Err("still offline".to_string())
            }
        })
        .await;

        // tx 1 cleared; tx 2 + tx 3 remain.
        let kept_ids: Vec<_> = kept.iter().map(|t| t.transaction_id).collect();
        assert!(!kept_ids.contains(&1));
        assert!(kept_ids.contains(&2));
        assert!(kept_ids.contains(&3));

        let persisted_ids: Vec<_> = state
            .list_tx()
            .unwrap()
            .iter()
            .map(|t| t.transaction_id)
            .collect();
        assert!(!persisted_ids.contains(&1), "tx 1 should be cleared from store");
        assert!(persisted_ids.contains(&2));
        assert!(persisted_ids.contains(&3));
    }

    #[tokio::test]
    async fn replay_breaks_on_first_failure() {
        let (_d, store) = open_tmp_store();
        let queue = store.queue("CP-1").unwrap();

        // Enqueue 3 messages using the real API.
        queue.enqueue(&pc("BootNotification", 1)).unwrap();
        queue.enqueue(&pc("StatusNotification", 2)).unwrap();
        queue.enqueue(&pc("Heartbeat", 3)).unwrap();

        let calls: StdMutex<Vec<String>> = StdMutex::new(Vec::new());
        replay_queue_with(&queue, |action, _payload| {
            let mut v = calls.lock().unwrap();
            v.push(action.to_string());
            let n = v.len();
            drop(v);
            async move {
                // Fail on the second message to simulate mid-replay CSMS outage.
                if n == 2 {
                    Err(format!("simulated failure at #{n}"))
                } else {
                    Ok(())
                }
            }
        })
        .await;

        let seen = calls.lock().unwrap();
        // Must have stopped after 2 attempts (1 success + 1 failure).
        assert_eq!(seen.len(), 2, "must not attempt messages past first failure");
        assert_eq!(seen[0], "BootNotification");
        assert_eq!(seen[1], "StatusNotification");
        drop(seen);

        // BootNotification was acked; StatusNotification and Heartbeat must remain.
        let remaining: Vec<_> = queue.drain_iter().collect::<Result<_, _>>().unwrap();
        assert_eq!(remaining.len(), 2, "failed message and subsequent must remain in queue");
        assert_eq!(remaining[0].1.action, "StatusNotification", "failed msg must be at head");
        assert_eq!(remaining[1].1.action, "Heartbeat");
    }
}
