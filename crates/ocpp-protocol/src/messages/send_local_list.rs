use serde::{Deserialize, Serialize};

use crate::enums::{UpdateStatus, UpdateType};
use crate::messages::authorize::IdTagInfo;

/// A single authorization entry in the local list (OCPP 1.6 §5.12).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizationData {
    pub id_tag: String,
    /// Present when adding/updating in a `Full` or `Differential` update.
    /// Absent when removing an entry in a `Differential` update.
    pub id_tag_info: Option<IdTagInfo>,
}

/// CSMS → CP: push a local authorization list to the charge point (OCPP 1.6 §5.12.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendLocalListRequest {
    /// Version number the CP should store after applying this list.
    pub list_version: i32,
    /// How to apply the list: replace entirely (`Full`) or merge (`Differential`).
    pub update_type: UpdateType,
    /// The list entries (may be empty for a Full update that clears all entries).
    pub local_authorization_list: Vec<AuthorizationData>,
}

/// Response to SendLocalList.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendLocalListResponse {
    pub status: UpdateStatus,
}

impl crate::action::OcppRequest for SendLocalListRequest {
    type Response = SendLocalListResponse;
    const ACTION: &'static str = "SendLocalList";
}
impl crate::action::OcppResponse for SendLocalListResponse {}
