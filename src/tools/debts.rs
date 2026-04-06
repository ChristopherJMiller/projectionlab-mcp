//! Handler logic for debt tools (Current Finances).

use crate::models::DebtAccount;
use crate::sync::SyncManager;
use rmcp::{model::*, ErrorData as McpError};
use serde_json::json;

use super::accounts::merge_json_fields;
use super::params::*;

pub async fn list(
    sync: &SyncManager,
    params: DebtsListParams,
) -> Result<CallToolResult, McpError> {
    let data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let debts: Vec<_> = data
        .today
        .debts
        .iter()
        .filter(|d| {
            if let Some(ref filter_type) = params.debt_type {
                if &d.debt_type != filter_type {
                    return false;
                }
            }
            if let Some(ref filter_owner) = params.owner {
                let owner_str = format!("{:?}", d.owner).to_lowercase();
                if &owner_str != filter_owner {
                    return false;
                }
            }
            true
        })
        .map(|d| {
            json!({
                "id": d.id,
                "name": d.name,
                "type": d.debt_type,
                "balance": d.balance,
                "original_balance": d.original_balance,
                "owner": d.owner,
                "interest_rate": d.interest_rate,
                "monthly_payment": d.monthly_payment,
                "term_months": d.term_months,
            })
        })
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&json!({
            "total_count": debts.len(),
            "debts": debts,
        }))
        .unwrap_or_default(),
    )]))
}

pub async fn get(
    sync: &SyncManager,
    params: DebtsGetParams,
) -> Result<CallToolResult, McpError> {
    let data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let debt = data
        .today
        .debts
        .iter()
        .find(|d| d.id == params.debt_id)
        .ok_or_else(|| {
            McpError::internal_error(format!("Debt not found: {}", params.debt_id), None)
        })?;

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&debt).unwrap_or_default(),
    )]))
}

pub async fn create(
    sync: &SyncManager,
    params: DebtsCreateParams,
) -> Result<CallToolResult, McpError> {
    let mut data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let mut debt_data = params.data.clone();
    let debt_id = debt_data
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("debt_{}", chrono::Utc::now().timestamp_millis()));
    debt_data.insert("id".to_string(), json!(debt_id));

    let debt: DebtAccount =
        serde_json::from_value(json!(debt_data)).map_err(|e| {
            McpError::internal_error(format!("Invalid debt data: {}", e), None)
        })?;

    data.today.debts.push(debt);

    let new_finances = serde_json::to_value(&data.today).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize finances: {}", e), None)
    })?;

    sync.update_current_finances(new_finances)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Debt created successfully with ID: {}",
        debt_id
    ))]))
}

pub async fn update(
    sync: &SyncManager,
    params: DebtsUpdateParams,
) -> Result<CallToolResult, McpError> {
    let mut data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let debt = data
        .today
        .debts
        .iter_mut()
        .find(|d| d.id == params.debt_id)
        .ok_or_else(|| {
            McpError::internal_error(format!("Debt not found: {}", params.debt_id), None)
        })?;

    merge_json_fields(debt, &params.data)?;

    let new_finances = serde_json::to_value(&data.today).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize finances: {}", e), None)
    })?;

    sync.update_current_finances(new_finances)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Debt {} updated successfully",
        params.debt_id
    ))]))
}

pub async fn delete(
    sync: &SyncManager,
    params: DebtsDeleteParams,
) -> Result<CallToolResult, McpError> {
    let mut data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let idx = data
        .today
        .debts
        .iter()
        .position(|d| d.id == params.debt_id)
        .ok_or_else(|| {
            McpError::internal_error(format!("Debt not found: {}", params.debt_id), None)
        })?;

    data.today.debts.remove(idx);

    let new_finances = serde_json::to_value(&data.today).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize finances: {}", e), None)
    })?;

    sync.update_current_finances(new_finances)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Debt {} deleted successfully",
        params.debt_id
    ))]))
}
