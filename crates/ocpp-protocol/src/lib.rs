//! Pure OCPP protocol layer: frames, messages, enums.
//!
//! No IO. The transport layer is responsible for actually moving frames over
//! the wire; this crate only knows how to (de)serialize them.

pub mod action;
pub mod enums;
pub mod error;
pub mod frame;
pub mod messages;
pub mod version;

pub use action::{Action, OcppRequest, OcppResponse};
pub use error::{CallErrorCode, ProtocolError};
pub use frame::{Call, CallError, CallResult, Frame, MessageTypeId};
pub use version::{Ocpp16, OcppVersion};
