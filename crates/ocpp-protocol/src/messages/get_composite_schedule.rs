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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charging_rate_unit: Option<ChargingRateUnit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCompositeScheduleResponse {
    pub status: GetCompositeScheduleStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_id: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule_start: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charging_schedule: Option<ChargingSchedule>,
}

impl crate::action::OcppRequest for GetCompositeScheduleRequest {
    type Response = GetCompositeScheduleResponse;
    const ACTION: &'static str = "GetCompositeSchedule";
}
impl crate::action::OcppResponse for GetCompositeScheduleResponse {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_response_omits_absent_optional_fields() {
        let value = serde_json::to_value(GetCompositeScheduleResponse {
            status: GetCompositeScheduleStatus::Rejected,
            connector_id: None,
            schedule_start: None,
            charging_schedule: None,
        })
        .unwrap();

        assert_eq!(value, serde_json::json!({ "status": "Rejected" }));
    }
}
