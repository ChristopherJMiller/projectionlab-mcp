/// Debt account types for ProjectionLab starting conditions
use super::common::Owner;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A debt account in starting conditions (e.g., mortgage, auto loan, student loan, credit card)
///
/// Uses `#[serde(flatten)]` to capture any fields not explicitly defined,
/// ensuring forward compatibility with new ProjectionLab schema versions.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DebtAccount {
    pub id: String,
    pub name: String,
    pub title: String,
    #[serde(rename = "type")]
    pub debt_type: String,
    pub icon: String,
    pub color: String,
    pub owner: Owner,

    // Balance and terms
    pub balance: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_balance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interest_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monthly_payment: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compounding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<String>,

    // Loan specifics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub term_months: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payoff_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquid: Option<bool>,

    // Metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<f64>,

    /// Catch-all for any fields not explicitly defined above.
    /// Ensures we never lose data during round-trip serialization.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}
