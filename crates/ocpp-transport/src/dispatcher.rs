use async_trait::async_trait;
use ocpp_protocol::messages::{
    CancelReservationRequest, CancelReservationResponse, ChangeAvailabilityRequest,
    ChangeAvailabilityResponse, ChangeConfigurationRequest,
    ChangeConfigurationResponse, ClearCacheRequest, ClearCacheResponse,
    ClearChargingProfileRequest, ClearChargingProfileResponse, GetConfigurationRequest,
    GetConfigurationResponse, GetCompositeScheduleRequest, GetCompositeScheduleResponse,
    GetDiagnosticsRequest, GetDiagnosticsResponse,
    GetLocalListVersionRequest, GetLocalListVersionResponse,
    RemoteStartTransactionRequest, RemoteStartTransactionResponse,
    RemoteStopTransactionRequest, RemoteStopTransactionResponse, ReserveNowRequest,
    ReserveNowResponse, ResetRequest, ResetResponse, SendLocalListRequest, SendLocalListResponse,
    SetChargingProfileRequest, SetChargingProfileResponse, TriggerMessageRequest,
    TriggerMessageResponse, UnlockConnectorRequest, UnlockConnectorResponse, UpdateFirmwareRequest,
    UpdateFirmwareResponse,
};
use ocpp_protocol::{Action, Call, CallError, CallErrorCode, CallResult, Frame};
use serde::Serialize;
use serde_json::Value;
use tracing::warn;

/// Result of a CSMS-initiated handler.
pub type HandlerResult<T> = Result<T, HandlerError>;

#[derive(Debug, thiserror::Error)]
#[error("{code:?}: {description}")]
pub struct HandlerError {
    pub code: CallErrorCode,
    pub description: String,
}

impl HandlerError {
    pub fn new(code: CallErrorCode, description: impl Into<String>) -> Self {
        Self { code, description: description.into() }
    }

    pub fn not_implemented() -> Self {
        Self::new(CallErrorCode::NotImplemented, "action not implemented")
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(CallErrorCode::InternalError, msg)
    }
}

/// Trait implemented by the adapter layer to handle CSMS-initiated `CALL`s.
/// Default impls return `NotImplemented` so unsupported actions degrade
/// gracefully.
#[async_trait]
pub trait CsmsHandler: Send + Sync {
    async fn remote_start_transaction(
        &self,
        _req: RemoteStartTransactionRequest,
    ) -> HandlerResult<RemoteStartTransactionResponse> {
        Err(HandlerError::not_implemented())
    }

    async fn remote_stop_transaction(
        &self,
        _req: RemoteStopTransactionRequest,
    ) -> HandlerResult<RemoteStopTransactionResponse> {
        Err(HandlerError::not_implemented())
    }

    async fn trigger_message(
        &self,
        _req: TriggerMessageRequest,
    ) -> HandlerResult<TriggerMessageResponse> {
        Err(HandlerError::not_implemented())
    }

    async fn reset(&self, _req: ResetRequest) -> HandlerResult<ResetResponse> {
        Err(HandlerError::not_implemented())
    }

    async fn change_configuration(
        &self,
        _req: ChangeConfigurationRequest,
    ) -> HandlerResult<ChangeConfigurationResponse> {
        Err(HandlerError::not_implemented())
    }

    async fn get_configuration(
        &self,
        _req: GetConfigurationRequest,
    ) -> HandlerResult<GetConfigurationResponse> {
        Err(HandlerError::not_implemented())
    }

    async fn unlock_connector(
        &self,
        _req: UnlockConnectorRequest,
    ) -> HandlerResult<UnlockConnectorResponse> {
        Err(HandlerError::not_implemented())
    }
    async fn change_availability(
        &self,
        _req: ChangeAvailabilityRequest,
    ) -> HandlerResult<ChangeAvailabilityResponse> {
        Err(HandlerError::not_implemented())
    }

    /// CSMS instructs CP to download and install new firmware.
    /// Per OCPP 1.6 §5.16, CP returns an empty response immediately;
    /// progress is reported asynchronously via `FirmwareStatusNotification`.
    async fn update_firmware(
        &self,
        _req: UpdateFirmwareRequest,
    ) -> HandlerResult<UpdateFirmwareResponse> {
        Err(HandlerError::not_implemented())
    }

    /// CSMS requests CP to upload a diagnostics file.
    /// Per OCPP 1.6 §5.10, CP responds immediately with the filename it will upload
    /// (or absent if none); progress is reported via `DiagnosticsStatusNotification`.
    async fn get_diagnostics(
        &self,
        _req: GetDiagnosticsRequest,
    ) -> HandlerResult<GetDiagnosticsResponse> {
        Err(HandlerError::not_implemented())
    }

    /// CSMS requests CP to clear its local authorization cache (OCPP 1.6 §5.4).
    async fn clear_cache(
        &self,
        _req: ClearCacheRequest,
    ) -> HandlerResult<ClearCacheResponse> {
        Err(HandlerError::not_implemented())
    }

    /// CSMS queries which version of the local auth list the CP holds (OCPP 1.6 §5.12).
    async fn get_local_list_version(
        &self,
        _req: GetLocalListVersionRequest,
    ) -> HandlerResult<GetLocalListVersionResponse> {
        Err(HandlerError::not_implemented())
    }

    /// CSMS pushes a local authorization list to the CP (OCPP 1.6 §5.12).
    async fn send_local_list(
        &self,
        _req: SendLocalListRequest,
    ) -> HandlerResult<SendLocalListResponse> {
        Err(HandlerError::not_implemented())
    }

    /// CSMS reserves a connector for an idTag (OCPP 1.6 §5.13).
    async fn reserve_now(
        &self,
        _req: ReserveNowRequest,
    ) -> HandlerResult<ReserveNowResponse> {
        Err(HandlerError::not_implemented())
    }

    /// CSMS cancels an existing reservation (OCPP 1.6 §5.13).
    async fn cancel_reservation(
        &self,
        _req: CancelReservationRequest,
    ) -> HandlerResult<CancelReservationResponse> {
        Err(HandlerError::not_implemented())
    }

    /// CSMS installs a charging profile (OCPP 1.6 §7.3).
    async fn set_charging_profile(
        &self,
        _req: SetChargingProfileRequest,
    ) -> HandlerResult<SetChargingProfileResponse> {
        Err(HandlerError::not_implemented())
    }

    /// CSMS clears charging profiles matching filters (OCPP 1.6 §7.4).
    async fn clear_charging_profile(
        &self,
        _req: ClearChargingProfileRequest,
    ) -> HandlerResult<ClearChargingProfileResponse> {
        Err(HandlerError::not_implemented())
    }

    /// CSMS requests composite schedule (OCPP 1.6 §7.5).
    async fn get_composite_schedule(
        &self,
        _req: GetCompositeScheduleRequest,
    ) -> HandlerResult<GetCompositeScheduleResponse> {
        Err(HandlerError::not_implemented())
    }
}

/// Dispatch an incoming `Call` to the right handler method and produce a
/// `CallResult` or `CallError` frame to send back.
pub async fn dispatch(handler: &dyn CsmsHandler, call: Call) -> Frame {
    let unique_id = call.unique_id.clone();
    let action = match Action::parse(&call.action) {
        Some(a) => a,
        None => {
            warn!(action = %call.action, "unknown action from CSMS");
            return err_frame(unique_id, CallErrorCode::NotImplemented, "unknown action");
        }
    };

    macro_rules! handle {
        ($variant:ident, $method:ident) => {{
            match serde_json::from_value(call.payload) {
                Ok(req) => match handler.$method(req).await {
                    Ok(resp) => result_frame(unique_id, &resp),
                    Err(e) => err_frame(unique_id, e.code, &e.description),
                },
                Err(e) => err_frame(
                    unique_id,
                    CallErrorCode::FormationViolation,
                    &format!("bad payload: {e}"),
                ),
            }
        }};
    }

    match action {
        Action::RemoteStartTransaction => handle!(RemoteStart, remote_start_transaction),
        Action::RemoteStopTransaction => handle!(RemoteStop, remote_stop_transaction),
        Action::TriggerMessage => handle!(Trigger, trigger_message),
        Action::Reset => handle!(Reset, reset),
        Action::ChangeConfiguration => handle!(ChangeConfig, change_configuration),
        Action::GetConfiguration => handle!(GetConfig, get_configuration),
        Action::UnlockConnector => handle!(Unlock, unlock_connector),
        Action::ChangeAvailability => handle!(ChangeAvailability, change_availability),
        Action::UpdateFirmware => handle!(UpdateFirmware, update_firmware),
        Action::GetDiagnostics => handle!(GetDiagnostics, get_diagnostics),
        Action::ClearCache => handle!(ClearCache, clear_cache),
        Action::GetLocalListVersion => handle!(GetLocalListVersion, get_local_list_version),
        Action::SendLocalList => handle!(SendLocalList, send_local_list),
        Action::ReserveNow => handle!(ReserveNow, reserve_now),
        Action::CancelReservation => handle!(CancelReservation, cancel_reservation),
        Action::SetChargingProfile => handle!(SetChargingProfile, set_charging_profile),
        Action::ClearChargingProfile => handle!(ClearChargingProfile, clear_charging_profile),
        Action::GetCompositeSchedule => handle!(GetCompositeSchedule, get_composite_schedule),
        // CP-initiated actions arriving from CSMS are protocol violations.
        _ => err_frame(
            unique_id,
            CallErrorCode::NotSupported,
            "action not allowed from CSMS",
        ),
    }
}

fn result_frame<T: Serialize>(unique_id: String, payload: &T) -> Frame {
    let payload = serde_json::to_value(payload).unwrap_or(Value::Object(Default::default()));
    Frame::Result(CallResult { unique_id, payload })
}

fn err_frame(unique_id: String, code: CallErrorCode, description: &str) -> Frame {
    Frame::Error(CallError {
        unique_id,
        error_code: code,
        error_description: description.to_string(),
        error_details: Value::Object(Default::default()),
    })
}
