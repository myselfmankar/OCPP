use ocpp_protocol::CallErrorCode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    // tungstenite::Error is ~136 bytes; box it to keep `TransportError`
    // (and `Result<_, TransportError>`) small. Triggers clippy::result_large_err
    // otherwise.
    #[error("websocket error: {0}")]
    Ws(#[from] Box<tokio_tungstenite::tungstenite::Error>),
    #[error("invalid url: {0}")]
    Url(#[from] url::ParseError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("tls config error: {0}")]
    Tls(String),
    #[error("protocol error: {0}")]
    Protocol(#[from] Box<ocpp_protocol::ProtocolError>),
    #[error("connection closed")]
    Closed,
    #[error("call timed out")]
    Timeout,
    #[error("not connected")]
    NotConnected,
    #[error("subprotocol mismatch: expected {expected}, got {got}")]
    SubprotocolMismatch { expected: String, got: String },
}

// Allow `?` on bare `tungstenite::Error` to auto-box.
impl From<tokio_tungstenite::tungstenite::Error> for TransportError {
    fn from(e: tokio_tungstenite::tungstenite::Error) -> Self {
        TransportError::Ws(Box::new(e))
    }
}

impl From<ocpp_protocol::ProtocolError> for TransportError {
    fn from(e: ocpp_protocol::ProtocolError) -> Self {
        TransportError::Protocol(Box::new(e))
    }
}

/// Returned when a `CALL` to the CSMS fails — either with a transport-level
/// error or a `CALLERROR` from the peer.
#[derive(Debug, Error)]
pub enum CallFailure {
    #[error("transport error: {0}")]
    Transport(#[from] TransportError),
    #[error("OCPP CallError {code:?}: {description}")]
    CallError {
        code: CallErrorCode,
        description: String,
        details: serde_json::Value,
    },
    #[error("response payload could not be parsed: {0}")]
    BadResponse(String),
}
