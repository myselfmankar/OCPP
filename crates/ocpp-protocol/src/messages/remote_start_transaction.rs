use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::action::{OcppRequest, OcppResponse};
use crate::enums::RemoteStartStopStatus;

/// `chargingProfile` is included verbatim — the smart-charging types live
/// outside v1 scope, but the field must round-trip cleanly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStartTransactionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_id: Option<i32>,
    pub id_tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charging_profile: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStartTransactionResponse {
    pub status: RemoteStartStopStatus,
}

impl OcppRequest for RemoteStartTransactionRequest {
    type Response = RemoteStartTransactionResponse;
    const ACTION: &'static str = "RemoteStartTransaction";
}
impl OcppResponse for RemoteStartTransactionResponse {}
