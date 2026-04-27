use std::sync::Arc;
use chrono::Utc;
use ocpp_protocol::enums::RegistrationStatus;
use ocpp_protocol::messages::BootNotificationRequest;
use ocpp_protocol::Ocpp16;
use ocpp_store::queue::OutboundQueue;
use ocpp_store::state::{BootInfo, CpState};
use ocpp_transport::{Session, SessionConfig};
use tracing::{info, warn};

pub struct ConnectivityManager {
    cp_id: String,
    vendor: String,
    model: String,
    #[allow(dead_code)]
    session_cfg: SessionConfig,
}

impl ConnectivityManager {
    pub fn new(cp_id: String, vendor: String, model: String, session_cfg: SessionConfig) -> Self {
        Self { cp_id, vendor, model, session_cfg }
    }

    pub async fn boot(
        &self,
        session: &Arc<Session<Ocpp16>>,
        state: &CpState,
    ) -> anyhow::Result<u64> {
        let boot_req = BootNotificationRequest {
            charge_point_vendor: self.vendor.clone(),
            charge_point_model: self.model.clone(),
            ..Default::default()
        };
        let boot = session.call(boot_req).await?;
        info!(cp=%self.cp_id, status=?boot.status, interval=boot.interval, "BootNotification reply");
        
        state.put_boot(&BootInfo {
            status: format!("{:?}", boot.status),
            interval: boot.interval,
            last_boot: Utc::now(),
        })?;

        if !matches!(boot.status, RegistrationStatus::Accepted) {
            return Err(anyhow::anyhow!("BootNotification transition to {:?} not handled", boot.status));
        }

        // Notify CSMS that Charge Point is Available after boot (Mandatory in 1.6)
        let _ = session.call(ocpp_protocol::messages::StatusNotificationRequest {
            connector_id: 0,
            error_code: ocpp_protocol::enums::ChargePointErrorCode::NoError,
            status: ocpp_protocol::enums::ChargePointStatus::Available,
            info: Some("Init after boot".to_string()),
            timestamp: Some(Utc::now()),
            vendor_id: None,
            vendor_error_code: None,
        }).await;

        Ok(boot.interval.max(1) as u64)
    }

    pub async fn replay_queue(&self, queue: &OutboundQueue, session: &Arc<Session<Ocpp16>>) {
        if queue.is_empty() {
            return;
        }
        info!(cp=%self.cp_id, pending = queue.len(), "replaying offline queue");
        let snapshot: Vec<_> = queue.drain_iter().filter_map(|r| r.ok()).collect();
        for (id, call) in snapshot {
            match session.call_raw(&call.action, call.payload.clone()).await {
                Ok(_) => {
                    let _ = queue.ack(id);
                }
                Err(e) => {
                    warn!(cp=%self.cp_id, id, action=%call.action, error=%e, "replay failed; halting");
                    return;
                }
            }
        }
    }
}
