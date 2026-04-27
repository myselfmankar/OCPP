use serde::{Deserialize, Serialize};

use crate::action::{OcppRequest, OcppResponse};
use crate::enums::RemoteStartStopStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStopTransactionRequest {
    pub transaction_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStopTransactionResponse {
    pub status: RemoteStartStopStatus,
}

impl OcppRequest for RemoteStopTransactionRequest {
    type Response = RemoteStopTransactionResponse;
    const ACTION: &'static str = "RemoteStopTransaction";
}
impl OcppResponse for RemoteStopTransactionResponse {}
