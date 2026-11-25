use crate::sync::SyncManager;
use crate::tools::accounts::account_summary;
use crate::tools::history::load_net_worth_series;
use anyhow::{Context, Result};
use chrono::Utc;
use rmcp::model::{AnnotateAble, RawResource, ReadResourceResult, Resource, ResourceContents};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;
use tracing::debug;

/// Provides MCP resources for ProjectionLab data
pub struct ResourceProvider {
    sync: Arc<SyncManager>,
}

fn text_result(value: &Value, uri: &str) -> Result<ReadResourceResult> {
    Ok(ReadResourceResult {
        contents: vec![ResourceContents::text(
            serde_json::to_string_pretty(value)?,
            uri,
        )],
    })
}

impl ResourceProvider {
    pub fn new(sync: Arc<SyncManager>) -> Self {
        Self { sync }
    }

    /// List available resources
    pub async fn list_resources(&self) -> Vec<Resource> {
        [
            ("projectionlab://overview", "Net worth and counts of accounts, debts, real assets, and plans"),
            ("projectionlab://accounts/summary", "All active Current Finances accounts, debts, and real assets"),
            ("projectionlab://expenses/summary", "Expense events across all plans"),
            ("projectionlab://plans/summary", "All plans with key metadata"),
            ("projectionlab://income/summary", "Income events across all plans"),
            ("projectionlab://net-worth/history", "Net worth history recorded by ProjectionLab"),
        ]
        .into_iter()
        .map(|(uri, desc)| RawResource::new(uri, desc.to_string()).no_annotation())
        .collect()
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
            _ if uri.starts_with("projectionlab://accounts/") => self.get_account_details(uri).await,
            _ if uri.starts_with("projectionlab://plan/") => self.get_plan_resource(uri).await,
            _ => anyhow::bail!("Resource not found: {}", uri),
        }
    }

    async fn get_overview(&self) -> Result<ReadResourceResult> {
        let data = self.sync.get_data().await?;
        let cf = data.current_finances();
        let active = cf.active_accounts();

        let mut totals_by_bucket: BTreeMap<&str, f64> = BTreeMap::new();
        for a in &active {
            *totals_by_bucket.entry(a.bucket()).or_default() += a.current_value();
        }
        let count = |kind: &str| active.iter().filter(|a| a.kind == kind).count();

        text_result(
            &json!({
                "resource_type": "overview",
                "generated_at": Utc::now().to_rfc3339(),
                "net_worth": cf.net_worth(),
                "totals_by_bucket": totals_by_bucket,
                "accounts": count("account"),
                "debts": count("debt"),
                "real_assets": count("real-asset"),
                "archived": cf.accounts.len() - active.len(),
                "plans": data.plans().len(),
                "active_plan_id": data.active_plan_id(),
                "baseline_plan_id": data.baseline_plan_id(),
                "current_finances_last_updated": cf.last_updated.map(format_timestamp_ms),
                "exported_at": format_timestamp_ms(data.exported_at),
            }),
            "projectionlab://overview",
        )
    }

    async fn get_accounts_summary(&self) -> Result<ReadResourceResult> {
        let data = self.sync.get_data().await?;
        let cf = data.current_finances();
        let accounts: Vec<Value> = cf
            .active_accounts()
            .into_iter()
            .map(|a| {
                let mut s = account_summary(a);
                s["uri"] = json!(format!("projectionlab://accounts/{}", a.id));
                s
            })
            .collect();

        text_result(
            &json!({
                "resource_type": "accounts_summary",
                "generated_at": Utc::now().to_rfc3339(),
                "total_accounts": accounts.len(),
                "net_worth": cf.net_worth(),
                "accounts": accounts,
            }),
            "projectionlab://accounts/summary",
        )
    }

    /// `projectionlab://accounts/{id}` (the legacy `accounts/{type}/{id}` form also works).
    async fn get_account_details(&self, uri: &str) -> Result<ReadResourceResult> {
        let data = self.sync.get_data().await?;
        let account_id = uri
            .rsplit('/')
            .next()
            .filter(|s| !s.is_empty())
            .context(format!("Invalid account URI: {}", uri))?;
        let account = data
            .current_finances()
            .find(account_id)
            .context(format!("Account not found: {}", account_id))?;
        text_result(
            &json!({
                "resource_type": "account_details",
                "generated_at": Utc::now().to_rfc3339(),
                "account": account,
            }),
            uri,
        )
    }

    async fn get_expenses_summary(&self) -> Result<ReadResourceResult> {
        let data = self.sync.get_data().await?;
        let mut expenses = Vec::new();
        let mut by_type: BTreeMap<String, usize> = BTreeMap::new();
        for plan in data.plans() {
            for e in &plan.expenses.events {
                *by_type.entry(e.event_type.clone()).or_default() += 1;
                let mut s = e.summary(&["amount", "amountType", "frequency", "spendingType"]);
                s["plan_id"] = json!(plan.id);
                s["plan_name"] = json!(plan.name);
                expenses.push(s);
            }
        }
        text_result(
            &json!({
                "resource_type": "expenses_summary",
                "generated_at": Utc::now().to_rfc3339(),
                "total_expenses": expenses.len(),
                "expenses_by_type": by_type,
                "expenses": expenses,
            }),
            "projectionlab://expenses/summary",
        )
    }

    async fn get_plans_summary(&self) -> Result<ReadResourceResult> {
        let data = self.sync.get_data().await?;
        let plans: Vec<Value> = data
            .plans()
            .iter()
            .map(|plan| {
                json!({
                    "id": plan.id,
                    "name": plan.name,
                    "icon": plan.icon,
                    "is_active": data.active_plan_id() == Some(plan.id.as_str()),
                    "is_baseline": data.baseline_plan_id() == Some(plan.id.as_str()),
                    "starting_conditions": plan.starting_conditions_type,
                    "last_updated": plan.last_updated.map(format_timestamp_ms),
                    "milestones": plan.milestones.iter().map(|m| json!({"id": m.id, "name": m.name})).collect::<Vec<_>>(),
                    "expenses": plan.expenses.events.len(),
                    "income": plan.income.events.len(),
                    "priorities": plan.priorities.events.len(),
                    "assets": plan.assets.events.len(),
                    "accounts": plan.accounts.events.len(),
                    "withdrawal_strategy": plan.withdrawal_strategy_name(),
                    "montecarlo_trials": plan.montecarlo.get("trials"),
                })
            })
            .collect();
        text_result(
            &json!({
                "resource_type": "plans_summary",
                "generated_at": Utc::now().to_rfc3339(),
                "total_plans": plans.len(),
                "plans": plans,
            }),
            "projectionlab://plans/summary",
        )
    }

    async fn get_income_summary(&self) -> Result<ReadResourceResult> {
        let data = self.sync.get_data().await?;
        let mut income = Vec::new();
        for plan in data.plans() {
            for e in &plan.income.events {
                let mut s = e.summary(&[
                    "amount", "amountType", "frequency", "owner", "start", "end", "taxExempt",
                    "taxCharacter", "withholdingMode", "withholdingRate",
                ]);
                s["plan_id"] = json!(plan.id);
                s["plan_name"] = json!(plan.name);
                income.push(s);
            }
        }
        text_result(
            &json!({
                "resource_type": "income_summary",
                "generated_at": Utc::now().to_rfc3339(),
                "total_income_events": income.len(),
                "income": income,
            }),
            "projectionlab://income/summary",
        )
    }

    async fn get_net_worth_history(&self) -> Result<ReadResourceResult> {
        let (points, settled) = load_net_worth_series(&self.sync).await?;
        text_result(
            &json!({
                "resource_type": "net_worth_history",
                "generated_at": Utc::now().to_rfc3339(),
                "settled": settled,
                "total_data_points": points.len(),
                "data": points,
            }),
            "projectionlab://net-worth/history",
        )
    }

    /// `projectionlab://plan/{id}/{variables|milestones}`
    async fn get_plan_resource(&self, uri: &str) -> Result<ReadResourceResult> {
        let data = self.sync.get_data().await?;
        let parts: Vec<&str> = uri.split('/').collect();
        if parts.len() < 5 {
            anyhow::bail!("Invalid plan resource URI: {}", uri);
        }
        let (plan_id, subresource) = (parts[3], parts[4]);
        let plan = data
            .find_plan(plan_id)
            .context(format!("Plan not found: {}", plan_id))?;

        let value = match subresource {
            "variables" => json!({
                "resource_type": "plan_variables",
                "generated_at": Utc::now().to_rfc3339(),
                "plan_id": plan.id,
                "plan_name": plan.name,
                "variables": plan.variables,
            }),
            "milestones" => json!({
                "resource_type": "plan_milestones",
                "generated_at": Utc::now().to_rfc3339(),
                "plan_id": plan.id,
                "plan_name": plan.name,
                "milestones": plan.milestones,
            }),
            _ => anyhow::bail!("Unknown plan subresource: {}", subresource),
        };
        text_result(&value, uri)
    }
}

fn format_timestamp_ms(timestamp_ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(timestamp_ms)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "unknown".to_string())
}
