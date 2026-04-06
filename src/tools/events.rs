//! Handler logic for plan event tools (expenses, income, priorities).

use crate::models;
use crate::sync::SyncManager;
use rmcp::{model::*, ErrorData as McpError};
use serde_json::{json, Value as JsonValue};

use super::accounts::merge_json_fields;
use super::params::*;

// ---- Expense handlers ----

pub async fn expenses_list(
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

pub async fn expenses_create(
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
        .unwrap_or_else(|| format!("exp_{}", chrono::Utc::now().timestamp_millis()));
    event_data.insert("id".to_string(), json!(event_id));

    let expense: models::ExpenseEvent =
        serde_json::from_value(json!(event_data)).map_err(|e| {
            McpError::internal_error(format!("Invalid expense data: {}", e), None)
        })?;

    plan.expenses.events.push(expense);

    let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
    })?;

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Expense created successfully with ID: {}",
        event_id
    ))]))
}

pub async fn expenses_update(
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

    let expense = plan
        .expenses
        .events
        .iter_mut()
        .find(|e| e.id == params.event_id)
        .ok_or_else(|| {
            McpError::internal_error(format!("Expense not found: {}", params.event_id), None)
        })?;

    merge_json_fields(expense, &params.data)?;

    let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
    })?;

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Expense '{}' updated successfully",
        params.event_id
    ))]))
}

pub async fn expenses_delete(
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
        .expenses
        .events
        .iter()
        .position(|e| e.id == params.event_id)
        .ok_or_else(|| {
            McpError::internal_error(format!("Expense not found: {}", params.event_id), None)
        })?;

    plan.expenses.events.remove(idx);

    let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
    })?;

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Expense '{}' deleted successfully",
        params.event_id
    ))]))
}

// ---- Income handlers ----

pub async fn income_list(
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

pub async fn income_create(
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
        .unwrap_or_else(|| format!("inc_{}", chrono::Utc::now().timestamp_millis()));
    event_data.insert("id".to_string(), json!(event_id));

    let income: models::IncomeEvent =
        serde_json::from_value(json!(event_data)).map_err(|e| {
            McpError::internal_error(format!("Invalid income data: {}", e), None)
        })?;

    plan.income.events.push(income);

    let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
    })?;

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Income event created successfully with ID: {}",
        event_id
    ))]))
}

pub async fn income_update(
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

    merge_json_fields(income, &params.data)?;

    let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
    })?;

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Income event '{}' updated successfully",
        params.event_id
    ))]))
}

pub async fn income_delete(
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

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Income event '{}' deleted successfully",
        params.event_id
    ))]))
}

// ---- Priority handlers ----

pub async fn priorities_list(
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

pub async fn priorities_create(
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
        .unwrap_or_else(|| format!("pri_{}", chrono::Utc::now().timestamp_millis()));
    event_data.insert("id".to_string(), json!(event_id));

    let priority: models::PriorityEvent =
        serde_json::from_value(json!(event_data)).map_err(|e| {
            McpError::internal_error(format!("Invalid priority data: {}", e), None)
        })?;

    plan.priorities.events.push(priority);

    let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
    })?;

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Priority created successfully with ID: {}",
        event_id
    ))]))
}

pub async fn priorities_update(
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

    merge_json_fields(priority, &params.data)?;

    let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
    })?;

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Priority '{}' updated successfully",
        params.event_id
    ))]))
}

pub async fn priorities_delete(
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
        .priorities
        .events
        .iter()
        .position(|p| p.id == params.event_id)
        .ok_or_else(|| {
            McpError::internal_error(
                format!("Priority not found: {}", params.event_id),
                None,
            )
        })?;

    plan.priorities.events.remove(idx);

    let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
    })?;

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Priority '{}' deleted successfully",
        params.event_id
    ))]))
}
