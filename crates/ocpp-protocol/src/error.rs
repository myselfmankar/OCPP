use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("frame is not a JSON array")]
    NotAnArray,
    #[error("frame is empty")]
    EmptyFrame,
    #[error("unknown MessageTypeId")]
    BadMessageTypeId,
    #[error("missing or invalid uniqueId")]
    BadUniqueId,
    #[error("missing or invalid action / errorCode")]
    BadAction,
    #[error("malformed frame: {0}")]
    BadFrameShape(&'static str),
    #[error("unknown action: {0}")]
    UnknownAction(String),
    #[error("payload (de)serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

/// OCPP-J standard error codes (section 4.3 of OCPP-J 1.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallErrorCode {
    NotImplemented,
    NotSupported,
    InternalError,
    ProtocolError,
    SecurityError,
    FormationViolation,
    PropertyConstraintViolation,
    OccurenceConstraintViolation,
    TypeConstraintViolation,
    GenericError,
}

impl CallErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            CallErrorCode::NotImplemented => "NotImplemented",
            CallErrorCode::NotSupported => "NotSupported",
            CallErrorCode::InternalError => "InternalError",
            CallErrorCode::ProtocolError => "ProtocolError",
            CallErrorCode::SecurityError => "SecurityError",
            CallErrorCode::FormationViolation => "FormationViolation",
            CallErrorCode::PropertyConstraintViolation => "PropertyConstraintViolation",
            CallErrorCode::OccurenceConstraintViolation => "OccurenceConstraintViolation",
            CallErrorCode::TypeConstraintViolation => "TypeConstraintViolation",
            CallErrorCode::GenericError => "GenericError",
        }
    }
}

impl FromStr for CallErrorCode {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, ()> {
        Ok(match s {
            "NotImplemented" => Self::NotImplemented,
            "NotSupported" => Self::NotSupported,
            "InternalError" => Self::InternalError,
            "ProtocolError" => Self::ProtocolError,
            "SecurityError" => Self::SecurityError,
            "FormationViolation" => Self::FormationViolation,
            "PropertyConstraintViolation" => Self::PropertyConstraintViolation,
            "OccurenceConstraintViolation" => Self::OccurenceConstraintViolation,
            "TypeConstraintViolation" => Self::TypeConstraintViolation,
            "GenericError" => Self::GenericError,
            _ => return Err(()),
        })
    }
}
