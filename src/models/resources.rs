/// MCP Resource response types for ProjectionLab
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct OverviewResource {
    pub resource_type: String,
    pub generated_at: String,
    pub total_accounts: usize,
    pub total_net_worth: f64,
    pub total_savings: f64,
    pub total_investment: f64,
    pub total_debt: f64,
    pub active_plans: usize,
    pub last_updated: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AccountsSummaryResource {
    pub resource_type: String,
    pub generated_at: String,
    pub total_accounts: usize,
    pub total_balance: f64,
    pub accounts_by_type: HashMap<String, usize>,
    pub accounts: Vec<AccountSummary>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AccountSummary {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub balance: f64,
    pub owner: String,
    pub uri: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AccountDetailsResource {
    pub resource_type: String,
    pub generated_at: String,
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub balance: f64,
    pub owner: String,
    pub liquid: bool,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub cost_basis: Option<f64>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ExpensesSummaryResource {
    pub resource_type: String,
    pub generated_at: String,
    pub total_expenses: usize,
    pub expenses_by_category: HashMap<String, usize>,
    pub expenses: Vec<ExpenseSummary>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ExpenseSummary {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub expense_type: String,
    pub amount: f64,
    pub frequency: String,
    pub uri: String,
}
