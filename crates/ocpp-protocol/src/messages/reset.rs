use serde::{Deserialize, Serialize};

use crate::action::{OcppRequest, OcppResponse};
use crate::enums::{ResetStatus, ResetType};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetRequest {
    #[serde(rename = "type")]
    pub reset_type: ResetType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetResponse {
    pub status: ResetStatus,
}

impl OcppRequest for ResetRequest {
    type Response = ResetResponse;
    const ACTION: &'static str = "Reset";
}
impl OcppResponse for ResetResponse {}
