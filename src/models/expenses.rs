/// Expense event types for ProjectionLab plans
use super::common::{DateOrMilestone, Owner, YearlyChange};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Container for expense events
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ExpenseContainer {
    pub events: Vec<ExpenseEvent>,
}

/// An expense event in a plan
/// Types include: living-expenses, rent, travel, vacation, wedding, etc.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseEvent {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub event_type: String,
    // Auto-generated events (e.g. "medicare") omit these spending-shape fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub icon: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<f64>,

    // Amount and timing
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_type: Option<String>, // "today$", "$", "%"
    pub owner: Owner,
    pub start: DateOrMilestone,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<DateOrMilestone>,

    // Frequency
    pub frequency: String,
    pub frequency_choices: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_interval: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_interval_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_scaler: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_end: Option<DateOrMilestone>,

    // Growth
    pub yearly_change: YearlyChange,

    // Metadata
    pub plan_path: String,
    pub spending_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fund_with_account: Option<bool>,

    /// Catch-all for any fields not explicitly defined above.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}
