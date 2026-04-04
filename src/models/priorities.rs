/// Priority/Goal event types for ProjectionLab plans
use super::common::{AmountType, DateOrMilestone, Owner};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Container for priority events
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PriorityContainer {
    pub events: Vec<PriorityEvent>,
}

/// A priority/goal event in a plan
/// Types include: 401k, savings, taxable, etc.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PriorityEvent {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub title: String,
    pub subtitle: String,
    pub icon: String,
    pub color: String,
    pub key: f64,

    // Goal settings
    pub goal_intent: String,
    pub owner: Owner,
    pub account_id: String,
    pub start: DateOrMilestone,
    pub end: DateOrMilestone,
    pub persistent: bool,

    // Amount settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_type: Option<AmountType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency: Option<String>,

    // Metadata
    pub plan_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_chart_icon: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_msg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desired_contribution: Option<String>,

    // 401k-specific fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contribution: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contribution_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contribution_limit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contributions_are_fixed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub employer_match: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub employer_match_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub employer_match_limit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reduce_employer_match: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub income_stream_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yearly_limit: Option<f64>,
    #[serde(rename = "yearlyLimit$Type", skip_serializing_if = "Option::is_none")]
    pub yearly_limit_dollar_type: Option<AmountType>,
    #[serde(rename = "yearlyLimitType", skip_serializing_if = "Option::is_none")]
    pub yearly_limit_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,

    // Mode settings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tap_fund: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tap_rate: Option<f64>,
}
