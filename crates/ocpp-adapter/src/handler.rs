use std::sync::Arc;

use async_trait::async_trait;
use ocpp_protocol::enums::{
    AvailabilityStatus, CancelReservationStatus, ChargingProfileStatus, ClearCacheStatus,
    ClearChargingProfileStatus, ConfigurationStatus, GetCompositeScheduleStatus,
    RemoteStartStopStatus, ReservationStatus, ResetStatus, TriggerMessageStatus, UnlockStatus,
    UpdateStatus,
};
use ocpp_protocol::messages::{
    CancelReservationRequest, CancelReservationResponse, ChangeAvailabilityRequest,
    ChangeAvailabilityResponse, ChangeConfigurationRequest, ChangeConfigurationResponse,
    ClearCacheRequest, ClearCacheResponse, ClearChargingProfileRequest,
    ClearChargingProfileResponse, GetCompositeScheduleRequest, GetCompositeScheduleResponse,
    GetConfigurationRequest, GetConfigurationResponse, GetDiagnosticsRequest,
    GetDiagnosticsResponse, GetLocalListVersionRequest, GetLocalListVersionResponse,
    RemoteStartTransactionRequest, RemoteStartTransactionResponse, RemoteStopTransactionRequest,
    RemoteStopTransactionResponse, ReserveNowRequest, ReserveNowResponse, ResetRequest,
    ResetResponse, SendLocalListRequest, SendLocalListResponse, SetChargingProfileRequest,
    SetChargingProfileResponse, TriggerMessageRequest, TriggerMessageResponse,
    UnlockConnectorRequest, UnlockConnectorResponse, UpdateFirmwareRequest, UpdateFirmwareResponse,
};
use ocpp_transport::{CsmsHandler, dispatcher::{HandlerError, HandlerResult}};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{timeout, Duration};
use tracing::info;

use crate::Device;
use crate::events::{DeviceAck, DeviceCommand};

/// `CsmsHandler` impl that translates incoming CSMS calls into `DeviceCommand`s.
///
/// Returns `Accepted` once the command has been delivered to the device. The
/// device-side acknowledgement (success/failure) is reported asynchronously
/// via `DeviceEvent`s and does not block the OCPP response.
pub struct AdapterHandler {
    device: Arc<dyn Device>,
    trigger_tx: mpsc::Sender<ocpp_protocol::enums::MessageTrigger>,
}

impl AdapterHandler {
    pub fn new(
        device: Arc<dyn Device>,
        trigger_tx: mpsc::Sender<ocpp_protocol::enums::MessageTrigger>,
    ) -> Self {
        Self { device, trigger_tx }
    }

    async fn dispatch_cmd(&self, cmd: DeviceCommand) -> HandlerResult<()> {
        self.device
            .send(cmd)
            .await
            .map_err(|e| HandlerError::internal(format!("device: {e}")))
    }

    async fn block_on_cmd<F>(&self, make_cmd: F) -> HandlerResult<DeviceAck>
    where
        F: FnOnce(oneshot::Sender<DeviceAck>) -> DeviceCommand,
    {
        let (tx, rx) = oneshot::channel();
        let cmd = make_cmd(tx);
        self.dispatch_cmd(cmd).await?;

        match timeout(Duration::from_secs(10), rx).await {
            Ok(Ok(ack)) => Ok(ack),
            Ok(Err(_)) => Err(HandlerError::internal("command-ack channel closed")),
            Err(_) => Err(HandlerError::internal("command-ack timeout")),
        }
    }
}

#[async_trait]
impl CsmsHandler for AdapterHandler {
    async fn remote_start_transaction(
        &self,
        req: RemoteStartTransactionRequest,
    ) -> HandlerResult<RemoteStartTransactionResponse> {
        info!(id_tag = %req.id_tag, connector = ?req.connector_id, "RemoteStartTransaction");
        let ack = self
            .block_on_cmd(|tx| DeviceCommand::StartCharging {
                connector_id: req.connector_id,
                id_tag: req.id_tag,
                ack_tx: Some(tx),
            })
            .await?;

        let status = match ack {
            DeviceAck::Accepted => RemoteStartStopStatus::Accepted,
            _ => RemoteStartStopStatus::Rejected,
        };

        Ok(RemoteStartTransactionResponse { status })
    }

    async fn remote_stop_transaction(
        &self,
        req: RemoteStopTransactionRequest,
    ) -> HandlerResult<RemoteStopTransactionResponse> {
        info!(tx = req.transaction_id, "RemoteStopTransaction");
        let ack = self
            .block_on_cmd(|tx| DeviceCommand::StopCharging {
                transaction_id: req.transaction_id,
                ack_tx: Some(tx),
            })
            .await?;

        let status = match ack {
            DeviceAck::Accepted => RemoteStartStopStatus::Accepted,
            _ => RemoteStartStopStatus::Rejected,
        };

        Ok(RemoteStopTransactionResponse { status })
    }

    async fn trigger_message(
        &self,
        req: TriggerMessageRequest,
    ) -> HandlerResult<TriggerMessageResponse> {
        info!(message = ?req.requested_message, "TriggerMessage");
        // Forward to the ChargePoint actor.
        match self.trigger_tx.send(req.requested_message).await {
            Ok(_) => Ok(TriggerMessageResponse {
                status: TriggerMessageStatus::Accepted,
            }),
            Err(_) => Ok(TriggerMessageResponse {
                status: TriggerMessageStatus::Rejected,
            }),
        }
    }

    async fn reset(&self, req: ResetRequest) -> HandlerResult<ResetResponse> {
        info!(reset_type = ?req.reset_type, "Reset");
        let hard = matches!(req.reset_type, ocpp_protocol::enums::ResetType::Hard);
        let ack = self
            .block_on_cmd(|tx| DeviceCommand::Reboot { hard, ack_tx: Some(tx) })
            .await?;

        let status = match ack {
            DeviceAck::Accepted => ResetStatus::Accepted,
            _ => ResetStatus::Rejected,
        };

        Ok(ResetResponse { status })
    }

    async fn change_configuration(
        &self,
        req: ChangeConfigurationRequest,
    ) -> HandlerResult<ChangeConfigurationResponse> {
        info!(key = %req.key, "ChangeConfiguration");
        let ack = self
            .block_on_cmd(|tx| DeviceCommand::SetConfig {
                key: req.key,
                value: req.value,
                ack_tx: Some(tx),
            })
            .await?;

        let status = match ack {
            DeviceAck::Accepted => ConfigurationStatus::Accepted,
            DeviceAck::Rejected => ConfigurationStatus::Rejected,
            DeviceAck::NotSupported => ConfigurationStatus::NotSupported,
            DeviceAck::Failed => ConfigurationStatus::Rejected,
        };

        Ok(ChangeConfigurationResponse { status })
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
        let ack = self
            .block_on_cmd(|tx| DeviceCommand::Unlock {
                connector_id: req.connector_id,
                ack_tx: Some(tx),
            })
            .await?;

        let status = match ack {
            DeviceAck::Accepted => UnlockStatus::Unlocked,
            DeviceAck::Rejected => UnlockStatus::UnlockFailed,
            DeviceAck::Failed => UnlockStatus::UnlockFailed,
            DeviceAck::NotSupported => UnlockStatus::NotSupported,
        };

        Ok(UnlockConnectorResponse { status })
    }

    async fn change_availability(
        &self,
        req: ChangeAvailabilityRequest,
    ) -> HandlerResult<ChangeAvailabilityResponse> {
        info!(connector = req.connector_id, r#type = ?req.r#type, "ChangeAvailability");
        let ack = self
            .block_on_cmd(|tx| DeviceCommand::SetAvailability {
                connector_id: req.connector_id,
                availability_type: req.r#type,
                ack_tx: Some(tx),
            })
            .await?;

        let status = match ack {
            DeviceAck::Accepted => AvailabilityStatus::Accepted,
            DeviceAck::Rejected => AvailabilityStatus::Rejected,
            DeviceAck::NotSupported => AvailabilityStatus::Rejected,
            DeviceAck::Failed => AvailabilityStatus::Rejected,
        };

        Ok(ChangeAvailabilityResponse { status })
    }

    async fn update_firmware(
        &self,
        req: UpdateFirmwareRequest,
    ) -> HandlerResult<UpdateFirmwareResponse> {
        info!(location = %req.location, retrieve_date = %req.retrieve_date, "UpdateFirmware");
        // Per OCPP 1.6 §5.16 the CP must respond immediately with an empty body.
        // We forward the command to the device in the background; progress is
        // reported asynchronously via FirmwareStatusNotification events.
        let cmd = DeviceCommand::UpdateFirmware {
            location: req.location,
            retrieve_date: req.retrieve_date,
            retries: req.retries,
            retry_interval: req.retry_interval,
        };
        self.device
            .send(cmd)
            .await
            .map_err(|e| HandlerError::internal(e.to_string()))?;
        Ok(UpdateFirmwareResponse {})
    }

    async fn get_diagnostics(
        &self,
        req: GetDiagnosticsRequest,
    ) -> HandlerResult<GetDiagnosticsResponse> {
        info!(location = %req.location, "GetDiagnostics");
        let (file_name_tx, file_name_rx) = oneshot::channel();
        let cmd = DeviceCommand::GetDiagnostics {
            location: req.location,
            retries: req.retries,
            retry_interval: req.retry_interval,
            start_time: req.start_time,
            stop_time: req.stop_time,
            file_name_tx: Some(file_name_tx),
        };
        self.device
            .send(cmd)
            .await
            .map_err(|e| HandlerError::internal(e.to_string()))?;
        // Wait up to 5 s for the device to report which file it will upload.
        let file_name = timeout(Duration::from_secs(5), file_name_rx)
            .await
            .ok()  // timeout
            .and_then(|r| r.ok())  // channel closed
            .flatten();
        Ok(GetDiagnosticsResponse { file_name })
    }

    async fn clear_cache(
        &self,
        _req: ClearCacheRequest,
    ) -> HandlerResult<ClearCacheResponse> {
        info!("ClearCache");
        let ack = self
            .block_on_cmd(|tx| DeviceCommand::ClearCache { ack_tx: Some(tx) })
            .await?;
        let status = match ack {
            DeviceAck::Accepted => ClearCacheStatus::Accepted,
            _ => ClearCacheStatus::Rejected,
        };
        Ok(ClearCacheResponse { status })
    }

    async fn get_local_list_version(
        &self,
        _req: GetLocalListVersionRequest,
    ) -> HandlerResult<GetLocalListVersionResponse> {
        info!("GetLocalListVersion");
        // No local list storage implemented yet; return version 0 (no list).
        // OCPP 1.6 §5.12: version 0 means no list is installed.
        Ok(GetLocalListVersionResponse { list_version: 0 })
    }

    async fn send_local_list(
        &self,
        req: SendLocalListRequest,
    ) -> HandlerResult<SendLocalListResponse> {
        info!(
            list_version = req.list_version,
            update_type = ?req.update_type,
            entries = req.local_authorization_list.len(),
            "SendLocalList"
        );
        // Honest stub: local list storage is not yet implemented.
        Ok(SendLocalListResponse { status: UpdateStatus::NotSupported })
    }

    async fn reserve_now(
        &self,
        req: ReserveNowRequest,
    ) -> HandlerResult<ReserveNowResponse> {
        info!(connector = req.connector_id, id_tag = %req.id_tag, "ReserveNow");
        let ack = self
            .block_on_cmd(|tx| DeviceCommand::ReserveNow {
                connector_id: req.connector_id,
                expiry_date: req.expiry_date,
                id_tag: req.id_tag,
                reservation_id: req.reservation_id,
                ack_tx: Some(tx),
            })
            .await?;

        let status = match ack {
            DeviceAck::Accepted => ReservationStatus::Accepted,
            DeviceAck::Rejected => ReservationStatus::Rejected,
            DeviceAck::Failed => ReservationStatus::Faulted,
            DeviceAck::NotSupported => ReservationStatus::Rejected,
        };

        Ok(ReserveNowResponse { status })
    }

    async fn cancel_reservation(
        &self,
        req: CancelReservationRequest,
    ) -> HandlerResult<CancelReservationResponse> {
        info!(id = req.reservation_id, "CancelReservation");
        let ack = self
            .block_on_cmd(|tx| DeviceCommand::CancelReservation {
                reservation_id: req.reservation_id,
                ack_tx: Some(tx),
            })
            .await?;

        let status = match ack {
            DeviceAck::Accepted => CancelReservationStatus::Accepted,
            _ => CancelReservationStatus::Rejected,
        };

        Ok(CancelReservationResponse { status })
    }

    async fn set_charging_profile(
        &self,
        req: SetChargingProfileRequest,
    ) -> HandlerResult<SetChargingProfileResponse> {
        info!(
            connector = req.connector_id,
            profile_id = req.cs_charging_profiles.charging_profile_id,
            "SetChargingProfile"
        );
        // Honest stub: charging profile management not yet implemented.
        Ok(SetChargingProfileResponse { status: ChargingProfileStatus::NotSupported })
    }

    async fn clear_charging_profile(
        &self,
        req: ClearChargingProfileRequest,
    ) -> HandlerResult<ClearChargingProfileResponse> {
        info!(id = ?req.id, "ClearChargingProfile");
        // Honest stub: nothing to clear if we don't store them.
        Ok(ClearChargingProfileResponse { status: ClearChargingProfileStatus::Accepted })
    }

    async fn get_composite_schedule(
        &self,
        req: GetCompositeScheduleRequest,
    ) -> HandlerResult<GetCompositeScheduleResponse> {
        info!(connector = req.connector_id, "GetCompositeSchedule");
        // Honest stub: schedule computation not yet implemented.
        Ok(GetCompositeScheduleResponse {
            status: GetCompositeScheduleStatus::Rejected,
            connector_id: None,
            schedule_start: None,
            charging_schedule: None,
        })
    }
}
