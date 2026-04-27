use async_trait::async_trait;
use ocpp_protocol::messages::{
    ChangeConfigurationRequest, ChangeConfigurationResponse, GetConfigurationRequest,
    GetConfigurationResponse, RemoteStartTransactionRequest, RemoteStartTransactionResponse,
    RemoteStopTransactionRequest, RemoteStopTransactionResponse, ResetRequest, ResetResponse,
    TriggerMessageRequest, TriggerMessageResponse, UnlockConnectorRequest, UnlockConnectorResponse,
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
