//! Handler logic for plan-level account event tools.

use crate::sync::SyncManager;
use rmcp::{model::*, ErrorData as McpError};
use serde_json::{json, Value as JsonValue};

use super::accounts::merge_json_fields;
use super::params::*;

pub async fn list(
    sync: &SyncManager,
    params: PlanEventsListParams,
) -> Result<CallToolResult, McpError> {
    let data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let plan = data
        .plans
        .iter()
        .find(|p| p.id == params.plan_id)
        .ok_or_else(|| {
            McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
        })?;

    let accounts: Vec<JsonValue> = plan
        .accounts
        .events
        .iter()
        .map(|a| {
            json!({
                "id": a.id,
                "name": a.name,
                "type": a.event_type,
                "balance": a.balance,
                "owner": a.owner,
                "liquid": a.liquid,
                "withdraw": a.withdraw,
                "investment_growth_rate": a.investment_growth_rate,
                "investment_growth_type": a.investment_growth_type,
                "dividend_rate": a.dividend_rate,
            })
        })
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&json!({
            "plan_id": plan.id,
            "plan_name": plan.name,
            "total_accounts": accounts.len(),
            "accounts": accounts,
        }))
        .unwrap_or_default(),
    )]))
}

pub async fn update(
    sync: &SyncManager,
    params: PlanEventUpdateParams,
) -> Result<CallToolResult, McpError> {
    let mut data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let plan = data
        .plans
        .iter_mut()
        .find(|p| p.id == params.plan_id)
        .ok_or_else(|| {
            McpError::internal_error(format!("Plan not found: {}", params.plan_id), None)
        })?;

    let account = plan
        .accounts
        .events
        .iter_mut()
        .find(|a| a.id == params.event_id)
        .ok_or_else(|| {
            McpError::internal_error(
                format!("Plan account event not found: {}", params.event_id),
                None,
            )
        })?;

    merge_json_fields(account, &params.data)?;

    let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
    })?;

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Plan account event '{}' updated successfully",
        params.event_id
    ))]))
}
