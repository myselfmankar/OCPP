use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::enums::{ChargingRateUnit, GetCompositeScheduleStatus};
use crate::messages::set_charging_profile::ChargingSchedule;

/// CSMS → CP: request the composite charging schedule for a connector (OCPP 1.6 §7.5).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCompositeScheduleRequest {
    pub connector_id: i32,
    /// Duration (seconds) of the schedule to compute.
    pub duration: i32,
    /// Preferred rate unit for the response (optional).
    pub charging_rate_unit: Option<ChargingRateUnit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCompositeScheduleResponse {
    pub status: GetCompositeScheduleStatus,
    pub connector_id: Option<i32>,
    pub schedule_start: Option<DateTime<Utc>>,
    pub charging_schedule: Option<ChargingSchedule>,
}

impl crate::action::OcppRequest for GetCompositeScheduleRequest {
    type Response = GetCompositeScheduleResponse;
    const ACTION: &'static str = "GetCompositeSchedule";
}
impl crate::action::OcppResponse for GetCompositeScheduleResponse {}
