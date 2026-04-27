use serde::{Deserialize, Serialize};

use crate::enums::DiagnosticsStatus;

/// CP → CSMS: report current diagnostics upload status.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsStatusNotificationRequest {
    pub status: DiagnosticsStatus,
}

/// Response to DiagnosticsStatusNotification — always empty `{}` per spec.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiagnosticsStatusNotificationResponse {}

impl crate::action::OcppRequest for DiagnosticsStatusNotificationRequest {
    type Response = DiagnosticsStatusNotificationResponse;
    const ACTION: &'static str = "DiagnosticsStatusNotification";
}
impl crate::action::OcppResponse for DiagnosticsStatusNotificationResponse {}
