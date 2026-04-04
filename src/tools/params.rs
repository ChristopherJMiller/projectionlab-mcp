//! Parameter types for all MCP tool handlers.

use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};

// ---- Raw Plugin API tools ----

/// Parameters for updating an account via the raw Plugin API
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct UpdateAccountParams {
    /// The ID of the account to update
    pub account_id: String,
    /// The new data for the account as a JSON object
    pub data: JsonMap<String, JsonValue>,
    /// Allow assignment of new properties to account objects
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,
}

/// Parameters for restoring Current Finances
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct RestoreCurrentFinancesParams {
    /// The new Current Finances state as a JSON object
    pub new_state: JsonMap<String, JsonValue>,
}

/// Parameters for restoring Plans
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct RestorePlansParams {
    /// The new plans data as a JSON object
    pub new_plans: JsonMap<String, JsonValue>,
}

/// Parameters for restoring Progress
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct RestoreProgressParams {
    /// The new progress data as a JSON object
    pub new_progress: JsonMap<String, JsonValue>,
}

/// Parameters for restoring Settings
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct RestoreSettingsParams {
    /// The new settings data as a JSON object
    pub new_settings: JsonMap<String, JsonValue>,
}

// ---- Account tools ----

/// Parameters for listing accounts
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct AccountsListParams {
    /// Filter by account type (savings, investment, debt)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_type: Option<String>,
    /// Filter by owner
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

/// Parameters for getting account details
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct AccountsGetParams {
    /// The ID of the account to retrieve
    pub account_id: String,
}

/// Parameters for creating a new account
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct AccountsCreateParams {
    /// The type of account to create (savings, investment, debt)
    pub account_type: String,
    /// The account data as a JSON object
    pub data: JsonMap<String, JsonValue>,
}

/// Parameters for updating an account
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct AccountsUpdateParams {
    /// The ID of the account to update
    pub account_id: String,
    /// The updated account data as a JSON object (partial updates supported)
    pub data: JsonMap<String, JsonValue>,
}

/// Parameters for updating account balance
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct AccountsUpdateBalanceParams {
    /// The ID of the account to update
    pub account_id: String,
    /// The new balance value
    pub balance: f64,
}

/// Parameters for deleting an account
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct AccountsDeleteParams {
    /// The ID of the account to delete
    pub account_id: String,
}

// ---- Plan tools ----

/// Parameters for getting a plan by ID
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PlanGetParams {
    /// The ID of the plan to retrieve
    pub plan_id: String,
}

/// Parameters for updating plan variables
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PlansUpdateVariablesParams {
    /// The ID of the plan to update
    pub plan_id: String,
    /// The variable updates as a JSON object (partial updates supported)
    pub updates: JsonMap<String, JsonValue>,
}

/// Parameters for cloning a plan
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PlansCloneParams {
    /// The ID of the plan to clone
    pub source_plan_id: String,
    /// The name for the cloned plan
    pub new_name: String,
}

// ---- Event tools (expenses, income, priorities) ----

/// Parameters for listing events in a plan
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PlanEventsListParams {
    /// The ID of the plan
    pub plan_id: String,
}

/// Parameters for creating an event in a plan
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PlanEventCreateParams {
    /// The ID of the plan to add the event to
    pub plan_id: String,
    /// The event data as a JSON object. Must include required fields for the event type.
    pub data: JsonMap<String, JsonValue>,
}

/// Parameters for updating an event in a plan
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PlanEventUpdateParams {
    /// The ID of the plan containing the event
    pub plan_id: String,
    /// The ID of the event to update
    pub event_id: String,
    /// The updated fields as a JSON object (partial updates supported)
    pub data: JsonMap<String, JsonValue>,
}

/// Parameters for deleting an event from a plan
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PlanEventDeleteParams {
    /// The ID of the plan containing the event
    pub plan_id: String,
    /// The ID of the event to delete
    pub event_id: String,
}

// ---- Progress tools ----

/// Parameters for adding a progress data point
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ProgressAddDataPointParams {
    /// Unix timestamp in milliseconds for the data point
    pub date: i64,
    /// Total net worth
    pub net_worth: f64,
    /// Total savings account balances
    #[serde(default)]
    pub savings: f64,
    /// Total taxable account balances
    #[serde(default)]
    pub taxable: f64,
    /// Total tax-deferred account balances (401k, traditional IRA)
    #[serde(default)]
    pub tax_deferred: f64,
    /// Total tax-free account balances (Roth IRA, Roth 401k)
    #[serde(default)]
    pub tax_free: f64,
    /// Total asset values
    #[serde(default)]
    pub assets: f64,
    /// Total debt balances
    #[serde(default)]
    pub debt: f64,
    /// Total loan balances
    #[serde(default)]
    pub loans: f64,
    /// Total crypto balances
    #[serde(default)]
    pub crypto: f64,
}

/// Parameters for getting progress history
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ProgressGetHistoryParams {
    /// Optional start date filter (unix timestamp in milliseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<i64>,
    /// Optional end date filter (unix timestamp in milliseconds)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<i64>,
}

// ---- Integration tools ----

/// A single account balance mapping for batch sync
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct BalanceMapping {
    /// The ProjectionLab account ID
    pub pl_account_id: String,
    /// The new balance value
    pub balance: f64,
}

/// Parameters for batch-syncing account balances from external sources
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SyncAccountBalancesParams {
    /// List of account balance mappings to update
    pub mappings: Vec<BalanceMapping>,
}
