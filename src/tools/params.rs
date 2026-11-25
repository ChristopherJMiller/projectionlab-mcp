//! Parameter types for all MCP tool handlers.

use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue};

// ---- Raw Plugin API tools ----

/// Parameters for `updateAccount` via the raw Plugin API
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct UpdateAccountParams {
    /// The ID of the Current Finances account to update
    pub account_id: String,
    /// Fields to assign on the account (camelCase ProjectionLab field names)
    pub data: JsonMap<String, JsonValue>,
    /// Allow assigning fields that don't already exist on the account
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force: Option<bool>,
}

/// Parameters for `updateAccountValues` via the raw Plugin API
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct UpdateAccountValuesParams {
    /// The ID of the Current Finances account to update
    pub account_id: String,
    /// Values to set. Supported: `balance` (accounts, debts), `costBasis` (taxable-style
    /// accounts), `value` and `purchasePrice` (real assets). Must be finite numbers.
    pub values: JsonMap<String, JsonValue>,
}

/// Parameters for restoring Current Finances
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct RestoreCurrentFinancesParams {
    /// The Current Finances document (`{ accounts: {<id>: {...}}, household, location, ... }`).
    /// Submitted accounts are merged over existing ones; omitted accounts are kept.
    pub new_state: JsonMap<String, JsonValue>,
}

/// Parameters for restoring Plans
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct RestorePlansParams {
    /// The complete array of plans. Any existing plan not in this array is DELETED.
    pub new_plans: Vec<JsonValue>,
}

/// Parameters for restoring Settings
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct RestoreSettingsParams {
    /// The new settings document as a JSON object
    pub new_settings: JsonMap<String, JsonValue>,
}

// ---- Current Finances account tools ----

/// Parameters for listing Current Finances accounts
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct AccountsListParams {
    /// Filter by kind: "account", "debt", or "real-asset"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Filter by type (e.g. "savings", "taxable", "401k", "mortgage", "real-estate"),
    /// or "investment" for all non-savings accounts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_type: Option<String>,
    /// Filter by owner ("me", "spouse", "joint")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Include archived accounts (default false)
    #[serde(default)]
    pub include_archived: bool,
}

/// Parameters for getting account details
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct AccountsGetParams {
    /// The ID of the account to retrieve
    pub account_id: String,
}

/// Parameters for creating a Current Finances account, debt, or real asset
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct AccountsCreateParams {
    /// The ProjectionLab type, e.g. "savings", "taxable", "401k", "roth-ira", "hsa",
    /// "mortgage", "auto-loan", "student-loans", "real-estate", "car".
    pub account_type: String,
    /// Optional kind ("account", "debt", "real-asset"); inferred from the type if omitted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Account fields (camelCase), e.g. {"name": "Brokerage", "owner": "me", "balance": 1000}.
    /// Real assets use "value" (and optionally "purchasePrice") instead of "balance".
    pub data: JsonMap<String, JsonValue>,
}

/// Parameters for updating an account
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct AccountsUpdateParams {
    /// The ID of the account to update
    pub account_id: String,
    /// Fields to update (camelCase, partial). Value fields (balance, costBasis, value,
    /// purchasePrice) are applied via updateAccountValues so history is recorded.
    pub data: JsonMap<String, JsonValue>,
}

/// Parameters for updating an account's balance
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct AccountsUpdateBalanceParams {
    /// The ID of the account to update
    pub account_id: String,
    /// The new balance (or value, for real assets)
    pub balance: f64,
}

/// Parameters for archiving an account
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct AccountsArchiveParams {
    /// The ID of the account to archive
    pub account_id: String,
    /// Archive date (ISO YYYY-MM-DD). Defaults to today.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

/// Parameters for unarchiving an account
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct AccountsUnarchiveParams {
    /// The ID of the archived account to restore
    pub account_id: String,
}

// ---- History tools ----

/// Parameters for reading per-account value history
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct AccountHistoryParams {
    /// Restrict to one account. If omitted, returns history for all accounts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    /// Inclusive start date (ISO YYYY-MM-DD)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// Inclusive end date (ISO YYYY-MM-DD)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
    /// Include archived accounts when no account_id is given (default false)
    #[serde(default)]
    pub include_archived: bool,
}

/// Parameters for getting net worth history
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct NetWorthHistoryParams {
    /// Inclusive start date (ISO YYYY-MM-DD)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_date: Option<String>,
    /// Inclusive end date (ISO YYYY-MM-DD)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_date: Option<String>,
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

/// Parameters for updating plan metadata (name, icon)
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PlansUpdateMetadataParams {
    /// The ID of the plan to update
    pub plan_id: String,
    /// The metadata fields to update. Allowed keys: name, icon.
    pub updates: JsonMap<String, JsonValue>,
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

// ---- Plan event tools (expenses, income, priorities, assets, plan accounts) ----

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
    /// The event data (camelCase). `id` and `kind` are filled in if omitted.
    /// Tip: copy an existing event of the same type as a template.
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

// ---- Real asset tools (Current Finances, kind "real-asset") ----

/// Parameters for listing real assets
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct RealAssetsListParams {
    /// Filter by asset type (e.g., "real-estate", "car", "other")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset_type: Option<String>,
    /// Filter by owner
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Include archived assets (default false)
    #[serde(default)]
    pub include_archived: bool,
}

/// Parameters for getting a real asset by ID
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct RealAssetsGetParams {
    /// The ID of the asset to retrieve
    pub asset_id: String,
}

/// Parameters for creating a real asset
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct RealAssetsCreateParams {
    /// Asset fields (camelCase). Must include `type` (e.g. "real-estate", "car") and `name`;
    /// typically `value` and `purchasePrice`.
    pub data: JsonMap<String, JsonValue>,
}

/// Parameters for updating a real asset
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct RealAssetsUpdateParams {
    /// The ID of the asset to update
    pub asset_id: String,
    /// The updated fields as a JSON object (partial updates supported)
    pub data: JsonMap<String, JsonValue>,
}

// ---- Debt tools (Current Finances, kind "debt") ----

/// Parameters for listing debts
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DebtsListParams {
    /// Filter by debt type (e.g., "mortgage", "auto-loan", "student-loans", "credit-card")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debt_type: Option<String>,
    /// Filter by owner
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// Include archived debts (default false)
    #[serde(default)]
    pub include_archived: bool,
}

/// Parameters for getting a debt by ID
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DebtsGetParams {
    /// The ID of the debt to retrieve
    pub debt_id: String,
}

/// Parameters for creating a new debt
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DebtsCreateParams {
    /// Debt fields (camelCase). Must include `type` (e.g. "mortgage", "student-loans") and
    /// `name`; typically `balance`, `interestRate`, `monthlyPayment`, `owner`.
    pub data: JsonMap<String, JsonValue>,
}

/// Parameters for updating a debt
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct DebtsUpdateParams {
    /// The ID of the debt to update
    pub debt_id: String,
    /// The updated fields as a JSON object (partial updates supported)
    pub data: JsonMap<String, JsonValue>,
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
    /// Optional: navigate to this path first (e.g., "/plan/overview", "/net-worth/history")
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

// ---- Integration tools ----

/// A single account balance mapping for batch sync
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct BalanceMapping {
    /// The ProjectionLab account ID
    pub pl_account_id: String,
    /// The new balance (or value, for real assets)
    pub balance: f64,
}

/// Parameters for batch-syncing account balances from external sources
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SyncAccountBalancesParams {
    /// List of account balance mappings to update
    pub mappings: Vec<BalanceMapping>,
}

// ---- Schema help ----

/// Parameters for looking up field documentation for a ProjectionLab entity type
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct SchemaHelpParams {
    /// The entity type to get documentation for. Use "topics" to list all available topics.
    pub topic: String,
}

// ---- Analysis, strategy, and what-if tools ----

/// Parameters for tax analytics
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct TaxAnalyticsParams {
    /// The ID of the plan
    pub plan_id: String,
    /// First age to include (default: plan start)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_age: Option<i64>,
    /// Last age to include (default: end of plan)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_age: Option<i64>,
    /// Also return a detailed breakdown for this age: income and taxable income line
    /// items, deductions, taxes, filer variables, and the tax-bracket stacks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail_age: Option<i64>,
}

/// Parameters for a single-year view (cash flow, estate)
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct PlanYearParams {
    /// The ID of the plan
    pub plan_id: String,
    /// Age to report (default: first simulated year for cash flow, final year for estate)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age: Option<i64>,
}

/// Parameters for a year-by-year report
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ReportParams {
    /// The ID of the plan
    pub plan_id: String,
    /// Summary categories to include, e.g. ["netWorth", "income", "expenses", "taxes",
    /// "contributions", "drawdown", "liquidNetWorth", "savingsRate", "netLegacy"].
    /// Default: netWorth, income, expenses, taxes, contributions, drawdown.
    /// An unknown name returns the list of available categories.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<Vec<String>>,
    /// First age to include
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_age: Option<i64>,
    /// Last age to include
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_age: Option<i64>,
    /// Include each category's line items, not just totals (default false)
    #[serde(default)]
    pub line_items: bool,
}

/// A change to a plan event inside a what-if scenario
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct WhatIfEventChange {
    /// Plan section: "expenses", "income", "priorities", "assets", or "accounts"
    pub section: String,
    /// Event to patch or remove
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Fields to set on the event (camelCase)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<JsonMap<String, JsonValue>>,
    /// A new event to add (camelCase fields; id/kind filled in)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add: Option<JsonMap<String, JsonValue>>,
    /// Remove the event with `id`
    #[serde(default)]
    pub remove: bool,
}

/// A change to a milestone or Current Finances account inside a what-if scenario
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct WhatIfPatch {
    /// Milestone or account id
    pub id: String,
    /// Fields to set (camelCase)
    pub patch: JsonMap<String, JsonValue>,
}

/// One what-if scenario. All changes apply to an unsaved copy of the plan.
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct WhatIfScenario {
    /// Short label for the scenario
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Plan variables to set (shallow, camelCase), e.g. {"inflation": 3.5},
    /// {"rothConversionStrategy": {"enabled": true, "maxAnnualAmount": 50000}}
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<JsonMap<String, JsonValue>>,
    /// Withdrawal strategy fields to set (shallow, camelCase)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withdrawal_strategy: Option<JsonMap<String, JsonValue>>,
    /// Event changes (patch, add, or remove)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<WhatIfEventChange>>,
    /// Milestone changes, e.g. retirement criteria
    #[serde(skip_serializing_if = "Option::is_none")]
    pub milestones: Option<Vec<WhatIfPatch>>,
    /// Current Finances account changes, e.g. {"id": "...", "patch": {"balance": 50000}}
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accounts: Option<Vec<WhatIfPatch>>,
}

/// Parameters for what-if analysis
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct WhatIfParams {
    /// The ID of the plan to start from
    pub plan_id: String,
    /// Scenarios to compare against the saved plan (each runs independently)
    pub scenarios: Vec<WhatIfScenario>,
}

/// Parameters for strategy optimization comparisons
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct OptimizeParams {
    /// The ID of the plan
    pub plan_id: String,
    /// "tax" (overall tax strategy), "roth" (Roth conversions), "withdrawal_shielding"
    /// (drawdown blending), or "gain_harvesting"
    pub module: String,
}

/// Parameters for updating a plan's tax optimization settings (saved to the plan)
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct UpdateTaxStrategyParams {
    /// The ID of the plan
    pub plan_id: String,
    /// Fields for variables.taxStrategy, e.g. {"enabled": true, "type": "bracket", "bracketRate": 22}.
    /// type: "bracket" | "irmaa" | "income" | "federalPovertyLevel"; optional start/end, capGainsBracketRate,
    /// incomeThreshold/incomeThresholdType, niitCeiling/irmaaCeiling/acaCeiling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tax_strategy: Option<JsonMap<String, JsonValue>>,
    /// Fields for variables.rothConversionStrategy: enabled, start, end, maxAnnualAmount, sourceAccountIds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roth_conversions: Option<JsonMap<String, JsonValue>>,
    /// Fields for variables.drawdownBlendingStrategy ("withdrawal shielding"): enabled, type, start, end
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withdrawal_shielding: Option<JsonMap<String, JsonValue>>,
    /// Fields for variables.gainHarvestingStrategy: enabled, start, end, maxAnnualGain,
    /// capGainsBracketRate, sourceAccountIds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gain_harvesting: Option<JsonMap<String, JsonValue>>,
    /// Fields for variables.flexSpending: enabled, scope, interpolation, points
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flexible_spending: Option<JsonMap<String, JsonValue>>,
    /// Replacement for variables.drawdownOrder (account withdrawal order)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drawdown_order: Option<Vec<JsonValue>>,
}

/// Parameters for comparing saved plans
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct ComparePlansParams {
    /// The first plan
    pub plan_id: String,
    /// Plans to compare with it (default: the baseline plan)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_plan_ids: Option<Vec<String>>,
}

/// Parameters for running Monte Carlo
#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
pub struct MonteCarloParams {
    /// The ID of the plan
    pub plan_id: String,
    /// Optional unsaved overrides for the plan's Monte Carlo settings (camelCase),
    /// e.g. {"trials": 1000, "sampling": "historical"}
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<JsonMap<String, JsonValue>>,
}
