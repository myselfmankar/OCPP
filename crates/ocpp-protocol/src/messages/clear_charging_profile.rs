use serde::{Deserialize, Serialize};

use crate::enums::{ChargingProfilePurpose, ClearChargingProfileStatus};

/// CSMS → CP: remove charging profiles matching the given filters (OCPP 1.6 §7.4).
/// All filter fields are optional; absent = match all.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearChargingProfileRequest {
    /// Profile ID to clear (none = all profiles).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// Connector to clear (0 = CP level, absent = any).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_id: Option<i32>,
    /// Purpose filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charging_profile_purpose: Option<ChargingProfilePurpose>,
    /// Stack-level filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack_level: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearChargingProfileResponse {
    pub status: ClearChargingProfileStatus,
}

impl crate::action::OcppRequest for ClearChargingProfileRequest {
    type Response = ClearChargingProfileResponse;
    const ACTION: &'static str = "ClearChargingProfile";
}
impl crate::action::OcppResponse for ClearChargingProfileResponse {}
