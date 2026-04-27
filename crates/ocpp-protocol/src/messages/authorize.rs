use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::action::{OcppRequest, OcppResponse};
use crate::enums::AuthorizationStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdTagInfo {
    pub status: AuthorizationStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_date: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id_tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizeRequest {
    pub id_tag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizeResponse {
    pub id_tag_info: IdTagInfo,
}

impl OcppRequest for AuthorizeRequest {
    type Response = AuthorizeResponse;
    const ACTION: &'static str = "Authorize";
}
impl OcppResponse for AuthorizeResponse {}
