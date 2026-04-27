use serde::{Deserialize, Serialize};

/// CSMS → CP: query which version of the local authorization list the CP holds.
/// Per OCPP 1.6 §5.12.3, CP returns an integer version number.
/// A version of 0 means no local list is installed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GetLocalListVersionRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetLocalListVersionResponse {
    /// Current version of the local authorization list. 0 = no list.
    pub list_version: i32,
}

impl crate::action::OcppRequest for GetLocalListVersionRequest {
    type Response = GetLocalListVersionResponse;
    const ACTION: &'static str = "GetLocalListVersion";
}
impl crate::action::OcppResponse for GetLocalListVersionResponse {}
