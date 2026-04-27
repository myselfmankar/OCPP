//! OCPP transport layer.
//!
//! Wraps a WebSocket connection to a CSMS and provides a typed `Session`
//! API to send `OcppRequest`s, correlate `CALLRESULT`/`CALLERROR` replies,
//! and dispatch incoming `CALL`s to a `CsmsHandler`.

pub mod backoff;
pub mod correlator;
pub mod dispatcher;
pub mod error;
pub mod session;
pub mod tls;
pub mod ws_client;

pub use dispatcher::CsmsHandler;
pub use error::{CallFailure, TransportError};
pub use session::{Session, SessionConfig};
pub use tls::SecurityProfile;
