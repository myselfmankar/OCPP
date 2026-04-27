use std::sync::Arc;
use chrono::Utc;
use ocpp_protocol::enums::{ChargePointErrorCode, ChargePointStatus};
use ocpp_protocol::messages::{
    DiagnosticsStatusNotificationRequest, FirmwareStatusNotificationRequest, HeartbeatRequest,
    StatusNotificationRequest,
};
use ocpp_protocol::Ocpp16;
use ocpp_store::auth::AuthStore;
use ocpp_store::config::ConfigStore;
use ocpp_transport::Session;
use tracing::warn;

use crate::events::DeviceEvent;

use tokio::sync::Mutex;

pub struct MetadataManager {
    cp_id: String,
    config: ConfigStore,
    auth: AuthStore,
    last_firmware_status: Mutex<Option<ocpp_protocol::enums::FirmwareStatus>>,
    last_diagnostics_status: Mutex<Option<ocpp_protocol::enums::DiagnosticsStatus>>,
}

impl MetadataManager {
    pub fn new(cp_id: String, config: ConfigStore, auth: AuthStore) -> Self {
        Self {
            cp_id,
            config,
            auth,
            last_firmware_status: Mutex::new(None),
            last_diagnostics_status: Mutex::new(None),
        }
    }

    pub async fn handle_event(
        &self,
        session: &Arc<Session<Ocpp16>>,
        event: DeviceEvent,
    ) -> anyhow::Result<()> {
        match event {
            DeviceEvent::Plugged { connector_id } => {
                self.send_status(session, connector_id, ChargePointStatus::Preparing, None).await;
            }
            DeviceEvent::Unplugged { connector_id } => {
                self.send_status(session, connector_id, ChargePointStatus::Available, None).await;
            }
            DeviceEvent::Status {
                connector_id,
                status,
                error_code,
                info,
            } => {
                let _ = session.call(StatusNotificationRequest {
                    connector_id,
                    error_code,
                    status,
                    info,
                    timestamp: Some(Utc::now()),
                    vendor_id: None,
                    vendor_error_code: None,
                }).await;
            }
            DeviceEvent::FirmwareStatus { status } => {
                *self.last_firmware_status.lock().await = Some(status);
                let _ = session.call(FirmwareStatusNotificationRequest { status }).await;
            }
            DeviceEvent::DiagnosticsStatus { status } => {
                *self.last_diagnostics_status.lock().await = Some(status);
                let _ = session.call(DiagnosticsStatusNotificationRequest { status }).await;
            }
            _ => {}
        }
        Ok(())
    }

    pub async fn report_firmware_status(&self, session: &Arc<Session<Ocpp16>>) {
        let status = self.last_firmware_status.lock().await.unwrap_or(ocpp_protocol::enums::FirmwareStatus::Idle);
        let _ = session.call(FirmwareStatusNotificationRequest { status }).await;
    }

    pub async fn report_diagnostics_status(&self, session: &Arc<Session<Ocpp16>>) {
        let status = self.last_diagnostics_status.lock().await.unwrap_or(ocpp_protocol::enums::DiagnosticsStatus::Idle);
        let _ = session.call(DiagnosticsStatusNotificationRequest { status }).await;
    }

    pub async fn heartbeat(&self, session: &Arc<Session<Ocpp16>>) {
        if let Err(e) = session.call(HeartbeatRequest::default()).await {
            warn!(cp=%self.cp_id, error=%e, "Heartbeat failed");
        }
    }

    async fn send_status(
        &self,
        session: &Arc<Session<Ocpp16>>,
        connector_id: i32,
        status: ChargePointStatus,
        info: Option<String>,
    ) {
        let _ = session.call(StatusNotificationRequest {
            connector_id,
            error_code: ChargePointErrorCode::NoError,
            status,
            info,
            timestamp: Some(Utc::now()),
            vendor_id: None,
            vendor_error_code: None,
        }).await;
    }

    pub fn config(&self) -> &ConfigStore {
        &self.config
    }

    pub fn auth(&self) -> &AuthStore {
        &self.auth
    }
}
