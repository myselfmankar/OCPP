use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::action::{OcppRequest, OcppResponse};
use crate::enums::{ChargePointErrorCode, ChargePointStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusNotificationRequest {
    pub connector_id: i32,
    pub error_code: ChargePointErrorCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info: Option<String>,
    pub status: ChargePointStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vendor_error_code: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusNotificationResponse {}

impl OcppRequest for StatusNotificationRequest {
    type Response = StatusNotificationResponse;
    const ACTION: &'static str = "StatusNotification";
}
impl OcppResponse for StatusNotificationResponse {}
