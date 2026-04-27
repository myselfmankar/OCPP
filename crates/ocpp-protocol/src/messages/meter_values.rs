use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::action::{OcppRequest, OcppResponse};
use crate::enums::{Location, Measurand, Phase, ReadingContext, UnitOfMeasure, ValueFormat};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SampledValue {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<ReadingContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<ValueFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measurand: Option<Measurand>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<Phase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<Location>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<UnitOfMeasure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeterValue {
    pub timestamp: DateTime<Utc>,
    pub sampled_value: Vec<SampledValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeterValuesRequest {
    pub connector_id: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<i32>,
    pub meter_value: Vec<MeterValue>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MeterValuesResponse {}

impl OcppRequest for MeterValuesRequest {
    type Response = MeterValuesResponse;
    const ACTION: &'static str = "MeterValues";
}
impl OcppResponse for MeterValuesResponse {}
