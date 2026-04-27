use serde::{Deserialize, Serialize};

use crate::action::{OcppRequest, OcppResponse};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetConfigurationRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationKey {
    pub key: String,
    pub readonly: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GetConfigurationResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration_key: Option<Vec<ConfigurationKey>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unknown_key: Option<Vec<String>>,
}

impl OcppRequest for GetConfigurationRequest {
    type Response = GetConfigurationResponse;
    const ACTION: &'static str = "GetConfiguration";
}
impl OcppResponse for GetConfigurationResponse {}
