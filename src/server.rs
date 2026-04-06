use crate::browser::BrowserSession;
use crate::resources::ResourceProvider;
use crate::sync::SyncManager;
use crate::tools::params::*;
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    service::RequestContext,
    tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler,
};
use serde_json::{json, Value as JsonValue};
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

    /// Helper to get browser session or return error
    async fn get_browser(
        &self,
    ) -> Result<tokio::sync::MutexGuard<'_, Option<BrowserSession>>, McpError> {
        Ok(self.browser.lock().await)
    }

    /// Navigate to a plan page, wait for simulation, execute async JS, and return the result.
    /// Skips navigation if the browser is already on the same plan page.
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

        // Wait for the simulation to compute after navigation
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let result = browser.execute_js_async(script).await.map_err(|e| {
            McpError::internal_error(format!("JS execution failed: {}", e), None)
        })?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| format!("{:?}", result)),
        )]))
    }

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
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

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
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

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
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

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
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

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
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

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
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            "Settings restored successfully".to_string()
        )]))
    }

    #[tool(description = "List all accounts with optional filtering by type or owner")]
    async fn accounts_list(
        &self,
        Parameters(params): Parameters<AccountsListParams>,
    ) -> Result<CallToolResult, McpError> {
        let data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let mut accounts = Vec::new();

        // Collect savings accounts
        for account in &data.today.savings_accounts {
            if let Some(ref filter_type) = params.account_type {
                if filter_type != "savings" {
                    continue;
                }
            }
            if let Some(ref filter_owner) = params.owner {
                let owner_str = format!("{:?}", account.owner).to_lowercase();
                if &owner_str != filter_owner {
                    continue;
                }
            }
            accounts.push(json!({
                "id": account.id,
                "name": account.name,
                "type": "savings",
                "balance": account.balance,
                "owner": account.owner,
                "liquid": account.liquid,
            }));
        }

        // Collect investment accounts
        for account in &data.today.investment_accounts {
            if let Some(ref filter_type) = params.account_type {
                if filter_type != "investment" && filter_type != &account.account_type {
                    continue;
                }
            }
            if let Some(ref filter_owner) = params.owner {
                let owner_str = format!("{:?}", account.owner).to_lowercase();
                if &owner_str != filter_owner {
                    continue;
                }
            }
            accounts.push(json!({
                "id": account.id,
                "name": account.name,
                "type": account.account_type,
                "balance": account.balance,
                "owner": account.owner,
                "liquid": account.liquid,
                "cost_basis": account.cost_basis,
            }));
        }

        // Collect debt accounts
        for debt in &data.today.debts {
            if let Some(ref filter_type) = params.account_type {
                if filter_type != "debt" && filter_type != &debt.debt_type {
                    continue;
                }
            }
            if let Some(ref filter_owner) = params.owner {
                let owner_str = format!("{:?}", debt.owner).to_lowercase();
                if &owner_str != filter_owner {
                    continue;
                }
            }
            accounts.push(json!({
                "id": debt.id,
                "name": debt.name,
                "type": debt.debt_type,
                "balance": debt.balance,
                "owner": debt.owner,
                "interest_rate": debt.interest_rate,
                "monthly_payment": debt.monthly_payment,
            }));
        }

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "total_count": accounts.len(),
                "accounts": accounts,
            }))
            .unwrap_or_default(),
        )]))
    }

    #[tool(description = "Get detailed information about a specific account by ID")]
    async fn accounts_get(
        &self,
        Parameters(params): Parameters<AccountsGetParams>,
    ) -> Result<CallToolResult, McpError> {
        let data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        // Search in savings accounts
        if let Some(account) = data
            .today
            .savings_accounts
            .iter()
            .find(|a| a.id == params.account_id)
        {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&json!({
                    "id": account.id,
                    "name": account.name,
                    "type": "savings",
                    "balance": account.balance,
                    "owner": account.owner,
                    "liquid": account.liquid,
                    "icon": account.icon,
                    "color": account.color,
                }))
                .unwrap_or_default(),
            )]));
        }

        // Search in investment accounts
        if let Some(account) = data
            .today
            .investment_accounts
            .iter()
            .find(|a| a.id == params.account_id)
        {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&json!({
                    "id": account.id,
                    "name": account.name,
                    "type": account.account_type,
                    "balance": account.balance,
                    "owner": account.owner,
                    "liquid": account.liquid,
                    "icon": account.icon,
                    "color": account.color,
                    "cost_basis": account.cost_basis,
                }))
                .unwrap_or_default(),
            )]));
        }

        // Search in debt accounts
        if let Some(debt) = data
            .today
            .debts
            .iter()
            .find(|d| d.id == params.account_id)
        {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&json!({
                    "id": debt.id,
                    "name": debt.name,
                    "type": debt.debt_type,
                    "balance": debt.balance,
                    "owner": debt.owner,
                    "icon": debt.icon,
                    "color": debt.color,
                    "interest_rate": debt.interest_rate,
                    "monthly_payment": debt.monthly_payment,
                }))
                .unwrap_or_default(),
            )]));
        }

        Err(McpError::internal_error(
            format!("Account not found: {}", params.account_id),
            None,
        ))
    }

    #[tool(description = "Create a new account (savings, investment, or debt)")]
    async fn accounts_create(
        &self,
        Parameters(params): Parameters<AccountsCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        // Generate a new ID if not provided
        let account_id = params
            .data
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("acc_{}", chrono::Utc::now().timestamp_millis()));

        // Create the new account JSON with the provided data
        let mut new_account = params.data.clone();
        new_account.insert("id".to_string(), json!(account_id));

        // Add to the appropriate list based on account_type
        match params.account_type.as_str() {
            "savings" => {
                let account: crate::models::SavingsAccount =
                    serde_json::from_value(json!(new_account)).map_err(|e| {
                        McpError::internal_error(
                            format!("Invalid savings account data: {}", e),
                            None,
                        )
                    })?;
                data.today.savings_accounts.push(account);
            }
            "investment" => {
                let account: crate::models::InvestmentAccount =
                    serde_json::from_value(json!(new_account)).map_err(|e| {
                        McpError::internal_error(
                            format!("Invalid investment account data: {}", e),
                            None,
                        )
                    })?;
                data.today.investment_accounts.push(account);
            }
            "debt" => {
                let debt: crate::models::DebtAccount =
                    serde_json::from_value(json!(new_account)).map_err(|e| {
                        McpError::internal_error(
                            format!("Invalid debt account data: {}", e),
                            None,
                        )
                    })?;
                data.today.debts.push(debt);
            }
            _ => {
                return Err(McpError::internal_error(
                    format!(
                        "Invalid account type: {}. Must be 'savings', 'investment', or 'debt'",
                        params.account_type
                    ),
                    None,
                ));
            }
        }

        // Serialize the updated CurrentFinances state
        let new_finances = serde_json::to_value(&data.today).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize finances: {}", e), None)
        })?;

        // Update via sync manager (invalidates cache)
        self.sync_manager
            .update_current_finances(new_finances)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Account created successfully with ID: {}",
            account_id
        ))]))
    }

    #[tool(
        description = "Update an existing account with partial data (preserves unspecified fields)"
    )]
    async fn accounts_update(
        &self,
        Parameters(params): Parameters<AccountsUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let mut found = false;

        // Try to update in savings accounts
        if let Some(account) = data
            .today
            .savings_accounts
            .iter_mut()
            .find(|a| a.id == params.account_id)
        {
            // Serialize existing account to JSON, merge with updates, then deserialize back
            let mut account_json = serde_json::to_value(&*account).map_err(|e| {
                McpError::internal_error(format!("Failed to serialize account: {}", e), None)
            })?;

            if let Some(obj) = account_json.as_object_mut() {
                for (key, value) in &params.data {
                    obj.insert(key.clone(), value.clone());
                }
            }

            *account = serde_json::from_value(account_json).map_err(|e| {
                McpError::internal_error(
                    format!("Failed to deserialize updated account: {}", e),
                    None,
                )
            })?;
            found = true;
        }

        // Try to update in investment accounts
        if !found {
            if let Some(account) = data
                .today
                .investment_accounts
                .iter_mut()
                .find(|a| a.id == params.account_id)
            {
                let mut account_json = serde_json::to_value(&*account).map_err(|e| {
                    McpError::internal_error(format!("Failed to serialize account: {}", e), None)
                })?;

                if let Some(obj) = account_json.as_object_mut() {
                    for (key, value) in &params.data {
                        obj.insert(key.clone(), value.clone());
                    }
                }

                *account = serde_json::from_value(account_json).map_err(|e| {
                    McpError::internal_error(
                        format!("Failed to deserialize updated account: {}", e),
                        None,
                    )
                })?;
                found = true;
            }
        }

        // Try to update in debt accounts
        if !found {
            if let Some(debt) = data
                .today
                .debts
                .iter_mut()
                .find(|d| d.id == params.account_id)
            {
                let mut debt_json = serde_json::to_value(&*debt).map_err(|e| {
                    McpError::internal_error(format!("Failed to serialize debt: {}", e), None)
                })?;

                if let Some(obj) = debt_json.as_object_mut() {
                    for (key, value) in &params.data {
                        obj.insert(key.clone(), value.clone());
                    }
                }

                *debt = serde_json::from_value(debt_json).map_err(|e| {
                    McpError::internal_error(
                        format!("Failed to deserialize updated debt: {}", e),
                        None,
                    )
                })?;
                found = true;
            }
        }

        if !found {
            return Err(McpError::internal_error(
                format!("Account not found: {}", params.account_id),
                None,
            ));
        }

        // Serialize the updated CurrentFinances state
        let new_finances = serde_json::to_value(&data.today).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize finances: {}", e), None)
        })?;

        // Update via sync manager (invalidates cache)
        self.sync_manager
            .update_current_finances(new_finances)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Account {} updated successfully",
            params.account_id
        ))]))
    }

    #[tool(description = "Convenience method to update just the balance of an account")]
    async fn accounts_update_balance(
        &self,
        Parameters(params): Parameters<AccountsUpdateBalanceParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let mut found = false;

        // Try to update in savings accounts
        if let Some(account) = data
            .today
            .savings_accounts
            .iter_mut()
            .find(|a| a.id == params.account_id)
        {
            account.balance = params.balance;
            found = true;
        }

        // Try to update in investment accounts
        if !found {
            if let Some(account) = data
                .today
                .investment_accounts
                .iter_mut()
                .find(|a| a.id == params.account_id)
            {
                account.balance = params.balance;
                found = true;
            }
        }

        // Try to update in debt accounts
        if !found {
            if let Some(debt) = data
                .today
                .debts
                .iter_mut()
                .find(|d| d.id == params.account_id)
            {
                debt.balance = params.balance;
                found = true;
            }
        }

        if !found {
            return Err(McpError::internal_error(
                format!("Account not found: {}", params.account_id),
                None,
            ));
        }

        // Serialize the updated CurrentFinances state
        let new_finances = serde_json::to_value(&data.today).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize finances: {}", e), None)
        })?;

        // Update via sync manager (invalidates cache)
        self.sync_manager
            .update_current_finances(new_finances)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Account {} balance updated to {}",
            params.account_id, params.balance
        ))]))
    }

    #[tool(description = "Delete an account by ID")]
    async fn accounts_delete(
        &self,
        Parameters(params): Parameters<AccountsDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let mut found = false;

        // Try to delete from savings accounts
        if let Some(idx) = data
            .today
            .savings_accounts
            .iter()
            .position(|a| a.id == params.account_id)
        {
            data.today.savings_accounts.remove(idx);
            found = true;
        }

        // Try to delete from investment accounts
        if !found {
            if let Some(idx) = data
                .today
                .investment_accounts
                .iter()
                .position(|a| a.id == params.account_id)
            {
                data.today.investment_accounts.remove(idx);
                found = true;
            }
        }

        // Try to delete from debt accounts
        if !found {
            if let Some(idx) = data
                .today
                .debts
                .iter()
                .position(|d| d.id == params.account_id)
            {
                data.today.debts.remove(idx);
                found = true;
            }
        }

        if !found {
            return Err(McpError::internal_error(
                format!("Account not found: {}", params.account_id),
                None,
            ));
        }

        // Serialize the updated CurrentFinances state
        let new_finances = serde_json::to_value(&data.today).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize finances: {}", e), None)
        })?;

        // Update via sync manager (invalidates cache)
        self.sync_manager
            .update_current_finances(new_finances)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Account {} deleted successfully",
            params.account_id
        ))]))
    }

    // ---- Plan tools ----

    #[tool(description = "List all plans with key metadata (id, name, active status, event counts)")]
    async fn plans_list(&self) -> Result<CallToolResult, McpError> {
        let data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plans: Vec<JsonValue> = data
            .plans
            .iter()
            .map(|plan| {
                json!({
                    "id": plan.id,
                    "name": plan.name,
                    "icon": plan.icon,
                    "active": plan.active,
                    "last_updated": plan.last_updated,
                    "milestones": plan.milestones.len(),
                    "computed_milestones": plan.computed_milestones.len(),
                    "expenses": plan.expenses.events.len(),
                    "income": plan.income.events.len(),
                    "priorities": plan.priorities.events.len(),
                    "assets": plan.assets.events.len(),
                    "accounts": plan.accounts.events.len(),
                    "withdrawal_strategy": plan.withdrawal_strategy.strategy,
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "total_plans": plans.len(),
                "plans": plans,
            }))
            .unwrap_or_default(),
        )]))
    }

    #[tool(description = "Get detailed information about a specific plan including variables, milestones, withdrawal strategy, and Monte Carlo settings")]
    async fn plans_get(
        &self,
        Parameters(params): Parameters<PlanGetParams>,
    ) -> Result<CallToolResult, McpError> {
        let data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plan = data
            .plans
            .iter()
            .find(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&plan).unwrap_or_default(),
        )]))
    }

    #[tool(description = "Get just the variables/assumptions for a specific plan (investment returns, inflation, tax settings, etc.)")]
    async fn plans_get_variables(
        &self,
        Parameters(params): Parameters<PlanGetParams>,
    ) -> Result<CallToolResult, McpError> {
        let data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plan = data
            .plans
            .iter()
            .find(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "plan_id": plan.id,
                "plan_name": plan.name,
                "variables": plan.variables,
            }))
            .unwrap_or_default(),
        )]))
    }

    #[tool(description = "Update variables/assumptions for a specific plan (partial updates supported). Use this to adjust investment returns, inflation, tax settings, Monte Carlo parameters, etc.")]
    async fn plans_update_variables(
        &self,
        Parameters(params): Parameters<PlansUpdateVariablesParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plan = data
            .plans
            .iter_mut()
            .find(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        // Merge updates into existing variables
        let mut vars_json = serde_json::to_value(&plan.variables).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize variables: {}", e), None)
        })?;

        if let Some(obj) = vars_json.as_object_mut() {
            for (key, value) in &params.updates {
                obj.insert(key.clone(), value.clone());
            }
        }

        plan.variables = serde_json::from_value(vars_json).map_err(|e| {
            McpError::internal_error(
                format!("Failed to deserialize updated variables: {}", e),
                None,
            )
        })?;

        // Write all plans back via restorePlans
        let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
        })?;

        self.sync_manager
            .update_plans(plans_value)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Plan '{}' variables updated successfully",
            params.plan_id
        ))]))
    }

    #[tool(description = "Clone a plan for scenario comparison. Creates a deep copy with a new name and ID.")]
    async fn plans_clone(
        &self,
        Parameters(params): Parameters<PlansCloneParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let source = data
            .plans
            .iter()
            .find(|p| p.id == params.source_plan_id)
            .ok_or_else(|| {
                McpError::internal_error(
                    format!("Plan not found: {}", params.source_plan_id),
                    None,
                )
            })?
            .clone();

        let new_id = format!("plan_{}", chrono::Utc::now().timestamp_millis());
        let mut cloned = source;
        cloned.id = new_id.clone();
        cloned.name = params.new_name.clone();
        cloned.last_updated = chrono::Utc::now().timestamp_millis();

        data.plans.push(cloned);

        let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
        })?;

        self.sync_manager
            .update_plans(plans_value)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Plan cloned successfully. New plan '{}' with ID: {}",
            params.new_name, new_id
        ))]))
    }

    #[tool(description = "Create a new plan. Either creates an empty plan shell or clones from an existing plan. Use clone_from to base it on an existing plan's structure.")]
    async fn plans_create(
        &self,
        Parameters(params): Parameters<PlansCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let new_id = format!("plan_{}", chrono::Utc::now().timestamp_millis());
        let now = chrono::Utc::now().timestamp_millis();

        let new_plan = if let Some(source_id) = &params.clone_from {
            let source = data
                .plans
                .iter()
                .find(|p| p.id == *source_id)
                .ok_or_else(|| {
                    McpError::internal_error(
                        format!("Source plan not found: {}", source_id),
                        None,
                    )
                })?
                .clone();

            let mut cloned = source;
            cloned.id = new_id.clone();
            cloned.name = params.name.clone();
            cloned.icon = params.icon.clone();
            cloned.last_updated = now;
            cloned
        } else {
            // Create a minimal plan by cloning the first available plan as a template,
            // then clearing its events. This ensures we get a valid schema structure.
            let template = data.plans.first().ok_or_else(|| {
                McpError::internal_error(
                    "No existing plans to use as template. Use clone_from with an existing plan ID.",
                    None,
                )
            })?;

            let mut new_plan = template.clone();
            new_plan.id = new_id.clone();
            new_plan.name = params.name.clone();
            new_plan.icon = params.icon.clone();
            new_plan.active = true;
            new_plan.last_updated = now;
            // Clear all events to start fresh
            new_plan.expenses.events.clear();
            new_plan.income.events.clear();
            new_plan.priorities.events.clear();
            new_plan.assets.events.clear();
            new_plan.accounts.events.clear();
            new_plan.milestones.clear();
            new_plan.computed_milestones.clear();
            new_plan
        };

        data.plans.push(new_plan);

        let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
        })?;

        self.sync_manager
            .update_plans(plans_value)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Plan '{}' created successfully with ID: {}",
            params.name, new_id
        ))]))
    }

    #[tool(description = "Delete a plan permanently. Requires confirm=true as a safety check. This cannot be undone.")]
    async fn plans_delete(
        &self,
        Parameters(params): Parameters<PlansDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        if !params.confirm {
            return Err(McpError::internal_error(
                "Deletion not confirmed. Set confirm=true to delete the plan.",
                None,
            ));
        }

        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let idx = data
            .plans
            .iter()
            .position(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        let removed_name = data.plans[idx].name.clone();
        data.plans.remove(idx);

        let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
        })?;

        self.sync_manager
            .update_plans(plans_value)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Plan '{}' ({}) deleted successfully",
            removed_name, params.plan_id
        ))]))
    }

    #[tool(description = "Get milestones and computed milestones for a specific plan (retirement dates, financial independence targets, goal timelines)")]
    async fn plans_get_milestones(
        &self,
        Parameters(params): Parameters<PlanGetParams>,
    ) -> Result<CallToolResult, McpError> {
        let data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plan = data
            .plans
            .iter()
            .find(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "plan_id": plan.id,
                "plan_name": plan.name,
                "milestones": plan.milestones,
                "computed_milestones": plan.computed_milestones,
            }))
            .unwrap_or_default(),
        )]))
    }

    // ---- Milestone tools ----

    #[tool(description = "Create a new milestone in a plan. Requires: name, icon, color, criteria array. Each criterion has type ('year', 'milestone', 'account', 'loan'), value, and optional modifier/operator/ref_id fields.")]
    async fn milestones_create(
        &self,
        Parameters(params): Parameters<MilestoneCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plan = data
            .plans
            .iter_mut()
            .find(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        let mut milestone_data = params.data;
        let milestone_id = milestone_data
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("ms_{}", chrono::Utc::now().timestamp_millis()));
        milestone_data.insert("id".to_string(), json!(milestone_id));

        let milestone: crate::models::Milestone =
            serde_json::from_value(json!(milestone_data)).map_err(|e| {
                McpError::internal_error(format!("Invalid milestone data: {}", e), None)
            })?;

        plan.milestones.push(milestone);

        let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
        })?;

        self.sync_manager
            .update_plans(plans_value)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Milestone created successfully with ID: {}",
            milestone_id
        ))]))
    }

    #[tool(description = "Update an existing milestone in a plan (partial updates supported). Can modify name, icon, color, criteria, hidden, etc.")]
    async fn milestones_update(
        &self,
        Parameters(params): Parameters<MilestoneUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plan = data
            .plans
            .iter_mut()
            .find(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        let milestone = plan
            .milestones
            .iter_mut()
            .find(|m| m.id == params.milestone_id)
            .ok_or_else(|| {
                McpError::internal_error(
                    format!("Milestone not found: {}", params.milestone_id),
                    None,
                )
            })?;

        let mut milestone_json = serde_json::to_value(&*milestone).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize milestone: {}", e), None)
        })?;

        if let Some(obj) = milestone_json.as_object_mut() {
            for (key, value) in &params.data {
                obj.insert(key.clone(), value.clone());
            }
        }

        *milestone = serde_json::from_value(milestone_json).map_err(|e| {
            McpError::internal_error(
                format!("Failed to deserialize updated milestone: {}", e),
                None,
            )
        })?;

        let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
        })?;

        self.sync_manager
            .update_plans(plans_value)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Milestone '{}' updated successfully",
            params.milestone_id
        ))]))
    }

    #[tool(description = "Delete a milestone from a plan")]
    async fn milestones_delete(
        &self,
        Parameters(params): Parameters<MilestoneDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plan = data
            .plans
            .iter_mut()
            .find(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        let idx = plan
            .milestones
            .iter()
            .position(|m| m.id == params.milestone_id)
            .ok_or_else(|| {
                McpError::internal_error(
                    format!("Milestone not found: {}", params.milestone_id),
                    None,
                )
            })?;

        plan.milestones.remove(idx);

        let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
        })?;

        self.sync_manager
            .update_plans(plans_value)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Milestone '{}' deleted successfully",
            params.milestone_id
        ))]))
    }

    // ---- Plan metadata tools ----

    #[tool(description = "Update plan metadata: name, icon, or active status. Only these top-level fields can be changed — use dedicated tools for expenses, income, milestones, variables, etc.")]
    async fn plans_update_metadata(
        &self,
        Parameters(params): Parameters<PlansUpdateMetadataParams>,
    ) -> Result<CallToolResult, McpError> {
        const ALLOWED_KEYS: &[&str] = &["name", "icon", "active"];

        // Reject disallowed keys
        for key in params.updates.keys() {
            if !ALLOWED_KEYS.contains(&key.as_str()) {
                return Err(McpError::internal_error(
                    format!(
                        "Key '{}' is not allowed in metadata updates. Allowed keys: {}",
                        key,
                        ALLOWED_KEYS.join(", ")
                    ),
                    None,
                ));
            }
        }

        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plan = data
            .plans
            .iter_mut()
            .find(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        // Apply allowed updates
        if let Some(name) = params.updates.get("name").and_then(|v| v.as_str()) {
            plan.name = name.to_string();
        }
        if let Some(icon) = params.updates.get("icon").and_then(|v| v.as_str()) {
            plan.icon = icon.to_string();
        }
        if let Some(active) = params.updates.get("active").and_then(|v| v.as_bool()) {
            plan.active = active;
        }

        let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
        })?;

        self.sync_manager
            .update_plans(plans_value)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Plan '{}' metadata updated successfully",
            params.plan_id
        ))]))
    }

    // ---- Expense tools ----

    #[tool(description = "List all expense events in a plan with amounts, frequencies, and date ranges")]
    async fn expenses_list(
        &self,
        Parameters(params): Parameters<PlanEventsListParams>,
    ) -> Result<CallToolResult, McpError> {
        let data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plan = data
            .plans
            .iter()
            .find(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        let expenses: Vec<JsonValue> = plan
            .expenses
            .events
            .iter()
            .map(|e| {
                json!({
                    "id": e.id,
                    "name": e.name,
                    "type": e.event_type,
                    "amount": e.amount,
                    "amount_type": e.amount_type,
                    "frequency": e.frequency,
                    "owner": e.owner,
                    "start": e.start,
                    "end": e.end,
                    "spending_type": e.spending_type,
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "plan_id": plan.id,
                "plan_name": plan.name,
                "total_expenses": expenses.len(),
                "expenses": expenses,
            }))
            .unwrap_or_default(),
        )]))
    }

    #[tool(description = "Create a new expense event in a plan. Requires at minimum: name, amount, type, and timing fields.")]
    async fn expenses_create(
        &self,
        Parameters(params): Parameters<PlanEventCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plan = data
            .plans
            .iter_mut()
            .find(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        let mut event_data = params.data.clone();
        let event_id = event_data
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("exp_{}", chrono::Utc::now().timestamp_millis()));
        event_data.insert("id".to_string(), json!(event_id));

        let expense: crate::models::ExpenseEvent =
            serde_json::from_value(json!(event_data)).map_err(|e| {
                McpError::internal_error(format!("Invalid expense data: {}", e), None)
            })?;

        plan.expenses.events.push(expense);

        let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
        })?;

        self.sync_manager
            .update_plans(plans_value)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Expense created successfully with ID: {}",
            event_id
        ))]))
    }

    #[tool(description = "Update an existing expense event in a plan (partial updates supported)")]
    async fn expenses_update(
        &self,
        Parameters(params): Parameters<PlanEventUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plan = data
            .plans
            .iter_mut()
            .find(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        let expense = plan
            .expenses
            .events
            .iter_mut()
            .find(|e| e.id == params.event_id)
            .ok_or_else(|| {
                McpError::internal_error(
                    format!("Expense not found: {}", params.event_id),
                    None,
                )
            })?;

        let mut expense_json = serde_json::to_value(&*expense).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize expense: {}", e), None)
        })?;

        if let Some(obj) = expense_json.as_object_mut() {
            for (key, value) in &params.data {
                obj.insert(key.clone(), value.clone());
            }
        }

        *expense = serde_json::from_value(expense_json).map_err(|e| {
            McpError::internal_error(
                format!("Failed to deserialize updated expense: {}", e),
                None,
            )
        })?;

        let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
        })?;

        self.sync_manager
            .update_plans(plans_value)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Expense '{}' updated successfully",
            params.event_id
        ))]))
    }

    #[tool(description = "Delete an expense event from a plan")]
    async fn expenses_delete(
        &self,
        Parameters(params): Parameters<PlanEventDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plan = data
            .plans
            .iter_mut()
            .find(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        let idx = plan
            .expenses
            .events
            .iter()
            .position(|e| e.id == params.event_id)
            .ok_or_else(|| {
                McpError::internal_error(
                    format!("Expense not found: {}", params.event_id),
                    None,
                )
            })?;

        plan.expenses.events.remove(idx);

        let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
        })?;

        self.sync_manager
            .update_plans(plans_value)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Expense '{}' deleted successfully",
            params.event_id
        ))]))
    }

    // ---- Income tools ----

    #[tool(description = "List all income events in a plan with amounts, frequencies, and tax settings")]
    async fn income_list(
        &self,
        Parameters(params): Parameters<PlanEventsListParams>,
    ) -> Result<CallToolResult, McpError> {
        let data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plan = data
            .plans
            .iter()
            .find(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        let income: Vec<JsonValue> = plan
            .income
            .events
            .iter()
            .map(|i| {
                json!({
                    "id": i.id,
                    "name": i.name,
                    "type": i.event_type,
                    "amount": i.amount,
                    "amount_type": i.amount_type,
                    "frequency": i.frequency,
                    "owner": i.owner,
                    "start": i.start,
                    "end": i.end,
                    "tax_withholding": i.tax_withholding,
                    "withhold": i.withhold,
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "plan_id": plan.id,
                "plan_name": plan.name,
                "total_income": income.len(),
                "income": income,
            }))
            .unwrap_or_default(),
        )]))
    }

    #[tool(description = "Create a new income event in a plan. Requires at minimum: name, amount, type, and timing fields.")]
    async fn income_create(
        &self,
        Parameters(params): Parameters<PlanEventCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plan = data
            .plans
            .iter_mut()
            .find(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        let mut event_data = params.data.clone();
        let event_id = event_data
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("inc_{}", chrono::Utc::now().timestamp_millis()));
        event_data.insert("id".to_string(), json!(event_id));

        let income: crate::models::IncomeEvent =
            serde_json::from_value(json!(event_data)).map_err(|e| {
                McpError::internal_error(format!("Invalid income data: {}", e), None)
            })?;

        plan.income.events.push(income);

        let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
        })?;

        self.sync_manager
            .update_plans(plans_value)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Income event created successfully with ID: {}",
            event_id
        ))]))
    }

    #[tool(description = "Update an existing income event in a plan (partial updates supported)")]
    async fn income_update(
        &self,
        Parameters(params): Parameters<PlanEventUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plan = data
            .plans
            .iter_mut()
            .find(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        let income = plan
            .income
            .events
            .iter_mut()
            .find(|i| i.id == params.event_id)
            .ok_or_else(|| {
                McpError::internal_error(
                    format!("Income event not found: {}", params.event_id),
                    None,
                )
            })?;

        let mut income_json = serde_json::to_value(&*income).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize income: {}", e), None)
        })?;

        if let Some(obj) = income_json.as_object_mut() {
            for (key, value) in &params.data {
                obj.insert(key.clone(), value.clone());
            }
        }

        *income = serde_json::from_value(income_json).map_err(|e| {
            McpError::internal_error(
                format!("Failed to deserialize updated income: {}", e),
                None,
            )
        })?;

        let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
        })?;

        self.sync_manager
            .update_plans(plans_value)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Income event '{}' updated successfully",
            params.event_id
        ))]))
    }

    #[tool(description = "Delete an income event from a plan")]
    async fn income_delete(
        &self,
        Parameters(params): Parameters<PlanEventDeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plan = data
            .plans
            .iter_mut()
            .find(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        let idx = plan
            .income
            .events
            .iter()
            .position(|i| i.id == params.event_id)
            .ok_or_else(|| {
                McpError::internal_error(
                    format!("Income event not found: {}", params.event_id),
                    None,
                )
            })?;

        plan.income.events.remove(idx);

        let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
        })?;

        self.sync_manager
            .update_plans(plans_value)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Income event '{}' deleted successfully",
            params.event_id
        ))]))
    }

    // ---- Progress tools ----

    #[tool(description = "Add a net worth data point to progress history. Useful for periodic snapshots from external account data (e.g., Monarch Money).")]
    async fn progress_add_data_point(
        &self,
        Parameters(params): Parameters<ProgressAddDataPointParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let data_point = crate::models::ProgressDataPoint {
            date: params.date,
            net_worth: params.net_worth,
            savings: params.savings,
            taxable: params.taxable,
            tax_deferred: params.tax_deferred,
            tax_free: params.tax_free,
            assets: params.assets,
            debt: params.debt,
            loans: params.loans,
            crypto: params.crypto,
        };

        data.progress.data.push(data_point);
        data.progress.last_updated = chrono::Utc::now().timestamp_millis();

        let progress_value = serde_json::to_value(&data.progress).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize progress: {}", e), None)
        })?;

        self.sync_manager
            .update_progress(progress_value)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            "Progress data point added successfully".to_string(),
        )]))
    }

    #[tool(description = "Get progress history (net worth over time), optionally filtered by date range")]
    async fn progress_get_history(
        &self,
        Parameters(params): Parameters<ProgressGetHistoryParams>,
    ) -> Result<CallToolResult, McpError> {
        let data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let filtered: Vec<&crate::models::ProgressDataPoint> = data
            .progress
            .data
            .iter()
            .filter(|dp| {
                if let Some(start) = params.start_date {
                    if dp.date < start {
                        return false;
                    }
                }
                if let Some(end) = params.end_date {
                    if dp.date > end {
                        return false;
                    }
                }
                true
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "total_data_points": filtered.len(),
                "last_updated": data.progress.last_updated,
                "data": filtered,
            }))
            .unwrap_or_default(),
        )]))
    }

    // ---- Priority tools ----

    #[tool(description = "List all priority/goal events in a plan (401k contributions, savings goals, etc.)")]
    async fn priorities_list(
        &self,
        Parameters(params): Parameters<PlanEventsListParams>,
    ) -> Result<CallToolResult, McpError> {
        let data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plan = data
            .plans
            .iter()
            .find(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        let priorities: Vec<JsonValue> = plan
            .priorities
            .events
            .iter()
            .map(|p| {
                json!({
                    "id": p.id,
                    "name": p.name,
                    "type": p.event_type,
                    "goal_intent": p.goal_intent,
                    "owner": p.owner,
                    "account_id": p.account_id,
                    "start": p.start,
                    "end": p.end,
                    "amount": p.amount,
                    "frequency": p.frequency,
                    "contribution": p.contribution,
                    "employer_match": p.employer_match,
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "plan_id": plan.id,
                "plan_name": plan.name,
                "total_priorities": priorities.len(),
                "priorities": priorities,
            }))
            .unwrap_or_default(),
        )]))
    }

    #[tool(description = "Create a new priority/goal event in a plan (e.g., 401k contribution, savings goal)")]
    async fn priorities_create(
        &self,
        Parameters(params): Parameters<PlanEventCreateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plan = data
            .plans
            .iter_mut()
            .find(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        let mut event_data = params.data.clone();
        let event_id = event_data
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("pri_{}", chrono::Utc::now().timestamp_millis()));
        event_data.insert("id".to_string(), json!(event_id));

        let priority: crate::models::PriorityEvent =
            serde_json::from_value(json!(event_data)).map_err(|e| {
                McpError::internal_error(format!("Invalid priority data: {}", e), None)
            })?;

        plan.priorities.events.push(priority);

        let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
        })?;

        self.sync_manager
            .update_plans(plans_value)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Priority created successfully with ID: {}",
            event_id
        ))]))
    }

    #[tool(description = "Update an existing priority/goal event in a plan (partial updates supported)")]
    async fn priorities_update(
        &self,
        Parameters(params): Parameters<PlanEventUpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let plan = data
            .plans
            .iter_mut()
            .find(|p| p.id == params.plan_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
            })?;

        let priority = plan
            .priorities
            .events
            .iter_mut()
            .find(|p| p.id == params.event_id)
            .ok_or_else(|| {
                McpError::internal_error(
                    format!("Priority not found: {}", params.event_id),
                    None,
                )
            })?;

        let mut priority_json = serde_json::to_value(&*priority).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize priority: {}", e), None)
        })?;

        if let Some(obj) = priority_json.as_object_mut() {
            for (key, value) in &params.data {
                obj.insert(key.clone(), value.clone());
            }
        }

        *priority = serde_json::from_value(priority_json).map_err(|e| {
            McpError::internal_error(
                format!("Failed to deserialize updated priority: {}", e),
                None,
            )
        })?;

        let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
        })?;

        self.sync_manager
            .update_plans(plans_value)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Priority '{}' updated successfully",
            params.event_id
        ))]))
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

        if let Some(path) = &params.navigate_to {
            browser.navigate_to(path).await.map_err(|e| {
                McpError::internal_error(format!("Navigation failed: {}", e), None)
            })?;
        }

        let result = if params.r#async {
            browser.execute_js_async(&params.script).await
        } else {
            browser.execute_js(&params.script).await
        }
        .map_err(|e| McpError::internal_error(format!("JS execution failed: {}", e), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| format!("{:?}", result)),
        )]))
    }

    #[tool(description = "Get simulation/projection results for a plan. Navigates to the plan page, waits for simulation, and returns: year-by-year net worth/income/expenses/contributions/drawdown, milestone completion dates, notable events, and outcome status. Data comes from plan._runtime.results in the Pinia store.")]
    async fn plans_get_simulation_results(
        &self,
        Parameters(params): Parameters<PlanGetParams>,
    ) -> Result<CallToolResult, McpError> {
        let script = r#"
            const callback = arguments[arguments.length - 1];
            try {
                const app = document.querySelector('#app');
                if (!app || !app.__vue_app__) {
                    callback({"__error": "Vue app not found. Page may not be fully loaded."});
                    return;
                }
                const pinia = app.__vue_app__.config.globalProperties.$pinia;
                const store = pinia._s.get('plan');
                const results = store.plan._runtime?.results;
                if (!results || !results.data) {
                    callback({"__error": "Simulation results not found. The plan may not have finished computing."});
                    return;
                }

                // Extract concise yearly summaries
                const years = results.data.filter(y => y.isSimulatedYear).map(y => {
                    const s = y.summary || {};
                    return {
                        age: y.age,
                        calendarYear: y.calendarYear,
                        yearIndex: y.yearIndex,
                        netWorth: Math.round(s.netWorth?.total || 0),
                        netWorthNominal: Math.round(s.netWorth?.actualTotal || 0),
                        delta: Math.round(s.delta?.total || 0),
                        income: Math.round(s.income?.total || 0),
                        taxableIncome: Math.round(s.taxableIncome?.total || 0),
                        expenses: Math.round(s.expenses?.total || 0),
                        taxes: Math.round(s.taxes?.total || 0),
                        contributions: Math.round(s.contributions?.total || 0),
                        drawdown: Math.round(s.drawdown?.total || 0),
                        debtPayments: Math.round(s.debtPayments?.total || 0),
                        netLegacy: Math.round(s.netLegacy?.total || 0),
                        withdrawalRate: s.wr?.total || null,
                        endingAccounts: Math.round(y.endingValues?.totalAccounts || 0),
                        endingDebt: Math.round(y.endingValues?.totalDebt || 0),
                    };
                });

                // Meta info
                const meta = results._meta || {};

                // Notable events (milestones, purchases, job changes, etc.)
                const events = (results.notableEvents || []).map(e => ({
                    name: e.name,
                    icon: e.icon,
                    yearIndex: e.yearIndex,
                    color: e.color,
                    critical: e.critical,
                }));

                callback({
                    outcome: results.outcome,
                    meta: {
                        startAge: meta.startAge,
                        startYear: meta.startYear,
                        lifeExpectancy: meta.lifeExpectancy,
                        endOfPlanAge: meta.endOfPlanAge,
                        retirementYearIndex: meta.retirementYearXVal,
                        netWorthAtRetirement: Math.round(meta.netWorthAtRetirement || 0),
                        finalNetWorth: Math.round(meta.finalNetWorth || 0),
                        finalNetLegacy: Math.round(meta.finalNetLegacy || 0),
                    },
                    milestoneCompletions: meta.milestoneCompletionCache || {},
                    yearCount: years.length,
                    years: years,
                    notableEvents: events,
                });
            } catch(e) {
                callback({"__error": e.message || String(e)});
            }
        "#;

        self.run_plan_js(&params.plan_id, script).await
    }

    #[tool(description = "Get Monte Carlo simulation results for a plan. Returns trial outcome distribution, percentile plots, and milestone probability table from the monte-carlo Pinia store.")]
    async fn plans_get_montecarlo_results(
        &self,
        Parameters(params): Parameters<PlanGetParams>,
    ) -> Result<CallToolResult, McpError> {
        let script = r#"
            const callback = arguments[arguments.length - 1];
            try {
                const app = document.querySelector('#app');
                if (!app || !app.__vue_app__) {
                    callback({"__error": "Vue app not found."});
                    return;
                }
                const pinia = app.__vue_app__.config.globalProperties.$pinia;
                const mcStore = pinia._s.get('monte-carlo');
                const mc = mcStore?.montecarlo;
                if (!mc) {
                    callback({"__error": "Monte Carlo store not found."});
                    return;
                }

                const workerStore = pinia._s.get('worker');
                const mcWorker = workerStore?.workers?.montecarlo;

                // Safe serialize helper
                const seen = new WeakSet();
                function safe(obj) {
                    return JSON.parse(JSON.stringify(obj, (key, val) => {
                        if (val && typeof val === 'object') {
                            if (seen.has(val)) return '[circular]';
                            seen.add(val);
                        }
                        return val;
                    }));
                }

                callback({
                    resultCount: mc.resultCount,
                    dirty: mc.dirty,
                    rerunRequired: mc.rerunRequired,
                    workerRunning: mcWorker?.running || false,
                    workerTrials: mcWorker?.trials || 0,
                    workerStats: mcWorker?.stats || null,
                    percentilePlots: safe(mc.percentilePlots),
                    milestoneTable: safe(mc.milestoneTable),
                    config: safe(mc.config),
                });
            } catch(e) {
                callback({"__error": e.message || String(e)});
            }
        "#;

        self.run_plan_js(&params.plan_id, script).await
    }

    #[tool(description = "Get a detailed financial snapshot at a specific age from simulation results. Returns net worth, income, expenses, taxes, contributions, drawdown, debt payments, ending balances, and all summary categories for that year.")]
    async fn plans_get_year_snapshot(
        &self,
        Parameters(params): Parameters<YearSnapshotParams>,
    ) -> Result<CallToolResult, McpError> {
        let script = format!(
            r#"
            const callback = arguments[arguments.length - 1];
            const targetAge = {age};
            try {{
                const app = document.querySelector('#app');
                if (!app || !app.__vue_app__) {{
                    callback({{"__error": "Vue app not found"}});
                    return;
                }}
                const pinia = app.__vue_app__.config.globalProperties.$pinia;
                const store = pinia._s.get('plan');
                const data = store.plan._runtime?.results?.data;
                if (!data) {{
                    callback({{"__error": "No simulation data available"}});
                    return;
                }}

                const year = data.find(y => y.age === targetAge && y.isSimulatedYear);
                if (!year) {{
                    const ages = data.filter(y => y.isSimulatedYear).map(y => y.age);
                    callback({{"__error": "No data for age " + targetAge + ". Available ages: " + ages[0] + "-" + ages[ages.length-1]}});
                    return;
                }}

                // Extract all summary categories with their totals
                const summaryCategories = {{}};
                if (year.summary) {{
                    for (const [k, v] of Object.entries(year.summary)) {{
                        if (v && typeof v === 'object' && v.total !== undefined) {{
                            summaryCategories[k] = {{
                                total: Math.round(v.total * 100) / 100,
                                actualTotal: v.actualTotal !== undefined ? Math.round(v.actualTotal * 100) / 100 : undefined,
                                name: v.name || k,
                                visible: v.visible,
                            }};
                        }}
                    }}
                }}

                callback({{
                    age: year.age,
                    calendarYear: year.calendarYear,
                    yearIndex: year.yearIndex,
                    inflation: year.inflation,
                    cumulativeInflation: year.cumulativeInflation,
                    location: year.location,
                    endingAccounts: Math.round(year.endingValues?.totalAccounts || 0),
                    endingDebt: Math.round(year.endingValues?.totalDebt || 0),
                    summary: summaryCategories,
                }});
            }} catch(e) {{
                callback({{"__error": e.message || String(e)}});
            }}
            "#,
            age = params.age,
        );

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

        let script = format!(
            r#"
            const callback = arguments[arguments.length - 1];
            const startAge = {start_age};
            const endAge = {end_age};
            try {{
                const app = document.querySelector('#app');
                if (!app || !app.__vue_app__) {{
                    callback({{"__error": "Vue app not found"}});
                    return;
                }}
                const pinia = app.__vue_app__.config.globalProperties.$pinia;
                const store = pinia._s.get('plan');
                const data = store.plan._runtime?.results?.data;
                if (!data) {{
                    callback({{"__error": "No simulation data available"}});
                    return;
                }}

                const filtered = data.filter(y => y.isSimulatedYear && y.age >= startAge && y.age <= endAge);
                if (filtered.length === 0) {{
                    callback({{"__error": "No simulated years in range " + startAge + "-" + endAge}});
                    return;
                }}

                // Extract concise data with deltas
                const years = filtered.map((y, i) => {{
                    const s = y.summary || {{}};
                    const nw = Math.round(s.netWorth?.total || 0);
                    const inc = Math.round(s.income?.total || 0);
                    const exp = Math.round(s.expenses?.total || 0);
                    const tax = Math.round(s.taxes?.total || 0);
                    const contrib = Math.round(s.contributions?.total || 0);
                    const draw = Math.round(s.drawdown?.total || 0);

                    const entry = {{
                        age: y.age,
                        calendarYear: y.calendarYear,
                        netWorth: nw,
                        income: inc,
                        expenses: exp,
                        taxes: tax,
                        contributions: contrib,
                        drawdown: draw,
                        endingAccounts: Math.round(y.endingValues?.totalAccounts || 0),
                        endingDebt: Math.round(y.endingValues?.totalDebt || 0),
                    }};

                    // Compute deltas from previous year
                    if (i > 0) {{
                        const prev = filtered[i - 1].summary || {{}};
                        entry.deltaNW = nw - Math.round(prev.netWorth?.total || 0);
                    }}

                    return entry;
                }});

                // Summary stats
                const first = years[0];
                const last = years[years.length - 1];
                callback({{
                    range: {{ startAge, endAge, yearCount: years.length }},
                    summary: {{
                        netWorthChange: last.netWorth - first.netWorth,
                        avgAnnualNWGrowth: Math.round((last.netWorth - first.netWorth) / (years.length - 1 || 1)),
                        startNetWorth: first.netWorth,
                        endNetWorth: last.netWorth,
                    }},
                    years: years,
                }});
            }} catch(e) {{
                callback({{"__error": e.message || String(e)}});
            }}
            "#,
            start_age = params.start_age,
            end_age = params.end_age,
        );

        self.run_plan_js(&params.plan_id, &script).await
    }

    // ---- Integration tools ----

    #[tool(description = "Batch-update ProjectionLab account balances from external data (e.g., Monarch Money). Accepts a list of {pl_account_id, balance} mappings. Updates all matching accounts in a single operation.")]
    async fn sync_account_balances(
        &self,
        Parameters(params): Parameters<SyncAccountBalancesParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut data = self
            .sync_manager
            .get_data()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let mut updated = Vec::new();
        let mut not_found = Vec::new();

        for mapping in &params.mappings {
            let mut found = false;

            // Search savings accounts
            if let Some(account) = data
                .today
                .savings_accounts
                .iter_mut()
                .find(|a| a.id == mapping.pl_account_id)
            {
                account.balance = mapping.balance;
                found = true;
            }

            // Search investment accounts
            if !found {
                if let Some(account) = data
                    .today
                    .investment_accounts
                    .iter_mut()
                    .find(|a| a.id == mapping.pl_account_id)
                {
                    account.balance = mapping.balance;
                    found = true;
                }
            }

            // Search debt accounts
            if !found {
                if let Some(debt) = data
                    .today
                    .debts
                    .iter_mut()
                    .find(|d| d.id == mapping.pl_account_id)
                {
                    debt.balance = mapping.balance;
                    found = true;
                }
            }

            if found {
                updated.push(&mapping.pl_account_id);
            } else {
                not_found.push(&mapping.pl_account_id);
            }
        }

        if !updated.is_empty() {
            let new_finances = serde_json::to_value(&data.today).map_err(|e| {
                McpError::internal_error(format!("Failed to serialize finances: {}", e), None)
            })?;

            self.sync_manager
                .update_current_finances(new_finances)
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        }

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "updated": updated.len(),
                "not_found": not_found.len(),
                "updated_ids": updated,
                "not_found_ids": not_found,
            }))
            .unwrap_or_default(),
        )]))
    }

    #[tool(description = "Record a net worth snapshot to progress history. Convenience wrapper for progress_add_data_point that uses today's date. Pass category breakdowns computed from external account data (e.g., Monarch Money).")]
    async fn snapshot_net_worth(
        &self,
        Parameters(params): Parameters<ProgressAddDataPointParams>,
    ) -> Result<CallToolResult, McpError> {
        // Delegate to progress_add_data_point with same params
        self.progress_add_data_point(Parameters(params)).await
    }

    // ---- System tools ----

    #[tool(description = "Force refresh the cached data from ProjectionLab")]
    async fn refresh_cache(&self) -> Result<CallToolResult, McpError> {
        self.sync_manager
            .refresh()
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

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
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

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
                Plans: plans_list, plans_get, plans_create, plans_delete, plans_get_variables, plans_update_variables, plans_update_metadata, plans_clone, plans_get_milestones\n\
                Milestones: milestones_create, milestones_update, milestones_delete\n\
                Expenses: expenses_list, expenses_create, expenses_update, expenses_delete\n\
                Income: income_list, income_create, income_update, income_delete\n\
                Priorities: priorities_list, priorities_create, priorities_update\n\
                Progress: progress_add_data_point, progress_get_history\n\
                Simulation: plans_get_simulation_results, plans_get_montecarlo_results, plans_get_year_snapshot, plans_get_year_range\n\
                Browser: run_js_in_browser\n\
                Integration: sync_account_balances, snapshot_net_worth\n\
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

        // Spawn browser initialization in the background so the MCP
        // handshake completes immediately. Tools will wait for the
        // browser lock if they're called before init finishes.
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
