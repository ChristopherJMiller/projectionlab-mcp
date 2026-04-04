/// Milestone types for ProjectionLab plans
use super::common::AmountType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// User-defined milestone in a plan
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Milestone {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub color: String,
    pub criteria: Vec<MilestoneCriterion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removable: Option<bool>,
}

/// Criteria that must be met for a milestone to be reached
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MilestoneCriterion {
    #[serde(rename = "type")]
    pub criterion_type: String, // "year", "milestone", "account", "loan"
    pub value: Value, // Can be String or i64
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifier: Option<Value>, // Can be String enum or i64 offset
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>, // ">=", "<=", etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type: Option<AmountType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>, // Reference to account/asset
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logic: Option<String>, // "and" for multiple criteria
    #[serde(skip_serializing_if = "Option::is_none")]
    pub removable: Option<bool>,
}

/// Automatically computed milestone (e.g., from goals/priorities)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ComputedMilestone {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub color: String,
    pub goal_id: String,
    pub show_chart_icon: bool,
    pub criteria: Vec<ComputedMilestoneCriterion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ComputedMilestoneCriterion {
    pub ref_id: String,
    #[serde(rename = "type")]
    pub criterion_type: String,
    pub value: f64,
    pub value_type: String,
    pub operator: String,
}
