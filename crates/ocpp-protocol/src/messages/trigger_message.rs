use serde::{Deserialize, Serialize};

use crate::action::{OcppRequest, OcppResponse};
use crate::enums::{MessageTrigger, TriggerMessageStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerMessageRequest {
    pub requested_message: MessageTrigger,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_id: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerMessageResponse {
    pub status: TriggerMessageStatus,
}

impl OcppRequest for TriggerMessageRequest {
    type Response = TriggerMessageResponse;
    const ACTION: &'static str = "TriggerMessage";
}
impl OcppResponse for TriggerMessageResponse {}
