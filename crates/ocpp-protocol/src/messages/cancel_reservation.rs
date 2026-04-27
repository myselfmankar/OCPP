use serde::{Deserialize, Serialize};

use crate::enums::CancelReservationStatus;

/// CSMS → CP: cancel an existing reservation (OCPP 1.6 §5.13.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelReservationRequest {
    pub reservation_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelReservationResponse {
    pub status: CancelReservationStatus,
}

impl crate::action::OcppRequest for CancelReservationRequest {
    type Response = CancelReservationResponse;
    const ACTION: &'static str = "CancelReservation";
}
impl crate::action::OcppResponse for CancelReservationResponse {}
