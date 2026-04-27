use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// CSMS → CP: instruct the charge point to download and install firmware.
/// Per OCPP 1.6 §5.16, the CP returns an empty response immediately;
/// progress is reported via `FirmwareStatusNotification`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFirmwareRequest {
    /// URL of the firmware to download.
    pub location: String,
    /// Date/time after which the CP may start downloading.
    pub retrieve_date: DateTime<Utc>,
    /// Number of retry attempts (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries: Option<i32>,
    /// Interval in seconds between retry attempts (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_interval: Option<i32>,
}

/// Response to UpdateFirmware — always empty `{}` per spec.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UpdateFirmwareResponse {}

impl crate::action::OcppRequest for UpdateFirmwareRequest {
    type Response = UpdateFirmwareResponse;
    const ACTION: &'static str = "UpdateFirmware";
}
impl crate::action::OcppResponse for UpdateFirmwareResponse {}
