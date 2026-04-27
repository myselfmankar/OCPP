use serde::{Deserialize, Serialize};

use crate::enums::ClearCacheStatus;

/// CSMS → CP: request the charge point to clear its local authorization cache.
/// Per OCPP 1.6 §5.4, CP returns Accepted if it supports the cache and cleared it,
/// or Rejected otherwise.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClearCacheRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearCacheResponse {
    pub status: ClearCacheStatus,
}

impl crate::action::OcppRequest for ClearCacheRequest {
    type Response = ClearCacheResponse;
    const ACTION: &'static str = "ClearCache";
}
impl crate::action::OcppResponse for ClearCacheResponse {}
