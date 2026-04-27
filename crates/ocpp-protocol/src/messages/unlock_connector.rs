use serde::{Deserialize, Serialize};

use crate::action::{OcppRequest, OcppResponse};
use crate::enums::UnlockStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlockConnectorRequest {
    pub connector_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlockConnectorResponse {
    pub status: UnlockStatus,
}

impl OcppRequest for UnlockConnectorRequest {
    type Response = UnlockConnectorResponse;
    const ACTION: &'static str = "UnlockConnector";
}
impl OcppResponse for UnlockConnectorResponse {}
