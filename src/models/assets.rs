/// Asset event types for ProjectionLab plans
use super::common::{AmountType, DateOrMilestone, Owner, YearlyChange};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Container for asset events
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AssetContainer {
    pub events: Vec<AssetEvent>,
}

/// An asset event in a plan
/// Types include: car, furniture, real-estate, etc.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssetEvent {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub title: String,
    pub icon: String,
    pub key: f64,

    // Basic asset properties
    pub owner: Owner,
    pub start: DateOrMilestone,
    pub end: DateOrMilestone,
    pub initial_value: f64,
    pub initial_value_type: AmountType,
    pub amount: f64,
    pub amount_type: AmountType,

    // Growth
    pub yearly_change: YearlyChange,

    // Repeat settings
    pub repeat: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_interval: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_interval_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_scaler: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_end: Option<DateOrMilestone>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_keep_last: Option<bool>,

    // Loan/financing fields
    pub balance: f64,
    pub balance_type: AmountType,
    pub down_payment: f64,
    pub down_payment_type: AmountType,
    pub monthly_payment: f64,
    pub monthly_payment_type: AmountType,
    pub interest_rate: f64,
    pub interest_type: String,
    pub compounding: String,
    pub payment_method: String,
    #[serde(rename = "excludeLoanFromLNW")]
    pub exclude_loan_from_lnw: bool,

    // Costs and fees
    pub brokers_fee: f64,
    pub tax_rate: f64,
    pub tax_rate_type: String, // Should be enum
    pub insurance_rate: f64,
    pub insurance_rate_type: String, // Should be enum
    pub maintenance_rate: f64,
    pub maintenance_rate_type: String, // Should be enum

    // Metadata
    pub plan_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sell_if_needed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fund_with_accounts: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_icon: Option<bool>,

    // Real-estate specific fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_building_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_building_value_type: Option<AmountType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification: Option<String>, // "residential" or "commercial"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generate_income: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent_rented: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub income_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub income_rate_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_passive_income: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel_rent: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub improvement_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub improvement_rate_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub management_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub management_rate_type: Option<String>,
    #[serde(rename = "monthlyHOA", skip_serializing_if = "Option::is_none")]
    pub monthly_hoa: Option<f64>,
    #[serde(rename = "estimateQBI", skip_serializing_if = "Option::is_none")]
    pub estimate_qbi: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_employment: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimate_rental_deductions: Option<bool>,

    /// Catch-all for any fields not explicitly defined above.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}
