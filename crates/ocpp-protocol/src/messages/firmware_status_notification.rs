use serde::{Deserialize, Serialize};

use crate::enums::FirmwareStatus;

/// CP → CSMS: report current firmware update status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareStatusNotificationRequest {
    pub status: FirmwareStatus,
}

/// Response to FirmwareStatusNotification — always empty `{}` per spec.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FirmwareStatusNotificationResponse {}

impl crate::action::OcppRequest for FirmwareStatusNotificationRequest {
    type Response = FirmwareStatusNotificationResponse;
    const ACTION: &'static str = "FirmwareStatusNotification";
}
impl crate::action::OcppResponse for FirmwareStatusNotificationResponse {}
