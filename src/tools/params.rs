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

/// Parameters for creating a new plan
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PlansCreateParams {
    /// Name for the new plan
    pub name: String,
    /// Icon identifier for the plan (e.g., "mdi-airplane", "mdi-home")
    #[serde(default = "default_plan_icon")]
    pub icon: String,
    /// Optional: clone from an existing plan ID instead of creating empty
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clone_from: Option<String>,
}

fn default_plan_icon() -> String {
    "mdi-file-document-outline".to_string()
}

/// Parameters for deleting a plan
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PlansDeleteParams {
    /// The ID of the plan to delete
    pub plan_id: String,
    /// Must be true to confirm deletion (safety check)
    pub confirm: bool,
}

// ---- Milestone tools ----

/// Parameters for creating a milestone in a plan
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MilestoneCreateParams {
    /// The ID of the plan to add the milestone to
    pub plan_id: String,
    /// The milestone data as a JSON object. Must include: name, icon, color, criteria.
    pub data: JsonMap<String, JsonValue>,
}

/// Parameters for updating a milestone in a plan
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MilestoneUpdateParams {
    /// The ID of the plan containing the milestone
    pub plan_id: String,
    /// The ID of the milestone to update
    pub milestone_id: String,
    /// The updated fields as a JSON object (partial updates supported)
    pub data: JsonMap<String, JsonValue>,
}

/// Parameters for deleting a milestone from a plan
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MilestoneDeleteParams {
    /// The ID of the plan containing the milestone
    pub plan_id: String,
    /// The ID of the milestone to delete
    pub milestone_id: String,
}

// ---- Plan metadata tools ----

/// Parameters for updating plan metadata (name, icon, active status)
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PlansUpdateMetadataParams {
    /// The ID of the plan to update
    pub plan_id: String,
    /// The metadata fields to update. Allowed keys: name, icon, active.
    pub updates: JsonMap<String, JsonValue>,
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

// ---- Browser / Simulation tools ----

/// Parameters for executing JavaScript in the browser
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct RunJsInBrowserParams {
    /// The JavaScript code to execute in the browser context. Use 'return' for sync scripts.
    pub script: String,
    /// If true, treat script as async (last argument is a callback). Default: false.
    #[serde(default)]
    pub r#async: bool,
    /// Optional: navigate to this path first (e.g., "/plan/abc123")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub navigate_to: Option<String>,
}

/// Parameters for getting a year snapshot from simulation results
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct YearSnapshotParams {
    /// The ID of the plan
    pub plan_id: String,
    /// The age to get the financial snapshot for
    pub age: i64,
}

/// Parameters for getting year range deltas from simulation results
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct YearRangeParams {
    /// The ID of the plan
    pub plan_id: String,
    /// Start age of the range
    pub start_age: i64,
    /// End age of the range
    pub end_age: i64,
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
