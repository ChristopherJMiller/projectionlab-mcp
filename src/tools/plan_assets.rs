//! Handler logic for plan-level asset event tools (future purchases in a plan).

use crate::models::assets::AssetEvent;
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

    let assets: Vec<JsonValue> = plan
        .assets
        .events
        .iter()
        .map(|a| {
            json!({
                "id": a.id,
                "name": a.name,
                "type": a.event_type,
                "amount": a.amount,
                "amount_type": a.amount_type,
                "initial_value": a.initial_value,
                "owner": a.owner,
                "start": a.start,
                "end": a.end,
                "down_payment": a.down_payment,
                "interest_rate": a.interest_rate,
                "monthly_payment": a.monthly_payment,
                "balance": a.balance,
            })
        })
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&json!({
            "plan_id": plan.id,
            "plan_name": plan.name,
            "total_assets": assets.len(),
            "assets": assets,
        }))
        .unwrap_or_default(),
    )]))
}

pub async fn create(
    sync: &SyncManager,
    params: PlanEventCreateParams,
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

    let mut event_data = params.data.clone();
    let event_id = event_data
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("asset_{}", chrono::Utc::now().timestamp_millis()));
    event_data.insert("id".to_string(), json!(event_id));

    let asset: AssetEvent =
        serde_json::from_value(json!(event_data)).map_err(|e| {
            McpError::internal_error(format!("Invalid asset event data: {}", e), None)
        })?;

    plan.assets.events.push(asset);

    let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
    })?;

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Plan asset event created successfully with ID: {}",
        event_id
    ))]))
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

    let asset = plan
        .assets
        .events
        .iter_mut()
        .find(|a| a.id == params.event_id)
        .ok_or_else(|| {
            McpError::internal_error(
                format!("Plan asset event not found: {}", params.event_id),
                None,
            )
        })?;

    merge_json_fields(asset, &params.data)?;

    let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
    })?;

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Plan asset event '{}' updated successfully",
        params.event_id
    ))]))
}

pub async fn delete(
    sync: &SyncManager,
    params: PlanEventDeleteParams,
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

    let idx = plan
        .assets
        .events
        .iter()
        .position(|a| a.id == params.event_id)
        .ok_or_else(|| {
            McpError::internal_error(
                format!("Plan asset event not found: {}", params.event_id),
                None,
            )
        })?;

    plan.assets.events.remove(idx);

    let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
    })?;

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Plan asset event '{}' deleted successfully",
        params.event_id
    ))]))
}
