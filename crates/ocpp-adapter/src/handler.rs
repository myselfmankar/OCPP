use std::sync::Arc;

use async_trait::async_trait;
use ocpp_protocol::enums::{
    ConfigurationStatus, RemoteStartStopStatus, ResetStatus, TriggerMessageStatus, UnlockStatus,
};
use ocpp_protocol::messages::{
    ChangeConfigurationRequest, ChangeConfigurationResponse, GetConfigurationRequest,
    GetConfigurationResponse, RemoteStartTransactionRequest, RemoteStartTransactionResponse,
    RemoteStopTransactionRequest, RemoteStopTransactionResponse, ResetRequest, ResetResponse,
    TriggerMessageRequest, TriggerMessageResponse, UnlockConnectorRequest, UnlockConnectorResponse,
};
use ocpp_transport::{CsmsHandler, dispatcher::{HandlerError, HandlerResult}};
use tracing::info;

use crate::device::Device;
use crate::events::DeviceCommand;

/// `CsmsHandler` impl that translates incoming CSMS calls into `DeviceCommand`s.
///
/// Returns `Accepted` once the command has been delivered to the device. The
/// device-side acknowledgement (success/failure) is reported asynchronously
/// via `DeviceEvent`s and does not block the OCPP response.
pub struct AdapterHandler {
    device: Arc<dyn Device>,
}

impl AdapterHandler {
    pub fn new(device: Arc<dyn Device>) -> Self {
        Self { device }
    }

    async fn dispatch_cmd(&self, cmd: DeviceCommand) -> HandlerResult<()> {
        self.device
            .send(cmd)
            .await
            .map_err(|e| HandlerError::internal(format!("device: {e}")))
    }
}

#[async_trait]
impl CsmsHandler for AdapterHandler {
    async fn remote_start_transaction(
        &self,
        req: RemoteStartTransactionRequest,
    ) -> HandlerResult<RemoteStartTransactionResponse> {
        info!(id_tag = %req.id_tag, connector = ?req.connector_id, "RemoteStartTransaction");
        self.dispatch_cmd(DeviceCommand::StartCharging {
            connector_id: req.connector_id,
            id_tag: req.id_tag,
        })
        .await?;
        Ok(RemoteStartTransactionResponse {
            status: RemoteStartStopStatus::Accepted,
        })
    }

    async fn remote_stop_transaction(
        &self,
        req: RemoteStopTransactionRequest,
    ) -> HandlerResult<RemoteStopTransactionResponse> {
        info!(tx = req.transaction_id, "RemoteStopTransaction");
        self.dispatch_cmd(DeviceCommand::StopCharging {
            transaction_id: req.transaction_id,
        })
        .await?;
        Ok(RemoteStopTransactionResponse {
            status: RemoteStartStopStatus::Accepted,
        })
    }

    async fn trigger_message(
        &self,
        req: TriggerMessageRequest,
    ) -> HandlerResult<TriggerMessageResponse> {
        info!(message = ?req.requested_message, "TriggerMessage");
        // The ChargePoint actor watches a side channel for triggers. For v1
        // we acknowledge `Accepted` for messages the actor handles itself
        // (BootNotification, Heartbeat, StatusNotification, MeterValues).
        Ok(TriggerMessageResponse {
            status: TriggerMessageStatus::Accepted,
        })
    }

    async fn reset(&self, req: ResetRequest) -> HandlerResult<ResetResponse> {
        info!(reset_type = ?req.reset_type, "Reset");
        let hard = matches!(req.reset_type, ocpp_protocol::enums::ResetType::Hard);
        self.dispatch_cmd(DeviceCommand::Reboot { hard }).await?;
        Ok(ResetResponse {
            status: ResetStatus::Accepted,
        })
    }

    async fn change_configuration(
        &self,
        req: ChangeConfigurationRequest,
    ) -> HandlerResult<ChangeConfigurationResponse> {
        info!(key = %req.key, "ChangeConfiguration");
        self.dispatch_cmd(DeviceCommand::SetConfig {
            key: req.key,
            value: req.value,
        })
        .await?;
        Ok(ChangeConfigurationResponse {
            status: ConfigurationStatus::Accepted,
        })
    }

    async fn get_configuration(
        &self,
        _req: GetConfigurationRequest,
    ) -> HandlerResult<GetConfigurationResponse> {
        // v1: the gateway has no persistent CP-side configuration store yet.
        // Return an empty key list so the CSMS at least gets a well-formed reply.
        Ok(GetConfigurationResponse::default())
    }

    async fn unlock_connector(
        &self,
        req: UnlockConnectorRequest,
    ) -> HandlerResult<UnlockConnectorResponse> {
        info!(connector = req.connector_id, "UnlockConnector");
        self.dispatch_cmd(DeviceCommand::Unlock {
            connector_id: req.connector_id,
        })
        .await?;
        Ok(UnlockConnectorResponse {
            status: UnlockStatus::Unlocked,
        })
    }
}
