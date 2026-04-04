/// Income event types for ProjectionLab plans
use super::common::{AmountType, DateOrMilestone, Owner, YearlyChange};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Container for income events
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct IncomeContainer {
    pub events: Vec<IncomeEvent>,
}

/// An income event in a plan
/// Types include: salary, rsu, other
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IncomeEvent {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub title: String,
    pub icon: String,
    pub key: f64,

    // Amount and timing
    pub amount: f64,
    pub amount_type: AmountType,
    pub owner: Owner,
    pub start: DateOrMilestone,
    pub end: DateOrMilestone,

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

    // Tax settings
    pub tax_exempt: bool,
    pub tax_withholding: bool,
    pub withhold: f64,

    // Metadata
    pub plan_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_to_accounts: Option<Vec<String>>,

    // Pension fields (for salary type)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_pension: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pension_contribution: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pension_contribution_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contribs_reduce_taxable_income: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pension_payouts_start: Option<DateOrMilestone>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pension_payouts_end: Option<DateOrMilestone>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pension_payout_amount: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pension_payout_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pension_payout_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pension_payouts_are_tax_free: Option<bool>,

    // Part-time fields (for salary type)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub go_part_time: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_time_start: Option<DateOrMilestone>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_time_end: Option<DateOrMilestone>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub part_time_rate: Option<f64>,

    // Income type flags
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_dividend_income: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_passive_income: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_employment: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wage: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prevent_overflow: Option<bool>,
}
