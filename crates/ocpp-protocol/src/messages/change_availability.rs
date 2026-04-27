use serde::{Deserialize, Serialize};

use crate::enums::{AvailabilityStatus, AvailabilityType};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeAvailabilityRequest {
    pub connector_id: i32,
    pub r#type: AvailabilityType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeAvailabilityResponse {
    pub status: AvailabilityStatus,
}

impl crate::action::OcppRequest for ChangeAvailabilityRequest {
    type Response = ChangeAvailabilityResponse;
    const ACTION: &'static str = "ChangeAvailability";
}
impl crate::action::OcppResponse for ChangeAvailabilityResponse {}
