use serde::{Deserialize, Serialize};

use crate::action::{OcppRequest, OcppResponse};
use crate::enums::ConfigurationStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeConfigurationRequest {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeConfigurationResponse {
    pub status: ConfigurationStatus,
}

impl OcppRequest for ChangeConfigurationRequest {
    type Response = ChangeConfigurationResponse;
    const ACTION: &'static str = "ChangeConfiguration";
}
impl OcppResponse for ChangeConfigurationResponse {}
