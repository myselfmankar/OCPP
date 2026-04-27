use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::enums::{
    ChargingProfileKind, ChargingProfilePurpose, ChargingProfileStatus,
    ChargingRateUnit, RecurrencyKind,
};

/// A single time-interval entry in a charging schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChargingSchedulePeriod {
    /// Seconds from the start of the schedule.
    pub start_period: i32,
    /// Power/current limit for this period.
    pub limit: f64,
    /// Optional number of phases (1 or 3).
    pub number_phases: Option<i32>,
}

/// A time-based schedule defining power/current limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChargingSchedule {
    /// Optional total duration (seconds). Absent = unlimited.
    pub duration: Option<i32>,
    /// Start of the schedule (absolute). Required for Absolute profiles.
    pub start_schedule: Option<DateTime<Utc>>,
    pub charging_rate_unit: ChargingRateUnit,
    pub charging_schedule_period: Vec<ChargingSchedulePeriod>,
    /// Minimum charging rate (A or W). Optional.
    pub min_charging_rate: Option<f64>,
}

/// A charging profile as defined in OCPP 1.6 §7.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChargingProfile {
    pub charging_profile_id: i32,
    pub transaction_id: Option<i32>,
    pub stack_level: i32,
    pub charging_profile_purpose: ChargingProfilePurpose,
    pub charging_profile_kind: ChargingProfileKind,
    pub recurrency_kind: Option<RecurrencyKind>,
    pub valid_from: Option<DateTime<Utc>>,
    pub valid_to: Option<DateTime<Utc>>,
    pub charging_schedule: ChargingSchedule,
}

/// CSMS → CP: install a charging profile on a connector (OCPP 1.6 §7.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetChargingProfileRequest {
    /// 0 = charge point as a whole, >0 = specific connector.
    pub connector_id: i32,
    pub cs_charging_profiles: ChargingProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetChargingProfileResponse {
    pub status: ChargingProfileStatus,
}

impl crate::action::OcppRequest for SetChargingProfileRequest {
    type Response = SetChargingProfileResponse;
    const ACTION: &'static str = "SetChargingProfile";
}
impl crate::action::OcppResponse for SetChargingProfileResponse {}
