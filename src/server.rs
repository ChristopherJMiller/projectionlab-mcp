use crate::browser::BrowserSession;
use crate::resources::ResourceProvider;
use crate::sync::SyncManager;
use crate::tools::doc::{anyhow_err, err, ok_json, ok_text};
use crate::tools::params::*;
use crate::tools::plan_events::Section;
use crate::tools::{
    accounts, analysis, debts, history, plan_accounts, plan_events, plans, real_assets, schema_help,
};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    service::RequestContext,
    tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler,
};
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

/// Main MCP server for ProjectionLab integration
#[derive(Clone)]
pub struct ProjectionLabServer {
    browser: Arc<Mutex<Option<BrowserSession>>>,
    sync_manager: Arc<SyncManager>,
    resource_provider: Arc<ResourceProvider>,
    tool_router: ToolRouter<Self>,
}

impl Default for ProjectionLabServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl ProjectionLabServer {
    pub fn new() -> Self {
        let browser = Arc::new(Mutex::new(None));
        let sync_manager = Arc::new(SyncManager::new(browser.clone()));
        let resource_provider = Arc::new(ResourceProvider::new(sync_manager.clone()));

        Self {
            browser,
            sync_manager,
            resource_provider,
            tool_router: Self::tool_router(),
        }
    }

    /// Get a handle to the browser for cleanup on shutdown
    pub fn browser_handle(&self) -> Arc<Mutex<Option<BrowserSession>>> {
        self.browser.clone()
    }

    /// Get the browser session, waiting for background initialization if needed.
    async fn get_browser(
        &self,
    ) -> Result<tokio::sync::MutexGuard<'_, Option<BrowserSession>>, McpError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            {
                let guard = self.browser.lock().await;
                if guard.is_some() {
                    return Ok(guard);
                }
            }
            if std::time::Instant::now() >= deadline {
                return Err(err(
                    "Browser session not initialized after 60s — is Firefox/GeckoDriver available?",
                ));
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }

    // ---- Raw Plugin API tools ----

    #[tool(description = "Raw Plugin API: getAccounts. Returns active Current Finances accounts, debts, and real assets with their current values (balance/costBasis or value/purchasePrice). Lightweight alternative to export_data.")]
    async fn get_accounts(&self) -> Result<CallToolResult, McpError> {
        let result = self
            .sync_manager
            .call_api("getAccounts", vec![])
            .await
            .map_err(anyhow_err)?;
        ok_json(&result)
    }

    #[tool(description = "Raw Plugin API: updateAccount. Assigns fields on a Current Finances account. Without force=true only existing fields may be set; id/kind/type/subtype must still identify a supported account. Prefer accounts_update.")]
    async fn update_account(
        &self,
        Parameters(params): Parameters<UpdateAccountParams>,
    ) -> Result<CallToolResult, McpError> {
        self.sync_manager
            .update_account(
                &params.account_id,
                JsonValue::Object(params.data),
                params.force.unwrap_or(false),
            )
            .await
            .map_err(anyhow_err)?;
        ok_text(format!("Account {} updated", params.account_id))
    }

    #[tool(description = "Raw Plugin API: updateAccountValues. Sets current values on a Current Finances item and records history: balance (accounts, debts), costBasis (taxable-style accounts), value/purchasePrice (real assets). Archived items are rejected.")]
    async fn update_account_values(
        &self,
        Parameters(params): Parameters<UpdateAccountValuesParams>,
    ) -> Result<CallToolResult, McpError> {
        self.sync_manager
            .update_account_values(&params.account_id, JsonValue::Object(params.values))
            .await
            .map_err(anyhow_err)?;
        ok_text(format!("Account {} values updated", params.account_id))
    }

    #[tool(description = "Raw Plugin API: exportData. Returns the full V5 export envelope {schema, exportedAt, documents: {currentFinances, plans, settings, accountHistory}}. Large.")]
    async fn export_data(&self) -> Result<CallToolResult, McpError> {
        let raw = self.sync_manager.get_raw().await.map_err(anyhow_err)?;
        ok_json(&raw)
    }

    #[tool(description = "Raw Plugin API: restoreCurrentFinances. Replaces the Current Finances document ({accounts: {<id>: {...}}, household, location}). Submitted accounts are merged over existing ones; omitted accounts are kept (use accounts_archive to remove).")]
    async fn restore_current_finances(
        &self,
        Parameters(params): Parameters<RestoreCurrentFinancesParams>,
    ) -> Result<CallToolResult, McpError> {
        self.sync_manager
            .restore_current_finances(JsonValue::Object(params.new_state))
            .await
            .map_err(anyhow_err)?;
        ok_text("Current Finances restored")
    }

    #[tool(description = "Raw Plugin API: restorePlans. Replaces ALL plans with the given array; any plan not included is DELETED. Prefer the targeted plan/event tools.")]
    async fn restore_plans(
        &self,
        Parameters(params): Parameters<RestorePlansParams>,
    ) -> Result<CallToolResult, McpError> {
        if params.new_plans.is_empty() {
            return Err(err("Refusing to restore an empty plan list (that would delete every plan)"));
        }
        self.sync_manager
            .restore_plans(JsonValue::Array(params.new_plans))
            .await
            .map_err(anyhow_err)?;
        ok_text("Plans restored")
    }

    #[tool(description = "Raw Plugin API: restoreSettings. Replaces the settings document.")]
    async fn restore_settings(
        &self,
        Parameters(params): Parameters<RestoreSettingsParams>,
    ) -> Result<CallToolResult, McpError> {
        self.sync_manager
            .restore_settings(JsonValue::Object(params.new_settings))
            .await
            .map_err(anyhow_err)?;
        ok_text("Settings restored")
    }

    // ---- Current Finances: accounts ----

    #[tool(description = "List Current Finances items (accounts, debts, real assets) with balances/values. Filter by kind ('account', 'debt', 'real-asset'), account_type (e.g. 'savings', 'taxable', '401k', 'mortgage', or the groupings 'investment'/'debt'), or owner. Archived items are hidden unless include_archived=true.")]
    async fn accounts_list(
        &self,
        Parameters(params): Parameters<AccountsListParams>,
    ) -> Result<CallToolResult, McpError> {
        accounts::list(&self.sync_manager, params).await
    }

    #[tool(description = "Get full details of a Current Finances account, debt, or real asset by ID, plus its most recent history points")]
    async fn accounts_get(
        &self,
        Parameters(params): Parameters<AccountsGetParams>,
    ) -> Result<CallToolResult, McpError> {
        accounts::get(&self.sync_manager, params).await
    }

    #[tool(description = "Create a Current Finances account, debt, or real asset. account_type is the ProjectionLab type (e.g. 'savings', 'taxable', '401k', 'roth-ira', 'hsa', 'crypto', 'mortgage', 'student-loans', 'real-estate', 'car'); kind is inferred. data holds camelCase fields like name, owner ('me'|'spouse'|'joint'), balance (or value for real assets), costBasis, interestRate, monthlyPayment.")]
    async fn accounts_create(
        &self,
        Parameters(params): Parameters<AccountsCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        accounts::create(&self.sync_manager, params).await
    }

    #[tool(description = "Update a Current Finances account (partial, camelCase fields). Value fields (balance, costBasis, value, purchasePrice) are applied via updateAccountValues so ProjectionLab records history; other fields (name, owner, interestRate, ...) via updateAccount.")]
    async fn accounts_update(
        &self,
        Parameters(params): Parameters<AccountsUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        accounts::update(&self.sync_manager, params).await
    }

    #[tool(description = "Set the current balance of an account or debt (or the value of a real asset). ProjectionLab records the change in account and net worth history automatically.")]
    async fn accounts_update_balance(
        &self,
        Parameters(params): Parameters<AccountsUpdateBalanceParams>,
    ) -> Result<CallToolResult, McpError> {
        accounts::update_balance(&self.sync_manager, params).await
    }

    #[tool(description = "Archive a Current Finances account, debt, or real asset as of a date (default today); this is how items are removed (ProjectionLab's plugin API cannot delete them). It stops counting toward net worth from that date; history is kept. Reverse with accounts_unarchive.")]
    async fn accounts_archive(
        &self,
        Parameters(params): Parameters<AccountsArchiveParams>,
    ) -> Result<CallToolResult, McpError> {
        accounts::archive(&self.sync_manager, params).await
    }

    #[tool(description = "Unarchive a previously archived Current Finances account, debt, or real asset")]
    async fn accounts_unarchive(
        &self,
        Parameters(params): Parameters<AccountsUnarchiveParams>,
    ) -> Result<CallToolResult, McpError> {
        accounts::unarchive(&self.sync_manager, params).await
    }

    // ---- Current Finances: real assets ----

    #[tool(description = "List real assets in Current Finances (homes, cars, land, valuables; kind 'real-asset') with current value and purchase price")]
    async fn real_assets_list(
        &self,
        Parameters(params): Parameters<RealAssetsListParams>,
    ) -> Result<CallToolResult, McpError> {
        real_assets::list(&self.sync_manager, params).await
    }

    #[tool(description = "Get full details of a Current Finances real asset by ID")]
    async fn real_assets_get(
        &self,
        Parameters(params): Parameters<RealAssetsGetParams>,
    ) -> Result<CallToolResult, McpError> {
        real_assets::get(&self.sync_manager, params).await
    }

    #[tool(description = "Create a real asset in Current Finances. data must include type ('real-estate', 'car', 'land', 'motorcycle', 'boat', 'jewelry', 'precious-metals', 'furniture', 'instrument', 'machinery', 'other') and name; typically value and purchasePrice.")]
    async fn real_assets_create(
        &self,
        Parameters(params): Parameters<RealAssetsCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        real_assets::create(&self.sync_manager, params).await
    }

    #[tool(description = "Update a Current Finances real asset (partial). Changing value or purchasePrice records history. To remove one, use accounts_archive.")]
    async fn real_assets_update(
        &self,
        Parameters(params): Parameters<RealAssetsUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        real_assets::update(&self.sync_manager, params).await
    }

    // ---- Current Finances: debts ----

    #[tool(description = "List debts in Current Finances (mortgages, loans, credit cards; kind 'debt') with balance, rate, payment, and securing asset")]
    async fn debts_list(
        &self,
        Parameters(params): Parameters<DebtsListParams>,
    ) -> Result<CallToolResult, McpError> {
        debts::list(&self.sync_manager, params).await
    }

    #[tool(description = "Get full details of a Current Finances debt by ID")]
    async fn debts_get(
        &self,
        Parameters(params): Parameters<DebtsGetParams>,
    ) -> Result<CallToolResult, McpError> {
        debts::get(&self.sync_manager, params).await
    }

    #[tool(description = "Create a debt in Current Finances. data must include type ('mortgage', 'home-equity-loan', 'auto-loan', 'secured-loan', 'student-loans', 'credit-card', 'personal-loan', 'medical-debt', 'other') and name; typically balance, interestRate, monthlyPayment, owner, and securedByAssetId for secured loans.")]
    async fn debts_create(
        &self,
        Parameters(params): Parameters<DebtsCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        debts::create(&self.sync_manager, params).await
    }

    #[tool(description = "Update a Current Finances debt (partial). Use this to change balances, rates, payments, etc. Balance changes record history. To remove a paid-off debt, use accounts_archive.")]
    async fn debts_update(
        &self,
        Parameters(params): Parameters<DebtsUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        debts::update(&self.sync_manager, params).await
    }

    #[tool(description = "Get household information from Current Finances: people (you/partner, birth year/month), partner status, and location")]
    async fn household_get(&self) -> Result<CallToolResult, McpError> {
        let data = self.sync_manager.get_data().await.map_err(anyhow_err)?;
        let cf = data.current_finances();
        ok_json(&json!({
            "household": cf.household,
            "location": cf.location,
            "last_updated": cf.last_updated,
        }))
    }

    // ---- Plans ----

    #[tool(description = "List all plans with key metadata: id, name, which plan is active (selected in the UI) and which is the baseline, starting-conditions mode, and event counts")]
    async fn plans_list(&self) -> Result<CallToolResult, McpError> {
        plans::list(&self.sync_manager).await
    }

    #[tool(description = "Get the complete raw plan document (events, milestones, variables, withdrawal strategy, Monte Carlo settings, starting conditions)")]
    async fn plans_get(
        &self,
        Parameters(params): Parameters<PlanGetParams>,
    ) -> Result<CallToolResult, McpError> {
        plans::get(&self.sync_manager, params).await
    }

    #[tool(description = "Get the variables/assumptions for a plan (returns, inflation, taxes, bond settings, ...) along with its withdrawal strategy and Monte Carlo settings")]
    async fn plans_get_variables(
        &self,
        Parameters(params): Parameters<PlanGetParams>,
    ) -> Result<CallToolResult, McpError> {
        plans::get_variables(&self.sync_manager, params).await
    }

    #[tool(description = "Update variables/assumptions for a plan (shallow partial merge of top-level variable keys, camelCase). Use this to adjust investment returns, inflation, tax settings, etc.")]
    async fn plans_update_variables(
        &self,
        Parameters(params): Parameters<PlansUpdateVariablesParams>,
    ) -> Result<CallToolResult, McpError> {
        plans::update_variables(&self.sync_manager, params).await
    }

    #[tool(description = "Clone a plan for scenario comparison. Creates a deep copy with a new name and ID.")]
    async fn plans_clone(
        &self,
        Parameters(params): Parameters<PlansCloneParams>,
    ) -> Result<CallToolResult, McpError> {
        plans::clone_plan(&self.sync_manager, params).await
    }

    #[tool(description = "Create a new plan. Without clone_from, creates an empty plan (no expenses/income/priorities/assets; built-in milestones and settings kept) that starts from Current Finances. With clone_from, copies an existing plan.")]
    async fn plans_create(
        &self,
        Parameters(params): Parameters<PlansCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        plans::create(&self.sync_manager, params).await
    }

    #[tool(description = "Delete a plan permanently. Requires confirm=true as a safety check. This cannot be undone.")]
    async fn plans_delete(
        &self,
        Parameters(params): Parameters<PlansDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        plans::delete(&self.sync_manager, params).await
    }

    #[tool(description = "Update plan metadata: name or icon. (The per-plan `active` flag no longer exists in ProjectionLab V5.)")]
    async fn plans_update_metadata(
        &self,
        Parameters(params): Parameters<PlansUpdateMetadataParams>,
    ) -> Result<CallToolResult, McpError> {
        plans::update_metadata(&self.sync_manager, params).await
    }

    #[tool(description = "Get a plan's milestones (retirement, life expectancy, FI, and custom goals). Projected completion ages come from plans_get_simulation_results (and probabilities from plans_run_monte_carlo).")]
    async fn plans_get_milestones(
        &self,
        Parameters(params): Parameters<PlanGetParams>,
    ) -> Result<CallToolResult, McpError> {
        plans::get_milestones(&self.sync_manager, params).await
    }

    #[tool(description = "Create a milestone in a plan. Requires name, icon, color, and a criteria array; each criterion has type ('year', 'age', 'milestone', 'account', 'loan', 'liquidNetWorth', 'investableAssets', ...), value, and optional operator/modifier/refId/valueType. See schema_help topic 'milestone'.")]
    async fn milestones_create(
        &self,
        Parameters(params): Parameters<MilestoneCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        plans::create_milestone(&self.sync_manager, params).await
    }

    #[tool(description = "Update a milestone in a plan (partial). Works for built-in milestones too, e.g. changing retirement criteria or hiding one.")]
    async fn milestones_update(
        &self,
        Parameters(params): Parameters<MilestoneUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        plans::update_milestone(&self.sync_manager, params).await
    }

    #[tool(description = "Delete a custom milestone from a plan (built-in milestones cannot be deleted)")]
    async fn milestones_delete(
        &self,
        Parameters(params): Parameters<MilestoneDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        plans::delete_milestone(&self.sync_manager, params).await
    }

    // ---- Expenses ----

    #[tool(description = "List expense events in a plan with amounts, frequencies, timing, and spending type (including debt-repayment expenses)")]
    async fn expenses_list(
        &self,
        Parameters(params): Parameters<PlanEventsListParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_events::list(&self.sync_manager, Section::Expenses, params).await
    }

    #[tool(description = "Create an expense event in a plan. Requires name and type plus amount/timing fields (see schema_help topic 'expense'); id and kind are filled in.")]
    async fn expenses_create(
        &self,
        Parameters(params): Parameters<PlanEventCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_events::create(&self.sync_manager, Section::Expenses, params).await
    }

    #[tool(description = "Update an expense event in a plan (partial, camelCase fields)")]
    async fn expenses_update(
        &self,
        Parameters(params): Parameters<PlanEventUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_events::update(&self.sync_manager, Section::Expenses, params).await
    }

    #[tool(description = "Delete an expense event from a plan")]
    async fn expenses_delete(
        &self,
        Parameters(params): Parameters<PlanEventDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_events::delete(&self.sync_manager, Section::Expenses, params).await
    }

    // ---- Income ----

    #[tool(description = "List income events in a plan with amounts, frequencies, timing, and tax/withholding settings")]
    async fn income_list(
        &self,
        Parameters(params): Parameters<PlanEventsListParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_events::list(&self.sync_manager, Section::Income, params).await
    }

    #[tool(description = "Create an income event in a plan. Requires name and type ('salary', 'rsu', 'other', ...) plus amount/timing fields (see schema_help topic 'income').")]
    async fn income_create(
        &self,
        Parameters(params): Parameters<PlanEventCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_events::create(&self.sync_manager, Section::Income, params).await
    }

    #[tool(description = "Update an income event in a plan (partial, camelCase fields)")]
    async fn income_update(
        &self,
        Parameters(params): Parameters<PlanEventUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_events::update(&self.sync_manager, Section::Income, params).await
    }

    #[tool(description = "Delete an income event from a plan")]
    async fn income_delete(
        &self,
        Parameters(params): Parameters<PlanEventDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_events::delete(&self.sync_manager, Section::Income, params).await
    }

    // ---- Priorities ----

    #[tool(description = "List priority/goal events in a plan (401k/Roth contributions, savings goals, taxable investing, ...)")]
    async fn priorities_list(
        &self,
        Parameters(params): Parameters<PlanEventsListParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_events::list(&self.sync_manager, Section::Priorities, params).await
    }

    #[tool(description = "Create a priority/goal event in a plan (e.g. 401k contribution, savings goal). Requires name, type, and accountId (see schema_help topic 'priority').")]
    async fn priorities_create(
        &self,
        Parameters(params): Parameters<PlanEventCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_events::create(&self.sync_manager, Section::Priorities, params).await
    }

    #[tool(description = "Update a priority/goal event in a plan (partial, camelCase fields)")]
    async fn priorities_update(
        &self,
        Parameters(params): Parameters<PlanEventUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_events::update(&self.sync_manager, Section::Priorities, params).await
    }

    #[tool(description = "Delete a priority/goal event from a plan")]
    async fn priorities_delete(
        &self,
        Parameters(params): Parameters<PlanEventDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_events::delete(&self.sync_manager, Section::Priorities, params).await
    }

    // ---- Plan real-asset events ----

    #[tool(description = "List planned real-asset events in a plan (future home/car purchases) with value, purchase price, down payment, and financing")]
    async fn plan_assets_list(
        &self,
        Parameters(params): Parameters<PlanEventsListParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_events::list(&self.sync_manager, Section::Assets, params).await
    }

    #[tool(description = "Create a planned real-asset event in a plan (e.g. buy a house or car). See schema_help topic 'asset' for fields (value, purchasePrice, downPayment, interestRate, ...).")]
    async fn plan_assets_create(
        &self,
        Parameters(params): Parameters<PlanEventCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_events::create(&self.sync_manager, Section::Assets, params).await
    }

    #[tool(description = "Update a planned real-asset event in a plan (partial). Use this to change purchase price, down payment, mortgage rate, timing, etc.")]
    async fn plan_assets_update(
        &self,
        Parameters(params): Parameters<PlanEventUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_events::update(&self.sync_manager, Section::Assets, params).await
    }

    #[tool(description = "Delete a planned real-asset event from a plan")]
    async fn plan_assets_delete(
        &self,
        Parameters(params): Parameters<PlanEventDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_events::delete(&self.sync_manager, Section::Assets, params).await
    }

    // ---- Plan accounts ----

    #[tool(description = "List a plan's per-account settings (growth, dividends, liquidity, withdrawal age) with the linked Current Finances account balance and, for custom plans, the plan's own starting balance")]
    async fn plan_accounts_list(
        &self,
        Parameters(params): Parameters<PlanEventsListParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_accounts::list(&self.sync_manager, params).await
    }

    #[tool(description = "Update a plan's per-account settings (partial). Shared fields (balance, costBasis, name, icon, owner) are written through to the plan's starting conditions (custom plans) and the linked Current Finances account, so one call keeps simulation and dashboard in sync.")]
    async fn plan_accounts_update(
        &self,
        Parameters(params): Parameters<PlanEventUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_accounts::update(&self.sync_manager, params).await
    }

    // ---- History ----

    #[tool(description = "Get net worth history over time from ProjectionLab (recorded automatically whenever account values change). Each point has date, netWorth, liquidNetWorth, investableAssets, and a breakdown (savings, taxable, taxDeferred, taxFree, crypto, assets, debt, loans). Optional ISO start_date/end_date filters.")]
    async fn net_worth_history(
        &self,
        Parameters(params): Parameters<NetWorthHistoryParams>,
    ) -> Result<CallToolResult, McpError> {
        history::net_worth_history(&self.sync_manager, params).await
    }

    #[tool(description = "Get per-account value history (balance, costBasis, or value by date) for one account or all accounts, optionally within an ISO date range")]
    async fn account_history(
        &self,
        Parameters(params): Parameters<AccountHistoryParams>,
    ) -> Result<CallToolResult, McpError> {
        history::account_history(&self.sync_manager, params).await
    }

    // ---- Browser / Simulation tools ----

    #[tool(description = "Execute JavaScript in the ProjectionLab browser context. Useful for exploring Vue app internals (Pinia stores via document.querySelector('#app').__vue_app__.config.globalProperties.$pinia) or data not exposed by the Plugin API. Optionally navigate to an app path first (e.g. '/plan/overview', '/net-worth/history').")]
    async fn run_js_in_browser(
        &self,
        Parameters(params): Parameters<RunJsInBrowserParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut guard = self.get_browser().await?;
        let browser = guard
            .as_mut()
            .ok_or_else(|| err("Browser session not initialized"))?;

        let navigated = if let Some(path) = &params.navigate_to {
            browser
                .navigate_to(path)
                .await
                .map_err(|e| err(format!("Navigation failed: {}", e)))?;
            true
        } else {
            false
        };

        let result = if params.r#async {
            browser.execute_js_async(&params.script).await
        } else {
            browser.execute_js(&params.script).await
        }
        .map_err(|e| err(format!("JS execution failed: {:#}", e)))?;

        if navigated {
            browser
                .navigate_to_home()
                .await
                .map_err(|e| err(format!("Failed to navigate home: {}", e)))?;
        }

        ok_json(&result)
    }

    #[tool(description = "Get projection results for a plan: outcome, year-by-year net worth/income/expenses/taxes/contributions/drawdown, milestone completion ages, and notable events. Temporarily selects the plan in the app and restores the previous selection afterwards.")]
    async fn plans_get_simulation_results(
        &self,
        Parameters(params): Parameters<PlanGetParams>,
    ) -> Result<CallToolResult, McpError> {
        analysis::simulation_results(&self.sync_manager, params).await
    }

    #[tool(description = "Get a detailed financial snapshot at a specific age from a plan's projection: every summary category (income, taxes, expenses, contributions, drawdown, debt payments, net legacy, ...) with line items, plus ending balances.")]
    async fn plans_get_year_snapshot(
        &self,
        Parameters(params): Parameters<YearSnapshotParams>,
    ) -> Result<CallToolResult, McpError> {
        analysis::year_snapshot(&self.sync_manager, params).await
    }

    #[tool(description = "Reports: year-by-year table of chosen projection metrics (summary categories such as netWorth, income, expenses, taxes, contributions, drawdown, liquidNetWorth, savingsRate, netLegacy) over an optional age range, with totals and optional line items. An unknown metric name returns the available list.")]
    async fn plans_get_report(
        &self,
        Parameters(params): Parameters<ReportParams>,
    ) -> Result<CallToolResult, McpError> {
        analysis::report(&self.sync_manager, params).await
    }

    #[tool(description = "Tax analytics for a plan's projection: per year gross/taxable/net income, total tax and its components, effective rate, AGI, IRMAA MAGI, Roth conversions, gain harvesting, and penalties; lifetime totals and peak tax year; plus the tax settings in effect. With detail_age, also the income/deduction line items and tax-bracket stacks for that year.")]
    async fn plans_get_tax_analytics(
        &self,
        Parameters(params): Parameters<TaxAnalyticsParams>,
    ) -> Result<CallToolResult, McpError> {
        analysis::tax_analytics(&self.sync_manager, params).await
    }

    #[tool(description = "Cash flow for one year of a plan's projection: inflows (income, withdrawals, asset sales) and outflows (expenses, taxes, contributions, debt payments) with line items, spending by essential/discretionary/flexible, and how contributions were funded. Defaults to the first projected year.")]
    async fn plans_get_cash_flow(
        &self,
        Parameters(params): Parameters<PlanYearParams>,
    ) -> Result<CallToolResult, McpError> {
        analysis::cash_flow(&self.sync_manager, params).await
    }

    #[tool(description = "Estate view of a plan's projection: net legacy (what heirs receive after final taxes and costs) with its components by account, asset, and debt, the plan's estate settings, and net legacy over time. Defaults to the final projected year.")]
    async fn plans_get_estate(
        &self,
        Parameters(params): Parameters<PlanYearParams>,
    ) -> Result<CallToolResult, McpError> {
        analysis::estate(&self.sync_manager, params).await
    }

    #[tool(description = "Roth conversions in a plan's projection: each year's converted amount with that year's taxes, taxable income, and IRMAA MAGI, lifetime total, and the Roth conversion and tax strategy settings driving them.")]
    async fn plans_get_roth_conversions(
        &self,
        Parameters(params): Parameters<PlanGetParams>,
    ) -> Result<CallToolResult, McpError> {
        analysis::roth_conversions(&self.sync_manager, params).await
    }

    #[tool(description = "Get a plan's tax optimization settings (Optimize pages): tax strategy target, Roth conversions, withdrawal shielding, gain harvesting, flexible spending, and drawdown order, plus what the app derives for this plan (tax mode, valid bracket rates, gain-harvesting-eligible and tax-deferred accounts, drawdown modules).")]
    async fn plans_get_tax_strategy(
        &self,
        Parameters(params): Parameters<PlanGetParams>,
    ) -> Result<CallToolResult, McpError> {
        analysis::tax_strategy(&self.sync_manager, params).await
    }

    #[tool(description = "Save tax optimization settings to a plan (partial updates per strategy): tax_strategy, roth_conversions, withdrawal_shielding, gain_harvesting, flexible_spending, drawdown_order. Returns what ProjectionLab stored. Try changes first with plans_what_if or plans_optimize_tax_strategy.")]
    async fn plans_update_tax_strategy(
        &self,
        Parameters(params): Parameters<UpdateTaxStrategyParams>,
    ) -> Result<CallToolResult, McpError> {
        analysis::update_tax_strategy(&self.sync_manager, params).await
    }

    #[tool(description = "Optimize: compare a strategy module the way ProjectionLab's Optimize pages do — the saved plan vs. the strategy disabled (and enabled with defaults if currently off), plus each alternative bracket when the tax strategy fills a bracket. module: tax, roth, withdrawal_shielding, gain_harvesting. Returns headline metrics (final net worth, net legacy, lifetime taxes, Roth conversions, milestone ages) and differences. Nothing is saved.")]
    async fn plans_optimize_tax_strategy(
        &self,
        Parameters(params): Parameters<OptimizeParams>,
    ) -> Result<CallToolResult, McpError> {
        analysis::optimize(&self.sync_manager, params).await
    }

    #[tool(description = "What-if analysis: apply changes to an unsaved copy of a plan and compare against the saved plan. Each scenario can set plan variables (assumptions, tax/Roth strategies, ...), withdrawal strategy fields, patch/add/remove events (expenses, income, priorities, assets, accounts), patch milestones (e.g. retirement criteria), and patch Current Finances accounts. Returns headline metrics and differences per scenario. Nothing is saved.")]
    async fn plans_what_if(
        &self,
        Parameters(params): Parameters<WhatIfParams>,
    ) -> Result<CallToolResult, McpError> {
        analysis::what_if(&self.sync_manager, params).await
    }

    #[tool(description = "Compare saved plans side by side (default: against the baseline plan): outcome, retirement age, net worth at retirement, final net worth and net legacy, lifetime income/taxes/expenses, Roth conversions, milestone ages, and net worth by age, with differences from the first plan.")]
    async fn plans_compare(
        &self,
        Parameters(params): Parameters<ComparePlansParams>,
    ) -> Result<CallToolResult, McpError> {
        analysis::compare_plans(&self.sync_manager, params).await
    }

    #[tool(description = "Set the plan ProjectionLab uses as the comparison baseline (saved in settings)")]
    async fn plans_set_baseline(
        &self,
        Parameters(params): Parameters<PlanGetParams>,
    ) -> Result<CallToolResult, McpError> {
        analysis::set_baseline(&self.sync_manager, params).await
    }

    #[tool(description = "Run a Monte Carlo simulation (Chance of Success) for a plan and return the success rate, outcome breakdown, milestone occurrence rates with ages by percentile, and percentile bands (net worth, withdrawal rate, expenses) by age. Optional settings override the plan's Monte Carlo settings for this run only (e.g. {\"trials\": 1000}). Nothing is saved.")]
    async fn plans_run_monte_carlo(
        &self,
        Parameters(params): Parameters<MonteCarloParams>,
    ) -> Result<CallToolResult, McpError> {
        analysis::monte_carlo(&self.sync_manager, params).await
    }

    // ---- Integration tools ----

    #[tool(description = "Batch-update Current Finances balances from external data (e.g. Monarch Money). Accepts [{pl_account_id, balance}] (balance means value for real assets). Unchanged balances are skipped; each change records history. Returns per-account results.")]
    async fn sync_account_balances(
        &self,
        Parameters(params): Parameters<SyncAccountBalancesParams>,
    ) -> Result<CallToolResult, McpError> {
        accounts::sync_balances(&self.sync_manager, params).await
    }

    // ---- Schema help ----

    #[tool(description = "Get field documentation, valid values, and examples for ProjectionLab entity types. Call with topic=\"topics\" to see all available topics. Use this BEFORE creating or updating events to understand required fields and type-specific variations (e.g., salary vs RSU income, 401k vs savings priorities, financed vs pay-in-full assets).")]
    async fn schema_help(
        &self,
        Parameters(params): Parameters<SchemaHelpParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(schema_help::lookup(&params.topic))
    }

    // ---- System tools ----

    #[tool(description = "Force refresh the cached data from ProjectionLab")]
    async fn refresh_cache(&self) -> Result<CallToolResult, McpError> {
        let data = self.sync_manager.refresh().await.map_err(anyhow_err)?;
        ok_json(&json!({
            "message": "Cache refreshed",
            "plans": data.plans().len(),
            "accounts": data.current_finances().accounts.len(),
        }))
    }

    #[tool(description = "Get the age of the cached data for debugging")]
    async fn cache_status(&self) -> Result<CallToolResult, McpError> {
        match self.sync_manager.cache_age().await {
            Some(age) => ok_text(format!("Cache age: {:.2} seconds", age.as_secs_f64())),
            None => ok_text("Cache is empty"),
        }
    }

    #[tool(description = "Check that ProjectionLab still matches what this server expects — Plugin API methods/version, export and document schema versions, and the app internals used for simulations, history, and saving — and report drift since the server's baseline: newer ProjectionLab releases, new app routes (features), new Plugin API methods, and added/removed export fields. Run this first if tools start failing.")]
    async fn compatibility_check(&self) -> Result<CallToolResult, McpError> {
        let report = crate::compat::run(&self.sync_manager)
            .await
            .map_err(anyhow_err)?;
        ok_json(&serde_json::to_value(&report).map_err(err)?)
    }

    #[tool(description = "Validate that the cached Plugin API key is still valid")]
    async fn validate_api_key(&self) -> Result<CallToolResult, McpError> {
        self.sync_manager
            .call_api("validateApiKey", vec![])
            .await
            .map_err(anyhow_err)?;
        ok_text("API key is valid")
    }
}

const INSTRUCTIONS: &str = "ProjectionLab MCP Server - AI financial advisor tooling for ProjectionLab (V5 data model).\n\n\
Data model: Current Finances holds every balance-sheet item in one map, distinguished by kind: 'account' (savings, taxable, 401k, roth-ira, hsa, crypto, ...), 'debt' (mortgage, auto-loan, student-loans, credit-card, ...), and 'real-asset' (real-estate, car, ...). Items are archived rather than deleted. Account and net worth history are recorded automatically whenever values change. Plans have expenses, income, priorities, assets (planned purchases), accounts (per-account plan settings linked by accountId), and milestones.\n\n\
Resources (read-only):\n\
- projectionlab://overview - Net worth and counts\n\
- projectionlab://accounts/summary - All active accounts, debts, and real assets\n\
- projectionlab://accounts/{id} - One account\n\
- projectionlab://plans/summary - All plans\n\
- projectionlab://expenses/summary, projectionlab://income/summary - Across all plans\n\
- projectionlab://net-worth/history - Net worth history\n\
- projectionlab://plan/{id}/variables, projectionlab://plan/{id}/milestones\n\n\
Tools by category:\n\
Current Finances: accounts_list, accounts_get, accounts_create, accounts_update, accounts_update_balance, accounts_archive, accounts_unarchive, household_get\n\
Real assets: real_assets_list, real_assets_get, real_assets_create, real_assets_update\n\
Debts: debts_list, debts_get, debts_create, debts_update\n\
Plans: plans_list, plans_get, plans_create, plans_clone, plans_delete, plans_get_variables, plans_update_variables, plans_update_metadata, plans_get_milestones\n\
Milestones: milestones_create, milestones_update, milestones_delete\n\
Expenses: expenses_list, expenses_create, expenses_update, expenses_delete\n\
Income: income_list, income_create, income_update, income_delete\n\
Priorities: priorities_list, priorities_create, priorities_update, priorities_delete\n\
Planned assets: plan_assets_list, plan_assets_create, plan_assets_update, plan_assets_delete\n\
Plan accounts: plan_accounts_list, plan_accounts_update\n\
History: net_worth_history, account_history\n\
Projections: plans_get_simulation_results, plans_get_year_snapshot, plans_get_report, plans_get_tax_analytics, plans_get_cash_flow, plans_get_estate, plans_get_roth_conversions\n\
Optimize: plans_get_tax_strategy, plans_update_tax_strategy, plans_optimize_tax_strategy, plans_what_if\n\
Compare & Monte Carlo: plans_compare, plans_set_baseline, plans_run_monte_carlo\n\
Integration: sync_account_balances\n\
Schema: schema_help (topic='topics' lists all)\n\
System: compatibility_check, refresh_cache, cache_status, validate_api_key, run_js_in_browser\n\
Raw Plugin API: get_accounts, update_account, update_account_values, export_data, restore_current_finances, restore_plans, restore_settings\n\n\
If tools fail unexpectedly, run compatibility_check: it reports which ProjectionLab surface changed. The server launches Firefox on first connection; the session persists between restarts.";

#[tool_handler]
impl ServerHandler for ProjectionLabServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
            server_info: Implementation {
                name: "projectionlab-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: Some("ProjectionLab MCP Server".to_string()),
                website_url: None,
                icons: None,
            },
            instructions: Some(INSTRUCTIONS.to_string()),
        }
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let resources = self.resource_provider.list_resources().await;
        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        ReadResourceRequestParam { uri }: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        self.resource_provider
            .read_resource(&uri)
            .await
            .map_err(|e| err(format!("Failed to read resource: {:#}", e)))
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        info!("Initializing ProjectionLab MCP Server...");

        let browser = self.browser.clone();
        let sync = self.sync_manager.clone();
        tokio::spawn(async move {
            {
                let mut guard = browser.lock().await;
                if guard.is_some() {
                    return;
                }
                info!("Creating browser session in background...");
                match BrowserSession::new().await {
                    Ok(session) => {
                        *guard = Some(session);
                        info!("Browser session ready!");
                    }
                    Err(e) => {
                        error!("Failed to initialize browser session: {}", e);
                        return;
                    }
                }
            }
            // Surface ProjectionLab changes early rather than as confusing tool failures.
            match crate::compat::run(&sync).await {
                Ok(report) if report.status == "broken" => error!(
                    "ProjectionLab compatibility check FAILED (run the compatibility_check tool): {:?}",
                    report.errors
                ),
                Ok(report) if report.status == "drift" => warn!(
                    "ProjectionLab has changed since the server baseline {} (app {:?}, latest {:?}); warnings: {:?}; new features: {}",
                    report.baseline_app_version,
                    report.app_version,
                    report.latest_registry_version,
                    report.warnings,
                    report.new_features.len()
                ),
                Ok(_) => info!("ProjectionLab compatibility check passed"),
                Err(e) => warn!("Could not run ProjectionLab compatibility check: {:#}", e),
            }
        });

        Ok(self.get_info())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn registered_tools() -> BTreeSet<String> {
        ProjectionLabServer::tool_router()
            .list_all()
            .into_iter()
            .map(|t| t.name.to_string())
            .collect()
    }

    /// snake_case identifiers (with at least one underscore) in `text`.
    fn tool_like_names(text: &str) -> BTreeSet<String> {
        text.split(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
            .filter(|w| w.contains('_') && !w.starts_with('_') && !w.ends_with('_'))
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn instructions_list_exactly_the_registered_tools() {
        let tools = registered_tools();
        let section = INSTRUCTIONS
            .split("Tools by category:")
            .nth(1)
            .expect("instructions have a tool section");
        let listed = tool_like_names(section);
        let missing: Vec<_> = tools.difference(&listed).collect();
        let stale: Vec<_> = listed.difference(&tools).collect();
        assert!(missing.is_empty(), "tools missing from INSTRUCTIONS: {:?}", missing);
        assert!(stale.is_empty(), "INSTRUCTIONS mention unknown tools: {:?}", stale);
    }

    #[test]
    fn readme_lists_exactly_the_registered_tools() {
        let tools = registered_tools();
        let readme = include_str!("../README.md");
        let table = readme
            .split("## Available Tools")
            .nth(1)
            .and_then(|s| s.split("\nNotes:").next())
            .expect("README has an Available Tools table");
        let listed: BTreeSet<String> = table
            .split('`')
            .skip(1)
            .step_by(2)
            .filter(|s| s.contains('_') && s.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
            .map(str::to_string)
            .collect();
        let missing: Vec<_> = tools.difference(&listed).collect();
        let stale: Vec<_> = listed.difference(&tools).collect();
        assert!(missing.is_empty(), "tools missing from README: {:?}", missing);
        assert!(stale.is_empty(), "README mentions unknown tools: {:?}", stale);
    }
}
