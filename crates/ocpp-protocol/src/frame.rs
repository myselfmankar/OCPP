use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_json::Value;

use crate::error::{CallErrorCode, ProtocolError};

/// OCPP-J `MessageTypeId` (first element of every frame).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum MessageTypeId {
    Call = 2,
    CallResult = 3,
    CallError = 4,
}

/// `[2, "<uniqueId>", "<Action>", {payload}]`
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub unique_id: String,
    pub action: String,
    pub payload: Value,
}

/// `[3, "<uniqueId>", {payload}]`
#[derive(Debug, Clone, PartialEq)]
pub struct CallResult {
    pub unique_id: String,
    pub payload: Value,
}

/// `[4, "<uniqueId>", "<errorCode>", "<errorDescription>", {errorDetails}]`
#[derive(Debug, Clone, PartialEq)]
pub struct CallError {
    pub unique_id: String,
    pub error_code: CallErrorCode,
    pub error_description: String,
    pub error_details: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    Call(Call),
    Result(CallResult),
    Error(CallError),
}

impl Frame {
    pub fn unique_id(&self) -> &str {
        match self {
            Frame::Call(c) => &c.unique_id,
            Frame::Result(r) => &r.unique_id,
            Frame::Error(e) => &e.unique_id,
        }
    }

    pub fn from_text(text: &str) -> Result<Self, ProtocolError> {
        let v: Value = serde_json::from_str(text)?;
        Self::from_json(v)
    }

    pub fn to_text(&self) -> Result<String, ProtocolError> {
        Ok(serde_json::to_string(&self.to_json())?)
    }

    pub fn from_json(v: Value) -> Result<Self, ProtocolError> {
        let arr = v.as_array().ok_or(ProtocolError::NotAnArray)?;
        if arr.is_empty() {
            return Err(ProtocolError::EmptyFrame);
        }
        let mtid = arr[0]
            .as_u64()
            .ok_or(ProtocolError::BadMessageTypeId)? as u8;

        match mtid {
            2 => {
                if arr.len() != 4 {
                    return Err(ProtocolError::BadFrameShape("Call must have 4 elements"));
                }
                let unique_id = arr[1].as_str().ok_or(ProtocolError::BadUniqueId)?.to_string();
                let action = arr[2].as_str().ok_or(ProtocolError::BadAction)?.to_string();
                let payload = arr[3].clone();
                Ok(Frame::Call(Call { unique_id, action, payload }))
            }
            3 => {
                if arr.len() != 3 {
                    return Err(ProtocolError::BadFrameShape("CallResult must have 3 elements"));
                }
                let unique_id = arr[1].as_str().ok_or(ProtocolError::BadUniqueId)?.to_string();
                let payload = arr[2].clone();
                Ok(Frame::Result(CallResult { unique_id, payload }))
            }
            4 => {
                if arr.len() != 5 {
                    return Err(ProtocolError::BadFrameShape("CallError must have 5 elements"));
                }
                let unique_id = arr[1].as_str().ok_or(ProtocolError::BadUniqueId)?.to_string();
                let code_str = arr[2].as_str().ok_or(ProtocolError::BadAction)?;
                let error_code = code_str.parse::<CallErrorCode>()
                    .unwrap_or(CallErrorCode::GenericError);
                let error_description = arr[3].as_str().unwrap_or("").to_string();
                let error_details = arr[4].clone();
                Ok(Frame::Error(CallError {
                    unique_id,
                    error_code,
                    error_description,
                    error_details,
                }))
            }
            _ => Err(ProtocolError::BadMessageTypeId),
        }
    }

    pub fn to_json(&self) -> Value {
        match self {
            Frame::Call(c) => Value::Array(vec![
                Value::from(MessageTypeId::Call as u8),
                Value::String(c.unique_id.clone()),
                Value::String(c.action.clone()),
                c.payload.clone(),
            ]),
            Frame::Result(r) => Value::Array(vec![
                Value::from(MessageTypeId::CallResult as u8),
                Value::String(r.unique_id.clone()),
                r.payload.clone(),
            ]),
            Frame::Error(e) => Value::Array(vec![
                Value::from(MessageTypeId::CallError as u8),
                Value::String(e.unique_id.clone()),
                Value::String(e.error_code.as_str().to_string()),
                Value::String(e.error_description.clone()),
                e.error_details.clone(),
            ]),
        }
    }
}

impl Serialize for Frame {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        self.to_json().serialize(ser)
    }
}

impl<'de> Deserialize<'de> for Frame {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let v = Value::deserialize(de)?;
        Frame::from_json(v).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_call() {
        let raw = r#"[2,"abc","Heartbeat",{}]"#;
        let f = Frame::from_text(raw).unwrap();
        assert_eq!(f.unique_id(), "abc");
        assert_eq!(f.to_text().unwrap(), raw);
    }

    #[test]
    fn round_trip_call_result() {
        let raw = r#"[3,"abc",{"currentTime":"2024-01-01T00:00:00Z"}]"#;
        let f = Frame::from_text(raw).unwrap();
        assert!(matches!(f, Frame::Result(_)));
        assert_eq!(f.to_text().unwrap(), raw);
    }

    #[test]
    fn round_trip_call_error() {
        let raw = r#"[4,"abc","NotImplemented","nope",{}]"#;
        let f = Frame::from_text(raw).unwrap();
        assert!(matches!(f, Frame::Error(_)));
        assert_eq!(f.to_text().unwrap(), raw);
    }
}
