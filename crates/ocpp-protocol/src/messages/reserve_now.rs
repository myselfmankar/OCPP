use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::enums::ReservationStatus;

/// CSMS → CP: reserve a connector for a specific idTag (OCPP 1.6 §5.13.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReserveNowRequest {
    /// 0 = any available connector, >0 = specific connector.
    pub connector_id: i32,
    /// Date/time when reservation expires.
    pub expiry_date: DateTime<Utc>,
    /// The idTag for which the connector is reserved.
    pub id_tag: String,
    /// Optional parent idTag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id_tag: Option<String>,
    /// Unique reservation ID.
    pub reservation_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReserveNowResponse {
    pub status: ReservationStatus,
}

impl crate::action::OcppRequest for ReserveNowRequest {
    type Response = ReserveNowResponse;
    const ACTION: &'static str = "ReserveNow";
}
impl crate::action::OcppResponse for ReserveNowResponse {}
