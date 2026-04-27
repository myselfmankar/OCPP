use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// CSMS → CP: request the charge point to upload diagnostics to a given URL.
/// Per OCPP 1.6 §5.10, CP responds immediately with the filename it will upload
/// (empty string if it cannot produce diagnostics). Progress is then reported
/// asynchronously via `DiagnosticsStatusNotification`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetDiagnosticsRequest {
    /// URL the CP should upload the diagnostics file to (FTP/FTPS/HTTP/HTTPS).
    pub location: String,
    /// Number of retry attempts (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries: Option<i32>,
    /// Interval in seconds between retries (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_interval: Option<i32>,
    /// Only include diagnostics from this timestamp onwards (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<DateTime<Utc>>,
    /// Only include diagnostics up to this timestamp (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_time: Option<DateTime<Utc>>,
}

/// Response to GetDiagnostics.
/// `file_name` is the name of the diagnostics file the CP will upload,
/// or absent/empty if no diagnostics are available.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetDiagnosticsResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
}

impl crate::action::OcppRequest for GetDiagnosticsRequest {
    type Response = GetDiagnosticsResponse;
    const ACTION: &'static str = "GetDiagnostics";
}
impl crate::action::OcppResponse for GetDiagnosticsResponse {}
