use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use ocpp_protocol::enums::{
    AvailabilityStatus, CancelReservationStatus, ChargingProfileStatus, ClearCacheStatus,
    ClearChargingProfileStatus, ConfigurationStatus, GetCompositeScheduleStatus,
    RemoteStartStopStatus, ReservationStatus, ResetStatus, TriggerMessageStatus, UnlockStatus,
    UpdateStatus,
};
use ocpp_protocol::messages::{
    CancelReservationRequest, CancelReservationResponse, ChangeAvailabilityRequest,
    ChangeAvailabilityResponse, ChangeConfigurationRequest, ChangeConfigurationResponse,
    ChargingProfile, ClearCacheRequest, ClearCacheResponse, ClearChargingProfileRequest,
    ClearChargingProfileResponse, ConfigurationKey, DataTransferRequest, DataTransferResponse,
    GetCompositeScheduleRequest, GetCompositeScheduleResponse, GetConfigurationRequest,
    GetConfigurationResponse, GetDiagnosticsRequest, GetDiagnosticsResponse,
    GetLocalListVersionRequest, GetLocalListVersionResponse, RemoteStartTransactionRequest,
    RemoteStartTransactionResponse, RemoteStopTransactionRequest, RemoteStopTransactionResponse,
    ReserveNowRequest, ReserveNowResponse, ResetRequest, ResetResponse, SendLocalListRequest,
    SendLocalListResponse, SetChargingProfileRequest, SetChargingProfileResponse,
    TriggerMessageRequest, TriggerMessageResponse, UnlockConnectorRequest, UnlockConnectorResponse,
    UpdateFirmwareRequest, UpdateFirmwareResponse,
};
use ocpp_transport::dispatcher::{HandlerError, HandlerResult};
use ocpp_transport::CsmsHandler;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
use tracing::info;
use uuid::Uuid;

use crate::events::{DeviceAck, DeviceCommand, DeviceEvent};
use crate::metadata::MetadataManager;
use crate::transaction::{SmartChargingEngine, TransactionManager};
use crate::Device;

/// `CsmsHandler` impl that translates incoming CSMS calls into `DeviceCommand`s.
///
/// Returns `Accepted` once the command has been delivered to the device. The
/// device-side acknowledgement (success/failure) is reported asynchronously
/// via `DeviceEvent`s and does not block the OCPP response.
pub struct AdapterHandler {
    device: Arc<dyn Device>,
    trigger_tx: mpsc::Sender<ocpp_protocol::enums::MessageTrigger>,
    metadata: Arc<MetadataManager>,
    transactions: Arc<TransactionManager>,
}

impl AdapterHandler {
    pub fn new(
        device: Arc<dyn Device>,
        trigger_tx: mpsc::Sender<ocpp_protocol::enums::MessageTrigger>,
        metadata: Arc<MetadataManager>,
        transactions: Arc<TransactionManager>,
    ) -> Self {
        Self {
            device,
            trigger_tx,
            metadata,
            transactions,
        }
    }

    async fn dispatch_cmd(&self, cmd: DeviceCommand) -> HandlerResult<()> {
        self.device
            .send(cmd)
            .await
            .map_err(|e| HandlerError::internal(format!("device: {e}")))
    }

    async fn block_on_cmd<F>(&self, make_cmd: F) -> HandlerResult<DeviceAck>
    where
        F: FnOnce(String) -> DeviceCommand,
    {
        let command_id = Uuid::new_v4().to_string();
        let mut events = self
            .device
            .events()
            .await
            .map_err(|e| HandlerError::internal(format!("device events: {e}")))?;
        let cmd = make_cmd(command_id.clone());
        self.dispatch_cmd(cmd).await?;

        match timeout(Duration::from_secs(10), async move {
            while let Some(event) = events.recv().await {
                if let DeviceEvent::CommandAck {
                    command_id: id,
                    status,
                } = event
                {
                    if id == command_id {
                        return Ok(status);
                    }
                }
            }
            Err(HandlerError::internal("command-ack stream closed"))
        })
        .await
        {
            Ok(result) => result,
            Err(_) => Err(HandlerError::internal("command-ack timeout")),
        }
    }

    async fn wait_for_diagnostics_file(
        &self,
        command_id: String,
        mut events: mpsc::Receiver<DeviceEvent>,
    ) -> Option<String> {
        timeout(Duration::from_secs(5), async move {
            while let Some(event) = events.recv().await {
                if let DeviceEvent::DiagnosticsFile {
                    command_id: id,
                    file_name,
                } = event
                {
                    if id == command_id {
                        return file_name;
                    }
                }
            }
            None
        })
        .await
        .ok()
        .flatten()
    }

    async fn wait_for_data_transfer(
        &self,
        command_id: String,
        mut events: mpsc::Receiver<DeviceEvent>,
    ) -> (ocpp_protocol::enums::DataTransferStatus, Option<String>) {
        timeout(Duration::from_secs(10), async move {
            while let Some(event) = events.recv().await {
                if let DeviceEvent::DataTransferResult {
                    command_id: id,
                    status,
                    data,
                } = event
                {
                    if id == command_id {
                        return (status, data);
                    }
                }
            }
            (ocpp_protocol::enums::DataTransferStatus::Rejected, None)
        })
        .await
        .unwrap_or((ocpp_protocol::enums::DataTransferStatus::Rejected, None))
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
            .block_on_cmd(|command_id| DeviceCommand::StartCharging {
                command_id,
                connector_id: req.connector_id,
                id_tag: req.id_tag,
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
            .block_on_cmd(|command_id| DeviceCommand::StopCharging {
                command_id,
                transaction_id: req.transaction_id,
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
        let supported = matches!(
            req.requested_message,
            ocpp_protocol::enums::MessageTrigger::BootNotification
                | ocpp_protocol::enums::MessageTrigger::Heartbeat
                | ocpp_protocol::enums::MessageTrigger::FirmwareStatusNotification
                | ocpp_protocol::enums::MessageTrigger::DiagnosticsStatusNotification
        );

        if !supported {
            return Ok(TriggerMessageResponse {
                status: TriggerMessageStatus::NotImplemented,
            });
        }

        match self.trigger_tx.send(req.requested_message).await {
            Ok(_) => Ok(TriggerMessageResponse {
                status: TriggerMessageStatus::Accepted,
            }),
            Err(_) => Ok(TriggerMessageResponse {
                status: TriggerMessageStatus::Rejected,
            }),
        }
    }

    async fn clear_cache(&self, _req: ClearCacheRequest) -> HandlerResult<ClearCacheResponse> {
        info!("ClearCache");
        match self.metadata.auth().clear_cache() {
            Ok(()) => Ok(ClearCacheResponse {
                status: ClearCacheStatus::Accepted,
            }),
            Err(e) => Err(HandlerError::internal(e.to_string())),
        }
    }

    async fn reset(&self, req: ResetRequest) -> HandlerResult<ResetResponse> {
        info!(reset_type = ?req.reset_type, "Reset");
        let hard = matches!(req.reset_type, ocpp_protocol::enums::ResetType::Hard);
        let ack = self
            .block_on_cmd(|command_id| DeviceCommand::Reboot { command_id, hard })
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
        let key = req.key;
        let value = req.value;
        let ack = self
            .block_on_cmd(|command_id| DeviceCommand::SetConfig {
                command_id,
                key: key.clone(),
                value: value.clone(),
            })
            .await?;

        let status = match ack {
            DeviceAck::Accepted => {
                if let Err(e) = self.metadata.config().set(&key, &value) {
                    return Err(HandlerError::internal(format!("store: {e}")));
                }
                ConfigurationStatus::Accepted
            }
            DeviceAck::Rejected => ConfigurationStatus::Rejected,
            DeviceAck::NotSupported => ConfigurationStatus::NotSupported,
            DeviceAck::Failed => ConfigurationStatus::Rejected,
        };

        Ok(ChangeConfigurationResponse { status })
    }

    async fn get_configuration(
        &self,
        req: GetConfigurationRequest,
    ) -> HandlerResult<GetConfigurationResponse> {
        info!("GetConfiguration");
        let mut keys = Vec::new();
        let mut unknown = Vec::new();

        if let Some(requested) = req.key {
            for k in requested {
                if let Ok(Some(v)) = self.metadata.config().get(&k) {
                    keys.push(ConfigurationKey {
                        key: k,
                        readonly: false, // Default to false for now
                        value: Some(v),
                    });
                } else {
                    unknown.push(k);
                }
            }
        } else {
            // Return all
            if let Ok(all) = self.metadata.config().list() {
                for (k, v) in all {
                    keys.push(ConfigurationKey {
                        key: k,
                        readonly: false,
                        value: Some(v),
                    });
                }
            }
        }

        Ok(GetConfigurationResponse {
            configuration_key: Some(keys),
            unknown_key: if unknown.is_empty() {
                None
            } else {
                Some(unknown)
            },
        })
    }

    async fn unlock_connector(
        &self,
        req: UnlockConnectorRequest,
    ) -> HandlerResult<UnlockConnectorResponse> {
        info!(connector = req.connector_id, "UnlockConnector");
        let ack = self
            .block_on_cmd(|command_id| DeviceCommand::Unlock {
                command_id,
                connector_id: req.connector_id,
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
            .block_on_cmd(|command_id| DeviceCommand::SetAvailability {
                command_id,
                connector_id: req.connector_id,
                availability_type: req.r#type,
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
        let command_id = Uuid::new_v4().to_string();
        let events = self
            .device
            .events()
            .await
            .map_err(|e| HandlerError::internal(format!("device events: {e}")))?;
        let cmd = DeviceCommand::GetDiagnostics {
            command_id: command_id.clone(),
            location: req.location,
            retries: req.retries,
            retry_interval: req.retry_interval,
            start_time: req.start_time,
            stop_time: req.stop_time,
        };
        self.device
            .send(cmd)
            .await
            .map_err(|e| HandlerError::internal(e.to_string()))?;
        let file_name = self.wait_for_diagnostics_file(command_id, events).await;
        Ok(GetDiagnosticsResponse { file_name })
    }

    async fn get_local_list_version(
        &self,
        _req: GetLocalListVersionRequest,
    ) -> HandlerResult<GetLocalListVersionResponse> {
        info!("GetLocalListVersion");
        let list_version = self.metadata.auth().get_version().unwrap_or(0);
        Ok(GetLocalListVersionResponse { list_version })
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
        let full = matches!(req.update_type, ocpp_protocol::enums::UpdateType::Full);
        match self
            .metadata
            .auth()
            .update_list(req.list_version, req.local_authorization_list, full)
        {
            Ok(()) => Ok(SendLocalListResponse {
                status: UpdateStatus::Accepted,
            }),
            Err(e) => Err(HandlerError::internal(format!("store: {e}"))),
        }
    }

    async fn reserve_now(&self, req: ReserveNowRequest) -> HandlerResult<ReserveNowResponse> {
        info!(connector = req.connector_id, id_tag = %req.id_tag, "ReserveNow");

        // Notify device first
        let ack = self
            .block_on_cmd(|command_id| DeviceCommand::ReserveNow {
                command_id,
                connector_id: req.connector_id,
                expiry_date: req.expiry_date,
                id_tag: req.id_tag.clone(),
                reservation_id: req.reservation_id,
            })
            .await?;

        if !matches!(ack, DeviceAck::Accepted) {
            let status = match ack {
                DeviceAck::Rejected => ReservationStatus::Rejected,
                DeviceAck::Failed => ReservationStatus::Faulted,
                DeviceAck::NotSupported => ReservationStatus::Rejected,
                _ => ReservationStatus::Rejected,
            };
            return Ok(ReserveNowResponse { status });
        }

        // Persist reservation
        let res = ocpp_store::reservations::Reservation {
            connector_id: req.connector_id,
            expiry_date: req.expiry_date,
            id_tag: req.id_tag,
            reservation_id: req.reservation_id,
            parent_id_tag: req.parent_id_tag,
        };

        if let Err(e) = self.transactions.reservations.set(res) {
            return Err(HandlerError::internal(format!("store: {e}")));
        }

        Ok(ReserveNowResponse {
            status: ReservationStatus::Accepted,
        })
    }

    async fn cancel_reservation(
        &self,
        req: CancelReservationRequest,
    ) -> HandlerResult<CancelReservationResponse> {
        info!(id = req.reservation_id, "CancelReservation");

        // Find connector for this reservation ID
        let res = self
            .transactions
            .reservations
            .find_by_id(req.reservation_id)
            .map_err(|e| HandlerError::internal(e.to_string()))?;

        if let Some(r) = res {
            let ack = self
                .block_on_cmd(|command_id| DeviceCommand::CancelReservation {
                    command_id,
                    reservation_id: req.reservation_id,
                })
                .await?;

            if matches!(ack, DeviceAck::Accepted) {
                let _ = self.transactions.reservations.delete(r.connector_id);
                Ok(CancelReservationResponse {
                    status: CancelReservationStatus::Accepted,
                })
            } else {
                Ok(CancelReservationResponse {
                    status: CancelReservationStatus::Rejected,
                })
            }
        } else {
            Ok(CancelReservationResponse {
                status: CancelReservationStatus::Rejected,
            })
        }
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
        if let Err(e) = self
            .transactions
            .profiles
            .set(req.connector_id, req.cs_charging_profiles)
        {
            return Err(HandlerError::internal(format!("store: {e}")));
        }
        Ok(SetChargingProfileResponse {
            status: ChargingProfileStatus::Accepted,
        })
    }

    async fn clear_charging_profile(
        &self,
        req: ClearChargingProfileRequest,
    ) -> HandlerResult<ClearChargingProfileResponse> {
        info!(id = ?req.id, connector = ?req.connector_id, "ClearChargingProfile");

        let all = self
            .transactions
            .profiles
            .list(req.connector_id)
            .map_err(|e| HandlerError::internal(e.to_string()))?;
        let mut cleared = false;

        for (cid, p) in all {
            let match_id = req.id.is_none_or(|id| p.charging_profile_id == id);
            let match_purpose = req
                .charging_profile_purpose
                .is_none_or(|purp| p.charging_profile_purpose == purp);
            let match_stack = req.stack_level.is_none_or(|stack| p.stack_level == stack);

            if match_id && match_purpose && match_stack {
                let _ = self
                    .transactions
                    .profiles
                    .delete(cid, p.charging_profile_id);
                cleared = true;
            }
        }

        let status = if cleared {
            ClearChargingProfileStatus::Accepted
        } else {
            ClearChargingProfileStatus::Unknown
        };
        Ok(ClearChargingProfileResponse { status })
    }

    async fn get_composite_schedule(
        &self,
        req: GetCompositeScheduleRequest,
    ) -> HandlerResult<GetCompositeScheduleResponse> {
        info!(
            connector = req.connector_id,
            duration = req.duration,
            "GetCompositeSchedule"
        );

        let all = self
            .transactions
            .profiles
            .list(None)
            .map_err(|e| HandlerError::internal(e.to_string()))?;
        let applicable: Vec<ChargingProfile> = all
            .into_iter()
            .filter(|(cid, _)| *cid == 0 || *cid == req.connector_id)
            .map(|(_, p)| p)
            .collect();

        if applicable.is_empty() {
            return Ok(GetCompositeScheduleResponse {
                status: GetCompositeScheduleStatus::Rejected,
                connector_id: None,
                schedule_start: None,
                charging_schedule: None,
            });
        }

        // For simplicity in 1.6 response, we return the current limit as a single period
        let now = Utc::now();
        if let Some(limit) = SmartChargingEngine::evaluate_combined(&applicable, now) {
            Ok(GetCompositeScheduleResponse {
                status: GetCompositeScheduleStatus::Accepted,
                connector_id: Some(req.connector_id),
                schedule_start: Some(now),
                charging_schedule: Some(ocpp_protocol::messages::ChargingSchedule {
                    duration: Some(req.duration),
                    start_schedule: Some(now),
                    charging_rate_unit: applicable[0].charging_schedule.charging_rate_unit,
                    charging_schedule_period: vec![
                        ocpp_protocol::messages::ChargingSchedulePeriod {
                            start_period: 0,
                            limit,
                            number_phases: None,
                        },
                    ],
                    min_charging_rate: None,
                }),
            })
        } else {
            Ok(GetCompositeScheduleResponse {
                status: GetCompositeScheduleStatus::Rejected,
                connector_id: None,
                schedule_start: None,
                charging_schedule: None,
            })
        }
    }

    async fn data_transfer(&self, req: DataTransferRequest) -> HandlerResult<DataTransferResponse> {
        info!(vendor = %req.vendor_id, "DataTransfer");
        let command_id = Uuid::new_v4().to_string();
        let events = self
            .device
            .events()
            .await
            .map_err(|e| HandlerError::internal(format!("device events: {e}")))?;
        let cmd = DeviceCommand::DataTransfer {
            command_id: command_id.clone(),
            vendor_id: req.vendor_id,
            message_id: req.message_id,
            data: req.data,
        };
        self.dispatch_cmd(cmd).await?;

        let (status, data) = self.wait_for_data_transfer(command_id, events).await;
        Ok(DataTransferResponse { status, data })
    }
}
