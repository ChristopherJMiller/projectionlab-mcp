/// Common types shared across ProjectionLab data models
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Represents a date or milestone reference
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DateOrMilestone {
    #[serde(rename = "type")]
    pub date_type: DateType,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modifier: Option<String>, // Can be "include", "exclude", or a number offset
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DateType {
    Keyword,  // Values like "beforeCurrentYear", "endOfPlan"
    Date,     // ISO date string like "2027-01-01"
    Milestone, // UUID reference to a milestone
}

/// Yearly change pattern for amounts (growth, depreciation, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct YearlyChange {
    #[serde(rename = "type")]
    pub change_type: String, // "none", "match-inflation", "inflation+", "custom", "depreciate", "appreciate"
    pub amount: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_type: Option<AmountType>,
    pub limit: f64,
    pub limit_enabled: bool,
    pub limit_type: AmountType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom: Option<CustomYearlyChange>,
}

/// Custom yearly change data with year-by-year values
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CustomYearlyChange {
    #[serde(rename = "type")]
    pub custom_type: String, // e.g. "inflation+%"
    pub data: Vec<YearlyDataPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct YearlyDataPoint {
    pub x: i32, // Year
    pub y: f64, // Value
}

/// Amount type denomination
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum AmountType {
    #[serde(rename = "today$")]
    TodayDollars,
    #[serde(rename = "$")]
    Dollars,
    #[serde(rename = "%")]
    Percent,
}

/// Account/Asset owner
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Owner {
    Me,
    Spouse,
    Joint,
}

/// Frequency of recurring events
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Frequency {
    Monthly,
    Yearly,
    Once,
    Biweekly,
    #[serde(rename = "semi-monthly")]
    SemiMonthly,
    Quarterly,
    #[serde(rename = "semi-annually")]
    SemiAnnually,
}

/// Growth/assumption mode
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum AssumptionsMode {
    Fixed,
    Plan,
}

/// Bond allocation type
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum BondAllocationType {
    Plan,
    None,
}
