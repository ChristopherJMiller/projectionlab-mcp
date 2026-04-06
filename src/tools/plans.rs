//! Handler logic for plan management, metadata, variables, and milestone tools.

use crate::models;
use crate::sync::SyncManager;
use rmcp::{model::*, ErrorData as McpError};
use serde_json::{json, Value as JsonValue};

use super::params::*;

pub async fn list(sync: &SyncManager) -> Result<CallToolResult, McpError> {
    let data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

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

pub async fn get(
    sync: &SyncManager,
    params: PlanGetParams,
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

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&plan).unwrap_or_default(),
    )]))
}

pub async fn get_variables(
    sync: &SyncManager,
    params: PlanGetParams,
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

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&json!({
            "plan_id": plan.id,
            "plan_name": plan.name,
            "variables": plan.variables,
        }))
        .unwrap_or_default(),
    )]))
}

pub async fn update_variables(
    sync: &SyncManager,
    params: PlansUpdateVariablesParams,
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

    let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
    })?;

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Plan '{}' variables updated successfully",
        params.plan_id
    ))]))
}

pub async fn clone_plan(
    sync: &SyncManager,
    params: PlansCloneParams,
) -> Result<CallToolResult, McpError> {
    let mut data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

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

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Plan cloned successfully. New plan '{}' with ID: {}",
        params.new_name, new_id
    ))]))
}

pub async fn create(
    sync: &SyncManager,
    params: PlansCreateParams,
) -> Result<CallToolResult, McpError> {
    let mut data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let new_id = format!("plan_{}", chrono::Utc::now().timestamp_millis());
    let now = chrono::Utc::now().timestamp_millis();

    let new_plan = if let Some(source_id) = &params.clone_from {
        let source = data
            .plans
            .iter()
            .find(|p| p.id == *source_id)
            .ok_or_else(|| {
                McpError::internal_error(format!("Source plan not found: {}", source_id), None)
            })?
            .clone();

        let mut cloned = source;
        cloned.id = new_id.clone();
        cloned.name = params.name.clone();
        cloned.icon = params.icon.clone();
        cloned.last_updated = now;
        cloned
    } else {
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

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Plan '{}' created successfully with ID: {}",
        params.name, new_id
    ))]))
}

pub async fn delete(
    sync: &SyncManager,
    params: PlansDeleteParams,
) -> Result<CallToolResult, McpError> {
    if !params.confirm {
        return Err(McpError::internal_error(
            "Deletion not confirmed. Set confirm=true to delete the plan.",
            None,
        ));
    }

    let mut data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

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

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Plan '{}' ({}) deleted successfully",
        removed_name, params.plan_id
    ))]))
}

pub async fn update_metadata(
    sync: &SyncManager,
    params: PlansUpdateMetadataParams,
) -> Result<CallToolResult, McpError> {
    const ALLOWED_KEYS: &[&str] = &["name", "icon", "active"];

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

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Plan '{}' metadata updated successfully",
        params.plan_id
    ))]))
}

// ---- Milestone handlers ----

pub async fn get_milestones(
    sync: &SyncManager,
    params: PlanGetParams,
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

pub async fn create_milestone(
    sync: &SyncManager,
    params: MilestoneCreateParams,
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

    let mut milestone_data = params.data;
    let milestone_id = milestone_data
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("ms_{}", chrono::Utc::now().timestamp_millis()));
    milestone_data.insert("id".to_string(), json!(milestone_id));

    let milestone: models::Milestone =
        serde_json::from_value(json!(milestone_data)).map_err(|e| {
            McpError::internal_error(format!("Invalid milestone data: {}", e), None)
        })?;

    plan.milestones.push(milestone);

    let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
    })?;

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Milestone created successfully with ID: {}",
        milestone_id
    ))]))
}

pub async fn update_milestone(
    sync: &SyncManager,
    params: MilestoneUpdateParams,
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

    super::accounts::merge_json_fields(milestone, &params.data)?;

    let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
    })?;

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Milestone '{}' updated successfully",
        params.milestone_id
    ))]))
}

pub async fn delete_milestone(
    sync: &SyncManager,
    params: MilestoneDeleteParams,
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

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Milestone '{}' deleted successfully",
        params.milestone_id
    ))]))
}
