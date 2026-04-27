use serde::{Deserialize, Serialize};

use crate::action::{OcppRequest, OcppResponse};
use crate::enums::DataTransferStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataTransferRequest {
    pub vendor_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataTransferResponse {
    pub status: DataTransferStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

impl OcppRequest for DataTransferRequest {
    type Response = DataTransferResponse;
    const ACTION: &'static str = "DataTransfer";
}
impl OcppResponse for DataTransferResponse {}
