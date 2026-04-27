use ocpp_protocol::CallErrorCode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("websocket error: {0}")]
    Ws(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("invalid url: {0}")]
    Url(#[from] url::ParseError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("tls config error: {0}")]
    Tls(String),
    #[error("protocol error: {0}")]
    Protocol(#[from] ocpp_protocol::ProtocolError),
    #[error("connection closed")]
    Closed,
    #[error("call timed out")]
    Timeout,
    #[error("not connected")]
    NotConnected,
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
