use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::action::{OcppRequest, OcppResponse};
use crate::enums::StopReason;
use crate::messages::authorize::IdTagInfo;
use crate::messages::meter_values::MeterValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopTransactionRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_tag: Option<String>,
    pub meter_stop: i32,
    pub timestamp: DateTime<Utc>,
    pub transaction_id: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<StopReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_data: Option<Vec<MeterValue>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StopTransactionResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_tag_info: Option<IdTagInfo>,
}

impl OcppRequest for StopTransactionRequest {
    type Response = StopTransactionResponse;
    const ACTION: &'static str = "StopTransaction";
}
impl OcppResponse for StopTransactionResponse {}
