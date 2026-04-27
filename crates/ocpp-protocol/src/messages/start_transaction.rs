use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::action::{OcppRequest, OcppResponse};
use crate::messages::authorize::IdTagInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTransactionRequest {
    pub connector_id: i32,
    pub id_tag: String,
    pub meter_start: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reservation_id: Option<i32>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTransactionResponse {
    pub id_tag_info: IdTagInfo,
    pub transaction_id: i32,
}

impl OcppRequest for StartTransactionRequest {
    type Response = StartTransactionResponse;
    const ACTION: &'static str = "StartTransaction";
}
impl OcppResponse for StartTransactionResponse {}
