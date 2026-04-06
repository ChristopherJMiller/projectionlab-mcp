use crate::models::{
    AccountDetailsResource, AccountSummary, AccountsSummaryResource, ExpenseSummary,
    ExpensesSummaryResource, OverviewResource,
};
use crate::sync::SyncManager;
use anyhow::{Context, Result};
use chrono::Utc;
use rmcp::model::{AnnotateAble, RawResource, ReadResourceResult, Resource, ResourceContents};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;

/// Provides MCP resources for ProjectionLab data
pub struct ResourceProvider {
    sync: Arc<SyncManager>,
}

impl ResourceProvider {
    pub fn new(sync: Arc<SyncManager>) -> Self {
        Self { sync }
    }

    /// List available resources
    pub async fn list_resources(&self) -> Vec<Resource> {
        vec![
            self.create_resource("projectionlab://overview", "Overview summary of all financial data"),
            self.create_resource("projectionlab://accounts/summary", "Summary of all accounts"),
            self.create_resource("projectionlab://expenses/summary", "Summary of all expenses"),
            self.create_resource("projectionlab://plans/summary", "Summary of all plans with key metrics"),
            self.create_resource("projectionlab://income/summary", "Summary of all income events across active plans"),
            self.create_resource("projectionlab://net-worth/history", "Historical net worth data points from progress tracking"),
        ]
    }

    /// Read a specific resource by URI
    pub async fn read_resource(&self, uri: &str) -> Result<ReadResourceResult> {
        debug!("Reading resource: {}", uri);

        match uri {
            "projectionlab://overview" => self.get_overview().await,
            "projectionlab://accounts/summary" => self.get_accounts_summary().await,
            "projectionlab://expenses/summary" => self.get_expenses_summary().await,
            "projectionlab://plans/summary" => self.get_plans_summary().await,
            "projectionlab://income/summary" => self.get_income_summary().await,
            "projectionlab://net-worth/history" => self.get_net_worth_history().await,
            _ => {
                if uri.starts_with("projectionlab://accounts/") {
                    self.get_account_details(uri).await
                } else if uri.starts_with("projectionlab://plan/") {
                    self.get_plan_resource(uri).await
                } else {
                    anyhow::bail!("Resource not found: {}", uri)
                }
            }
        }
    }

    /// Create a resource descriptor
    fn create_resource(&self, uri: &str, description: &str) -> Resource {
        RawResource::new(uri, description.to_string()).no_annotation()
    }

    /// Get overview resource
    async fn get_overview(&self) -> Result<ReadResourceResult> {
        let data = self.sync.get_data().await?;

        let total_savings: f64 = data.today.savings_accounts.iter().map(|a| a.balance).sum();
        let total_investment: f64 = data
            .today
            .investment_accounts
            .iter()
            .map(|a| a.balance)
            .sum();
        let total_debt: f64 = data.today.debts.iter().map(|d| d.balance).sum();
        let total_net_worth = total_savings + total_investment - total_debt;

        let overview = OverviewResource {
            resource_type: "overview".to_string(),
            generated_at: Utc::now().to_rfc3339(),
            total_accounts: data.today.savings_accounts.len()
                + data.today.investment_accounts.len()
                + data.today.debts.len(),
            total_net_worth,
            total_savings,
            total_investment,
            total_debt,
            active_plans: data.plans.iter().filter(|p| p.active).count(),
            last_updated: format_timestamp(data.meta.last_updated),
        };

        let json = serde_json::to_string_pretty(&overview)?;

        Ok(ReadResourceResult {
            contents: vec![ResourceContents::text(json, "projectionlab://overview")],
        })
    }

    /// Get accounts summary resource
    async fn get_accounts_summary(&self) -> Result<ReadResourceResult> {
        let data = self.sync.get_data().await?;

        let mut accounts = Vec::new();
        let mut accounts_by_type = HashMap::new();

        // Add savings accounts
        for account in &data.today.savings_accounts {
            accounts.push(AccountSummary {
                id: account.id.clone(),
                name: account.name.clone(),
                account_type: "savings".to_string(),
                balance: account.balance,
                owner: format!("{:?}", account.owner).to_lowercase(),
                uri: format!("projectionlab://accounts/savings/{}", account.id),
            });
            *accounts_by_type.entry("savings".to_string()).or_insert(0) += 1;
        }

        // Add investment accounts
        for account in &data.today.investment_accounts {
            let account_type = account.account_type.clone();
            accounts.push(AccountSummary {
                id: account.id.clone(),
                name: account.name.clone(),
                account_type: account_type.clone(),
                balance: account.balance,
                owner: format!("{:?}", account.owner).to_lowercase(),
                uri: format!("projectionlab://accounts/investment/{}", account.id),
            });
            *accounts_by_type.entry(account_type).or_insert(0) += 1;
        }

        // Add debt accounts
        for debt in &data.today.debts {
            accounts.push(AccountSummary {
                id: debt.id.clone(),
                name: debt.name.clone(),
                account_type: debt.debt_type.clone(),
                balance: debt.balance,
                owner: format!("{:?}", debt.owner).to_lowercase(),
                uri: format!("projectionlab://accounts/debt/{}", debt.id),
            });
            *accounts_by_type
                .entry(debt.debt_type.clone())
                .or_insert(0) += 1;
        }

        let total_balance: f64 = accounts.iter().map(|a| a.balance).sum();

        let summary = AccountsSummaryResource {
            resource_type: "accounts_summary".to_string(),
            generated_at: Utc::now().to_rfc3339(),
            total_accounts: accounts.len(),
            total_balance,
            accounts_by_type,
            accounts,
        };

        let json = serde_json::to_string_pretty(&summary)?;

        Ok(ReadResourceResult {
            contents: vec![ResourceContents::text(
                json,
                "projectionlab://accounts/summary",
            )],
        })
    }

    /// Get account details by URI
    async fn get_account_details(&self, uri: &str) -> Result<ReadResourceResult> {
        let data = self.sync.get_data().await?;

        // Extract account ID from URI
        let parts: Vec<&str> = uri.split('/').collect();
        if parts.len() < 4 {
            anyhow::bail!("Invalid account URI: {}", uri);
        }

        let account_type = parts[2]; // savings, investment, or debt
        let account_id = parts[3];

        let details = match account_type {
            "savings" => {
                let account = data
                    .today
                    .savings_accounts
                    .iter()
                    .find(|a| a.id == account_id)
                    .context(format!("Savings account not found: {}", account_id))?;

                AccountDetailsResource {
                    resource_type: "account_details".to_string(),
                    generated_at: Utc::now().to_rfc3339(),
                    id: account.id.clone(),
                    name: account.name.clone(),
                    account_type: "savings".to_string(),
                    balance: account.balance,
                    owner: format!("{:?}", account.owner).to_lowercase(),
                    liquid: account.liquid,
                    icon: Some(account.icon.clone()),
                    color: Some(account.color.clone()),
                    cost_basis: None,
                }
            }
            "investment" => {
                let account = data
                    .today
                    .investment_accounts
                    .iter()
                    .find(|a| a.id == account_id)
                    .context(format!("Investment account not found: {}", account_id))?;

                AccountDetailsResource {
                    resource_type: "account_details".to_string(),
                    generated_at: Utc::now().to_rfc3339(),
                    id: account.id.clone(),
                    name: account.name.clone(),
                    account_type: account.account_type.clone(),
                    balance: account.balance,
                    owner: format!("{:?}", account.owner).to_lowercase(),
                    liquid: account.liquid,
                    icon: Some(account.icon.clone()),
                    color: Some(account.color.clone()),
                    cost_basis: account.cost_basis,
                }
            }
            "debt" => {
                let debt = data
                    .today
                    .debts
                    .iter()
                    .find(|d| d.id == account_id)
                    .context(format!("Debt account not found: {}", account_id))?;

                AccountDetailsResource {
                    resource_type: "account_details".to_string(),
                    generated_at: Utc::now().to_rfc3339(),
                    id: debt.id.clone(),
                    name: debt.name.clone(),
                    account_type: debt.debt_type.clone(),
                    balance: debt.balance,
                    owner: format!("{:?}", debt.owner).to_lowercase(),
                    liquid: debt.liquid.unwrap_or(false),
                    icon: Some(debt.icon.clone()),
                    color: Some(debt.color.clone()),
                    cost_basis: None,
                }
            }
            _ => anyhow::bail!("Invalid account type: {}", account_type),
        };

        let json = serde_json::to_string_pretty(&details)?;

        Ok(ReadResourceResult {
            contents: vec![ResourceContents::text(json, uri)],
        })
    }

    /// Get expenses summary resource
    async fn get_expenses_summary(&self) -> Result<ReadResourceResult> {
        let data = self.sync.get_data().await?;

        let mut expenses = Vec::new();
        let mut expenses_by_category: HashMap<String, usize> = HashMap::new();

        // Extract expenses from all plans
        for plan in &data.plans {
            // Access expenses directly (not optional)
            for event in &plan.expenses.events {
                let expense_type = event.event_type.clone();

                expenses.push(ExpenseSummary {
                    id: event.id.clone(),
                    name: event.name.clone(),
                    expense_type: expense_type.clone(),
                    amount: event.amount,
                    frequency: event.frequency.clone(),
                    uri: format!("projectionlab://plans/{}/expenses/{}", plan.id, event.id),
                });
                *expenses_by_category.entry(expense_type).or_insert(0) += 1;
            }
        }

        let summary = ExpensesSummaryResource {
            resource_type: "expenses_summary".to_string(),
            generated_at: chrono::Utc::now().to_rfc3339(),
            total_expenses: expenses.len(),
            expenses_by_category,
            expenses,
        };

        Ok(ReadResourceResult {
            contents: vec![ResourceContents::text(
                serde_json::to_string_pretty(&summary).unwrap_or_default(),
                "projectionlab://expenses/summary".to_string(),
            )],
        })
    }

    /// Get plans summary resource
    async fn get_plans_summary(&self) -> Result<ReadResourceResult> {
        let data = self.sync.get_data().await?;

        let plans: Vec<serde_json::Value> = data
            .plans
            .iter()
            .map(|plan| {
                // Find the retirement milestone if any
                let retirement_milestone = plan
                    .milestones
                    .iter()
                    .find(|m| m.name.to_lowercase().contains("retire"))
                    .map(|m| json!({"name": m.name, "id": m.id}));

                json!({
                    "id": plan.id,
                    "name": plan.name,
                    "icon": plan.icon,
                    "active": plan.active,
                    "last_updated": plan.last_updated,
                    "retirement_milestone": retirement_milestone,
                    "milestones": plan.milestones.len(),
                    "expenses": plan.expenses.events.len(),
                    "income": plan.income.events.len(),
                    "priorities": plan.priorities.events.len(),
                    "assets": plan.assets.events.len(),
                    "accounts": plan.accounts.events.len(),
                    "withdrawal_strategy": plan.withdrawal_strategy.strategy,
                    "montecarlo_trials": plan.montecarlo.trials,
                })
            })
            .collect();

        let json = serde_json::to_string_pretty(&json!({
            "resource_type": "plans_summary",
            "generated_at": Utc::now().to_rfc3339(),
            "total_plans": plans.len(),
            "active_plans": data.plans.iter().filter(|p| p.active).count(),
            "plans": plans,
        }))?;

        Ok(ReadResourceResult {
            contents: vec![ResourceContents::text(json, "projectionlab://plans/summary")],
        })
    }

    /// Get income summary resource across all plans
    async fn get_income_summary(&self) -> Result<ReadResourceResult> {
        let data = self.sync.get_data().await?;

        let mut income_events = Vec::new();

        for plan in &data.plans {
            for event in &plan.income.events {
                income_events.push(json!({
                    "id": event.id,
                    "name": event.name,
                    "type": event.event_type,
                    "amount": event.amount,
                    "amount_type": event.amount_type,
                    "frequency": event.frequency,
                    "owner": event.owner,
                    "start": event.start,
                    "end": event.end,
                    "tax_withholding": event.tax_withholding,
                    "withhold": event.withhold,
                    "plan_id": plan.id,
                    "plan_name": plan.name,
                }));
            }
        }

        let json = serde_json::to_string_pretty(&json!({
            "resource_type": "income_summary",
            "generated_at": Utc::now().to_rfc3339(),
            "total_income_events": income_events.len(),
            "income": income_events,
        }))?;

        Ok(ReadResourceResult {
            contents: vec![ResourceContents::text(json, "projectionlab://income/summary")],
        })
    }

    /// Get net worth history from progress data
    async fn get_net_worth_history(&self) -> Result<ReadResourceResult> {
        let data = self.sync.get_data().await?;

        let json = serde_json::to_string_pretty(&json!({
            "resource_type": "net_worth_history",
            "generated_at": Utc::now().to_rfc3339(),
            "total_data_points": data.progress.data.len(),
            "last_updated": data.progress.last_updated,
            "data": data.progress.data,
        }))?;

        Ok(ReadResourceResult {
            contents: vec![ResourceContents::text(
                json,
                "projectionlab://net-worth/history",
            )],
        })
    }

    /// Handle plan-specific resource URIs like projectionlab://plan/{id}/variables
    async fn get_plan_resource(&self, uri: &str) -> Result<ReadResourceResult> {
        let data = self.sync.get_data().await?;

        // Parse: projectionlab://plan/{id}/{subresource}
        // URI parts after split on '/': ["projectionlab:", "", "plan", "{id}", "{subresource}"]
        let parts: Vec<&str> = uri.split('/').collect();
        if parts.len() < 5 {
            anyhow::bail!("Invalid plan resource URI: {}", uri);
        }

        let plan_id = parts[3];
        let subresource = parts[4];

        let plan = data
            .plans
            .iter()
            .find(|p| p.id == plan_id)
            .context(format!("Plan not found: {}", plan_id))?;

        let json = match subresource {
            "variables" => serde_json::to_string_pretty(&json!({
                "resource_type": "plan_variables",
                "generated_at": Utc::now().to_rfc3339(),
                "plan_id": plan.id,
                "plan_name": plan.name,
                "variables": plan.variables,
            }))?,
            "milestones" => serde_json::to_string_pretty(&json!({
                "resource_type": "plan_milestones",
                "generated_at": Utc::now().to_rfc3339(),
                "plan_id": plan.id,
                "plan_name": plan.name,
                "milestones": plan.milestones,
                "computed_milestones": plan.computed_milestones,
            }))?,
            _ => anyhow::bail!("Unknown plan subresource: {}", subresource),
        };

        Ok(ReadResourceResult {
            contents: vec![ResourceContents::text(json, uri)],
        })
    }
}

fn format_timestamp(timestamp: i64) -> String {
    chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "unknown".to_string())
}

