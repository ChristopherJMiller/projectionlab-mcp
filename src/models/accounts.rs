/// Account event types for ProjectionLab plans
use super::common::{AssumptionsMode, BondAllocationType, DateOrMilestone, Owner};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Container for account events
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AccountContainer {
    pub events: Vec<AccountEvent>,
}

/// An account event in a plan
/// Types include: savings, taxable, 401k, roth-ira, etc.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountEvent {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub title: String,
    pub icon: String,
    pub color: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<f64>,

    // Basic account properties
    pub owner: Owner,
    pub balance: f64,
    pub liquid: bool,
    pub persistent: Option<bool>,
    pub withdraw: bool,
    pub withdraw_age: DateOrMilestone,

    // Investment settings
    pub investment_growth_rate: f64,
    pub investment_growth_type: AssumptionsMode,
    pub dividend_rate: f64,
    pub dividend_type: BondAllocationType,
    pub is_passive_income: bool,

    // Metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repurpose: Option<bool>,

    // Taxable account fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_basis: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yearly_fee: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yearly_fee_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dividend_reinvestment: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dividend_tax_type: Option<AssumptionsMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dividends_are_passive_income: Option<bool>,

    // Retirement account fields (401k, Roth IRA, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(rename = "EWAge", skip_serializing_if = "Option::is_none")]
    pub ew_age: Option<i64>,
    #[serde(rename = "hasEWPenalty", skip_serializing_if = "Option::is_none")]
    pub has_ew_penalty: Option<bool>,
    #[serde(rename = "EWPenaltyRate", skip_serializing_if = "Option::is_none")]
    pub ew_penalty_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rmd_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withdraw_contribs_free: Option<bool>,
}
