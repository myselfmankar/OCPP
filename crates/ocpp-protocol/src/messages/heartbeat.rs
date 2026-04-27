use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::action::{OcppRequest, OcppResponse};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeartbeatRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeartbeatResponse {
    pub current_time: DateTime<Utc>,
}

impl OcppRequest for HeartbeatRequest {
    type Response = HeartbeatResponse;
    const ACTION: &'static str = "Heartbeat";
}
impl OcppResponse for HeartbeatResponse {}
