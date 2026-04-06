use crate::browser::BrowserSession;
use crate::resources::ResourceProvider;
use crate::sync::SyncManager;
use crate::tools::{accounts, debts, events, plan_accounts, plan_assets, plans, progress, schema_help, simulation, starting_assets};
use crate::tools::params::*;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    service::RequestContext,
    tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler,
};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

/// Main MCP server for ProjectionLab integration
#[derive(Clone)]
pub struct ProjectionLabServer {
    browser: Arc<Mutex<Option<BrowserSession>>>,
    sync_manager: Arc<SyncManager>,
    resource_provider: Arc<ResourceProvider>,
    tool_router: ToolRouter<Self>,
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

    /// Helper to get browser session, waiting for it to initialize if needed.
    async fn get_browser(
        &self,
    ) -> Result<tokio::sync::MutexGuard<'_, Option<BrowserSession>>, McpError> {
        // Wait up to 60s for the background browser init to complete
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            {
                let guard = self.browser.lock().await;
                if guard.is_some() {
                    return Ok(guard);
                }
            }
            if std::time::Instant::now() >= deadline {
                return Err(McpError::internal_error(
                    "Browser session not initialized after 60s — is Firefox/GeckoDriver available?",
                    None,
                ));
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    }

    /// Navigate to a plan page, wait for simulation, execute async JS, and return the result.
    async fn run_plan_js(
        &self,
        plan_id: &str,
        script: &str,
    ) -> Result<CallToolResult, McpError> {
        let mut guard = self.get_browser().await?;
        let browser = guard.as_mut().ok_or_else(|| {
            McpError::internal_error("Browser session not initialized", None)
        })?;

        let path = format!("/plan/{}", plan_id);
        browser.navigate_to(&path).await.map_err(|e| {
            McpError::internal_error(format!("Navigation failed: {}", e), None)
        })?;

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let result = browser.execute_js_async(script).await.map_err(|e| {
            McpError::internal_error(format!("JS execution failed: {}", e), None)
        })?;

        browser.navigate_to_home().await.map_err(|e| {
            McpError::internal_error(format!("Failed to navigate home after plan JS: {}", e), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| format!("{:?}", result)),
        )]))
    }

    // ---- Raw Plugin API tools ----

    #[tool(description = "Update an account in Current Finances with new data")]
    async fn update_account(
        &self,
        Parameters(params): Parameters<UpdateAccountParams>,
    ) -> Result<CallToolResult, McpError> {
        let browser_guard = self.get_browser().await?;
        let browser = browser_guard
            .as_ref()
            .ok_or_else(|| McpError::internal_error("Browser session not initialized", None))?;

        let mut args = vec![json!(params.account_id), json!(params.data)];
        if let Some(f) = params.force {
            args.push(json!({ "force": f }));
        }

        let result = browser
            .call_plugin_api("updateAccount", args)
            .await
            .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Account {} updated successfully: {}",
            params.account_id,
            serde_json::to_string_pretty(&result).unwrap_or_default()
        ))]))
    }

    #[tool(description = "Export all financial data from ProjectionLab")]
    async fn export_data(&self) -> Result<CallToolResult, McpError> {
        let browser_guard = self.get_browser().await?;
        let browser = browser_guard
            .as_ref()
            .ok_or_else(|| McpError::internal_error("Browser session not initialized", None))?;

        let result = browser
            .call_plugin_api("exportData", vec![])
            .await
            .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Replace the Current Finances state with new data")]
    async fn restore_current_finances(
        &self,
        Parameters(params): Parameters<RestoreCurrentFinancesParams>,
    ) -> Result<CallToolResult, McpError> {
        let browser_guard = self.get_browser().await?;
        let browser = browser_guard
            .as_ref()
            .ok_or_else(|| McpError::internal_error("Browser session not initialized", None))?;

        let result = browser
            .call_plugin_api("restoreCurrentFinances", vec![json!(params.new_state)])
            .await
            .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Current Finances restored successfully: {}",
            serde_json::to_string_pretty(&result).unwrap_or_default()
        ))]))
    }

    #[tool(description = "Replace all Plans with a new set of plans")]
    async fn restore_plans(
        &self,
        Parameters(params): Parameters<RestorePlansParams>,
    ) -> Result<CallToolResult, McpError> {
        self.sync_manager
            .update_plans(json!(params.new_plans))
            .await
            .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            "Plans restored successfully".to_string()
        )]))
    }

    #[tool(description = "Replace the Progress state with new data")]
    async fn restore_progress(
        &self,
        Parameters(params): Parameters<RestoreProgressParams>,
    ) -> Result<CallToolResult, McpError> {
        self.sync_manager
            .update_progress(json!(params.new_progress))
            .await
            .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            "Progress restored successfully".to_string()
        )]))
    }

    #[tool(description = "Replace Settings state with new data")]
    async fn restore_settings(
        &self,
        Parameters(params): Parameters<RestoreSettingsParams>,
    ) -> Result<CallToolResult, McpError> {
        self.sync_manager
            .update_settings(json!(params.new_settings))
            .await
            .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            "Settings restored successfully".to_string()
        )]))
    }

    // ---- Account tools ----

    #[tool(description = "List all accounts with optional filtering by type or owner")]
    async fn accounts_list(
        &self,
        Parameters(params): Parameters<AccountsListParams>,
    ) -> Result<CallToolResult, McpError> {
        accounts::list(&self.sync_manager, params).await
    }

    #[tool(description = "Get detailed information about a specific account by ID")]
    async fn accounts_get(
        &self,
        Parameters(params): Parameters<AccountsGetParams>,
    ) -> Result<CallToolResult, McpError> {
        accounts::get(&self.sync_manager, params).await
    }

    #[tool(description = "Create a new account (savings, investment, or debt)")]
    async fn accounts_create(
        &self,
        Parameters(params): Parameters<AccountsCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        accounts::create(&self.sync_manager, params).await
    }

    #[tool(
        description = "Update an existing account with partial data (preserves unspecified fields)"
    )]
    async fn accounts_update(
        &self,
        Parameters(params): Parameters<AccountsUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        accounts::update(&self.sync_manager, params).await
    }

    #[tool(description = "Convenience method to update just the balance of an account")]
    async fn accounts_update_balance(
        &self,
        Parameters(params): Parameters<AccountsUpdateBalanceParams>,
    ) -> Result<CallToolResult, McpError> {
        accounts::update_balance(&self.sync_manager, params).await
    }

    #[tool(description = "Delete an account by ID")]
    async fn accounts_delete(
        &self,
        Parameters(params): Parameters<AccountsDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        accounts::delete(&self.sync_manager, params).await
    }

    // ---- Starting Asset tools (Current Finances) ----

    #[tool(description = "List all starting assets in Current Finances (current car values, home values, etc.) with optional filtering by type or owner")]
    async fn starting_assets_list(
        &self,
        Parameters(params): Parameters<StartingAssetsListParams>,
    ) -> Result<CallToolResult, McpError> {
        starting_assets::list(&self.sync_manager, params).await
    }

    #[tool(description = "Get full details of a starting asset by ID")]
    async fn starting_assets_get(
        &self,
        Parameters(params): Parameters<StartingAssetsGetParams>,
    ) -> Result<CallToolResult, McpError> {
        starting_assets::get(&self.sync_manager, params).await
    }

    #[tool(description = "Create a new starting asset in Current Finances (e.g., car, home, valuables)")]
    async fn starting_assets_create(
        &self,
        Parameters(params): Parameters<StartingAssetsCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        starting_assets::create(&self.sync_manager, params).await
    }

    #[tool(description = "Update a starting asset in Current Finances (partial updates supported). Use this to change current asset values like home value, car value, etc.")]
    async fn starting_assets_update(
        &self,
        Parameters(params): Parameters<StartingAssetsUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        starting_assets::update(&self.sync_manager, params).await
    }

    #[tool(description = "Delete a starting asset from Current Finances")]
    async fn starting_assets_delete(
        &self,
        Parameters(params): Parameters<StartingAssetsDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        starting_assets::delete(&self.sync_manager, params).await
    }

    // ---- Debt tools (Current Finances) ----

    #[tool(description = "List all debt accounts in Current Finances (mortgages, loans, credit cards) with optional filtering by type or owner")]
    async fn debts_list(
        &self,
        Parameters(params): Parameters<DebtsListParams>,
    ) -> Result<CallToolResult, McpError> {
        debts::list(&self.sync_manager, params).await
    }

    #[tool(description = "Get full details of a debt account by ID")]
    async fn debts_get(
        &self,
        Parameters(params): Parameters<DebtsGetParams>,
    ) -> Result<CallToolResult, McpError> {
        debts::get(&self.sync_manager, params).await
    }

    #[tool(description = "Create a new debt in Current Finances (e.g., mortgage, auto loan, student loan, credit card)")]
    async fn debts_create(
        &self,
        Parameters(params): Parameters<DebtsCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        debts::create(&self.sync_manager, params).await
    }

    #[tool(description = "Update a debt in Current Finances (partial updates supported). Use this to change mortgage rates, loan balances, payment amounts, etc.")]
    async fn debts_update(
        &self,
        Parameters(params): Parameters<DebtsUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        debts::update(&self.sync_manager, params).await
    }

    #[tool(description = "Delete a debt from Current Finances")]
    async fn debts_delete(
        &self,
        Parameters(params): Parameters<DebtsDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        debts::delete(&self.sync_manager, params).await
    }

    // ---- Plan tools ----

    #[tool(description = "List all plans with key metadata (id, name, active status, event counts)")]
    async fn plans_list(&self) -> Result<CallToolResult, McpError> {
        plans::list(&self.sync_manager).await
    }

    #[tool(description = "Get detailed information about a specific plan including variables, milestones, withdrawal strategy, and Monte Carlo settings")]
    async fn plans_get(
        &self,
        Parameters(params): Parameters<PlanGetParams>,
    ) -> Result<CallToolResult, McpError> {
        plans::get(&self.sync_manager, params).await
    }

    #[tool(description = "Get just the variables/assumptions for a specific plan (investment returns, inflation, tax settings, etc.)")]
    async fn plans_get_variables(
        &self,
        Parameters(params): Parameters<PlanGetParams>,
    ) -> Result<CallToolResult, McpError> {
        plans::get_variables(&self.sync_manager, params).await
    }

    #[tool(description = "Update variables/assumptions for a specific plan (partial updates supported). Use this to adjust investment returns, inflation, tax settings, Monte Carlo parameters, etc.")]
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

    #[tool(description = "Create a new plan. Either creates an empty plan shell or clones from an existing plan. Use clone_from to base it on an existing plan's structure.")]
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

    #[tool(description = "Update plan metadata: name, icon, or active status. Only these top-level fields can be changed — use dedicated tools for expenses, income, milestones, variables, etc.")]
    async fn plans_update_metadata(
        &self,
        Parameters(params): Parameters<PlansUpdateMetadataParams>,
    ) -> Result<CallToolResult, McpError> {
        plans::update_metadata(&self.sync_manager, params).await
    }

    // ---- Milestone tools ----

    #[tool(description = "Get milestones and computed milestones for a specific plan (retirement dates, financial independence targets, goal timelines)")]
    async fn plans_get_milestones(
        &self,
        Parameters(params): Parameters<PlanGetParams>,
    ) -> Result<CallToolResult, McpError> {
        plans::get_milestones(&self.sync_manager, params).await
    }

    #[tool(description = "Create a new milestone in a plan. Requires: name, icon, color, criteria array. Each criterion has type ('year', 'milestone', 'account', 'loan'), value, and optional modifier/operator/ref_id fields.")]
    async fn milestones_create(
        &self,
        Parameters(params): Parameters<MilestoneCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        plans::create_milestone(&self.sync_manager, params).await
    }

    #[tool(description = "Update an existing milestone in a plan (partial updates supported). Can modify name, icon, color, criteria, hidden, etc.")]
    async fn milestones_update(
        &self,
        Parameters(params): Parameters<MilestoneUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        plans::update_milestone(&self.sync_manager, params).await
    }

    #[tool(description = "Delete a milestone from a plan")]
    async fn milestones_delete(
        &self,
        Parameters(params): Parameters<MilestoneDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        plans::delete_milestone(&self.sync_manager, params).await
    }

    // ---- Expense tools ----

    #[tool(description = "List all expense events in a plan with amounts, frequencies, and date ranges")]
    async fn expenses_list(
        &self,
        Parameters(params): Parameters<PlanEventsListParams>,
    ) -> Result<CallToolResult, McpError> {
        events::expenses_list(&self.sync_manager, params).await
    }

    #[tool(description = "Create a new expense event in a plan. Requires at minimum: name, amount, type, and timing fields.")]
    async fn expenses_create(
        &self,
        Parameters(params): Parameters<PlanEventCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        events::expenses_create(&self.sync_manager, params).await
    }

    #[tool(description = "Update an existing expense event in a plan (partial updates supported)")]
    async fn expenses_update(
        &self,
        Parameters(params): Parameters<PlanEventUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        events::expenses_update(&self.sync_manager, params).await
    }

    #[tool(description = "Delete an expense event from a plan")]
    async fn expenses_delete(
        &self,
        Parameters(params): Parameters<PlanEventDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        events::expenses_delete(&self.sync_manager, params).await
    }

    // ---- Income tools ----

    #[tool(description = "List all income events in a plan with amounts, frequencies, and tax settings")]
    async fn income_list(
        &self,
        Parameters(params): Parameters<PlanEventsListParams>,
    ) -> Result<CallToolResult, McpError> {
        events::income_list(&self.sync_manager, params).await
    }

    #[tool(description = "Create a new income event in a plan. Requires at minimum: name, amount, type, and timing fields.")]
    async fn income_create(
        &self,
        Parameters(params): Parameters<PlanEventCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        events::income_create(&self.sync_manager, params).await
    }

    #[tool(description = "Update an existing income event in a plan (partial updates supported)")]
    async fn income_update(
        &self,
        Parameters(params): Parameters<PlanEventUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        events::income_update(&self.sync_manager, params).await
    }

    #[tool(description = "Delete an income event from a plan")]
    async fn income_delete(
        &self,
        Parameters(params): Parameters<PlanEventDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        events::income_delete(&self.sync_manager, params).await
    }

    // ---- Priority tools ----

    #[tool(description = "List all priority/goal events in a plan (401k contributions, savings goals, etc.)")]
    async fn priorities_list(
        &self,
        Parameters(params): Parameters<PlanEventsListParams>,
    ) -> Result<CallToolResult, McpError> {
        events::priorities_list(&self.sync_manager, params).await
    }

    #[tool(description = "Create a new priority/goal event in a plan (e.g., 401k contribution, savings goal)")]
    async fn priorities_create(
        &self,
        Parameters(params): Parameters<PlanEventCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        events::priorities_create(&self.sync_manager, params).await
    }

    #[tool(description = "Update an existing priority/goal event in a plan (partial updates supported)")]
    async fn priorities_update(
        &self,
        Parameters(params): Parameters<PlanEventUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        events::priorities_update(&self.sync_manager, params).await
    }

    #[tool(description = "Delete a priority/goal event from a plan")]
    async fn priorities_delete(
        &self,
        Parameters(params): Parameters<PlanEventDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        events::priorities_delete(&self.sync_manager, params).await
    }

    // ---- Plan Asset tools ----

    #[tool(description = "List all planned asset events in a plan (future home purchases, car purchases, etc.)")]
    async fn plan_assets_list(
        &self,
        Parameters(params): Parameters<PlanEventsListParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_assets::list(&self.sync_manager, params).await
    }

    #[tool(description = "Create a new asset event in a plan (e.g., buy a house, buy a car). Requires timing, value, and loan fields.")]
    async fn plan_assets_create(
        &self,
        Parameters(params): Parameters<PlanEventCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_assets::create(&self.sync_manager, params).await
    }

    #[tool(description = "Update a planned asset event in a plan (partial updates supported). Use this to change house purchase price, down payment, mortgage rate, cabin price, car prices, etc.")]
    async fn plan_assets_update(
        &self,
        Parameters(params): Parameters<PlanEventUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_assets::update(&self.sync_manager, params).await
    }

    #[tool(description = "Delete a planned asset event from a plan")]
    async fn plan_assets_delete(
        &self,
        Parameters(params): Parameters<PlanEventDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_assets::delete(&self.sync_manager, params).await
    }

    // ---- Plan Account tools ----

    #[tool(description = "List all plan-level account events in a plan (account overrides, balances, growth rates)")]
    async fn plan_accounts_list(
        &self,
        Parameters(params): Parameters<PlanEventsListParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_accounts::list(&self.sync_manager, params).await
    }

    #[tool(description = "Update a plan-level account event (partial updates supported). Use this to change account growth rates, balances, or withdrawal settings within a plan.")]
    async fn plan_accounts_update(
        &self,
        Parameters(params): Parameters<PlanEventUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        plan_accounts::update(&self.sync_manager, params).await
    }

    // ---- Progress tools ----

    #[tool(description = "Add a net worth data point to progress history. Useful for periodic snapshots from external account data (e.g., Monarch Money).")]
    async fn progress_add_data_point(
        &self,
        Parameters(params): Parameters<ProgressAddDataPointParams>,
    ) -> Result<CallToolResult, McpError> {
        progress::add_data_point(&self.sync_manager, params).await
    }

    #[tool(description = "Get progress history (net worth over time), optionally filtered by date range")]
    async fn progress_get_history(
        &self,
        Parameters(params): Parameters<ProgressGetHistoryParams>,
    ) -> Result<CallToolResult, McpError> {
        progress::get_history(&self.sync_manager, params).await
    }

    // ---- Browser / Simulation tools ----

    #[tool(description = "Execute JavaScript in the ProjectionLab browser context. Useful for exploring Vue app internals, accessing Pinia stores, or extracting data not available through the Plugin API. Optionally navigate to a path first.")]
    async fn run_js_in_browser(
        &self,
        Parameters(params): Parameters<RunJsInBrowserParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut guard = self.get_browser().await?;
        let browser = guard.as_mut().ok_or_else(|| {
            McpError::internal_error("Browser session not initialized", None)
        })?;

        let navigated = if let Some(path) = &params.navigate_to {
            browser.navigate_to(path).await.map_err(|e| {
                McpError::internal_error(format!("Navigation failed: {}", e), None)
            })?;
            true
        } else {
            false
        };

        let result = if params.r#async {
            browser.execute_js_async(&params.script).await
        } else {
            browser.execute_js(&params.script).await
        }
        .map_err(|e| McpError::internal_error(format!("JS execution failed: {}", e), None))?;

        if navigated {
            browser.navigate_to_home().await.map_err(|e| {
                McpError::internal_error(format!("Failed to navigate home: {}", e), None)
            })?;
        }

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| format!("{:?}", result)),
        )]))
    }

    #[tool(description = "Get simulation/projection results for a plan. Navigates to the plan page, waits for simulation, and returns: year-by-year net worth/income/expenses/contributions/drawdown, milestone completion dates, notable events, and outcome status. Data comes from plan._runtime.results in the Pinia store.")]
    async fn plans_get_simulation_results(
        &self,
        Parameters(params): Parameters<PlanGetParams>,
    ) -> Result<CallToolResult, McpError> {
        self.run_plan_js(&params.plan_id, simulation::SIMULATION_RESULTS_JS).await
    }

    #[tool(description = "Get Monte Carlo simulation results for a plan. Returns trial outcome distribution, percentile plots, and milestone probability table from the monte-carlo Pinia store.")]
    async fn plans_get_montecarlo_results(
        &self,
        Parameters(params): Parameters<PlanGetParams>,
    ) -> Result<CallToolResult, McpError> {
        self.run_plan_js(&params.plan_id, simulation::MONTECARLO_RESULTS_JS).await
    }

    #[tool(description = "Get a detailed financial snapshot at a specific age from simulation results. Returns net worth, income, expenses, taxes, contributions, drawdown, debt payments, ending balances, and all summary categories for that year.")]
    async fn plans_get_year_snapshot(
        &self,
        Parameters(params): Parameters<YearSnapshotParams>,
    ) -> Result<CallToolResult, McpError> {
        let script = simulation::year_snapshot_js(params.age);
        self.run_plan_js(&params.plan_id, &script).await
    }

    #[tool(description = "Get year-by-year financial data and deltas for an age range from simulation results. Returns yearly net worth, income, expenses, taxes, contributions, drawdown with year-over-year changes and summary statistics.")]
    async fn plans_get_year_range(
        &self,
        Parameters(params): Parameters<YearRangeParams>,
    ) -> Result<CallToolResult, McpError> {
        if params.start_age >= params.end_age {
            return Err(McpError::internal_error(
                "start_age must be less than end_age",
                None,
            ));
        }
        let script = simulation::year_range_js(params.start_age, params.end_age);
        self.run_plan_js(&params.plan_id, &script).await
    }

    // ---- Integration tools ----

    #[tool(description = "Batch-update ProjectionLab account balances from external data (e.g., Monarch Money). Accepts a list of {pl_account_id, balance} mappings. Updates all matching accounts in a single operation.")]
    async fn sync_account_balances(
        &self,
        Parameters(params): Parameters<SyncAccountBalancesParams>,
    ) -> Result<CallToolResult, McpError> {
        accounts::sync_balances(&self.sync_manager, params).await
    }

    #[tool(description = "Record a net worth snapshot to progress history. Convenience wrapper for progress_add_data_point that uses today's date. Pass category breakdowns computed from external account data (e.g., Monarch Money).")]
    async fn snapshot_net_worth(
        &self,
        Parameters(params): Parameters<ProgressAddDataPointParams>,
    ) -> Result<CallToolResult, McpError> {
        self.progress_add_data_point(Parameters(params)).await
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
        self.sync_manager
            .refresh()
            .await
            .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            "Cache refreshed successfully".to_string()
        )]))
    }

    #[tool(description = "Get the age of the cached data for debugging")]
    async fn cache_status(&self) -> Result<CallToolResult, McpError> {
        match self.sync_manager.cache_age().await {
            Some(age) => Ok(CallToolResult::success(vec![Content::text(format!(
                "Cache age: {:.2} seconds",
                age.as_secs_f64()
            ))])),
            None => Ok(CallToolResult::success(vec![Content::text(
                "Cache is empty".to_string()
            )])),
        }
    }

    #[tool(description = "Validate that the cached API key is still valid")]
    async fn validate_api_key(&self) -> Result<CallToolResult, McpError> {
        let browser_guard = self.get_browser().await?;
        let browser = browser_guard
            .as_ref()
            .ok_or_else(|| McpError::internal_error("Browser session not initialized", None))?;

        let result = browser
            .call_plugin_api("validateApiKey", vec![])
            .await
            .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "API key validation result: {}",
            serde_json::to_string_pretty(&result).unwrap_or_default()
        ))]))
    }
}

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
                website_url: Some("https://github.com/yourusername/projectionlab-mcp".to_string()),
                icons: None,
            },
            instructions: Some(
                "ProjectionLab MCP Server - AI financial advisor tooling for ProjectionLab.\n\n\
                Resources (read-only structured access):\n\
                - projectionlab://overview - Net worth, account counts, active plans\n\
                - projectionlab://accounts/summary - All accounts with balances\n\
                - projectionlab://plans/summary - All plans with key metrics\n\
                - projectionlab://expenses/summary - Expenses across active plans\n\
                - projectionlab://income/summary - Income across active plans\n\
                - projectionlab://net-worth/history - Progress data points over time\n\
                - projectionlab://plan/{id}/variables - Plan assumptions\n\
                - projectionlab://plan/{id}/milestones - Plan milestones\n\n\
                Tools by category:\n\
                Accounts: accounts_list, accounts_get, accounts_create, accounts_update, accounts_update_balance, accounts_delete\n\
                Starting Assets: starting_assets_list, starting_assets_get, starting_assets_create, starting_assets_update, starting_assets_delete\n\
                Debts: debts_list, debts_get, debts_create, debts_update, debts_delete\n\
                Plans: plans_list, plans_get, plans_create, plans_delete, plans_get_variables, plans_update_variables, plans_update_metadata, plans_clone, plans_get_milestones\n\
                Milestones: milestones_create, milestones_update, milestones_delete\n\
                Expenses: expenses_list, expenses_create, expenses_update, expenses_delete\n\
                Income: income_list, income_create, income_update, income_delete\n\
                Priorities: priorities_list, priorities_create, priorities_update, priorities_delete\n\
                Plan Assets: plan_assets_list, plan_assets_create, plan_assets_update, plan_assets_delete\n\
                Plan Accounts: plan_accounts_list, plan_accounts_update\n\
                Progress: progress_add_data_point, progress_get_history\n\
                Simulation: plans_get_simulation_results, plans_get_montecarlo_results, plans_get_year_snapshot, plans_get_year_range\n\
                Browser: run_js_in_browser\n\
                Integration: sync_account_balances, snapshot_net_worth\n\
                Schema: schema_help — call with topic='topics' to see all available schema docs (date_or_milestone, yearly_change, expense, income, priority, account, asset, etc.)\n\
                System: refresh_cache, cache_status, validate_api_key\n\
                Raw API: update_account, export_data, restore_current_finances, restore_plans, restore_progress, restore_settings\n\n\
                The server launches Firefox on first connection. Session persists between restarts."
                    .to_string(),
            ),
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
            .map_err(|e| McpError::internal_error(format!("Failed to read resource: {}", e), None))
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        info!("Initializing ProjectionLab MCP Server...");

        let browser = self.browser.clone();
        tokio::spawn(async move {
            let mut guard = browser.lock().await;
            if guard.is_none() {
                info!("Creating browser session in background...");
                match BrowserSession::new().await {
                    Ok(session) => {
                        *guard = Some(session);
                        info!("Browser session ready!");
                    }
                    Err(e) => {
                        error!("Failed to initialize browser session: {}", e);
                    }
                }
            }
        });

        Ok(self.get_info())
    }
}
