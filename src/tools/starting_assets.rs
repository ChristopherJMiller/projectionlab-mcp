//! Handler logic for starting asset tools (Current Finances).

use crate::models::StartingAsset;
use crate::sync::SyncManager;
use rmcp::{model::*, ErrorData as McpError};
use serde_json::json;

use super::accounts::merge_json_fields;
use super::params::*;

pub async fn list(
    sync: &SyncManager,
    params: StartingAssetsListParams,
) -> Result<CallToolResult, McpError> {
    let data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let assets: Vec<_> = data
        .today
        .assets
        .iter()
        .filter(|a| {
            if let Some(ref filter_type) = params.asset_type {
                if &a.asset_type != filter_type {
                    return false;
                }
            }
            if let Some(ref filter_owner) = params.owner {
                let owner_str = format!("{:?}", a.owner).to_lowercase();
                if &owner_str != filter_owner {
                    return false;
                }
            }
            true
        })
        .map(|a| {
            json!({
                "id": a.id,
                "name": a.name,
                "type": a.asset_type,
                "balance": a.balance,
                "initial_value": a.initial_value,
                "owner": a.owner,
                "interest_rate": a.interest_rate,
                "monthly_payment": a.monthly_payment,
                "liquid": a.liquid,
            })
        })
        .collect();

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&json!({
            "total_count": assets.len(),
            "assets": assets,
        }))
        .unwrap_or_default(),
    )]))
}

pub async fn get(
    sync: &SyncManager,
    params: StartingAssetsGetParams,
) -> Result<CallToolResult, McpError> {
    let data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let asset = data
        .today
        .assets
        .iter()
        .find(|a| a.id == params.asset_id)
        .ok_or_else(|| {
            McpError::internal_error(format!("Starting asset not found: {}", params.asset_id), None)
        })?;

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&asset).unwrap_or_default(),
    )]))
}

pub async fn create(
    sync: &SyncManager,
    params: StartingAssetsCreateParams,
) -> Result<CallToolResult, McpError> {
    let mut data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let mut asset_data = params.data.clone();
    let asset_id = asset_data
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("asset_{}", chrono::Utc::now().timestamp_millis()));
    asset_data.insert("id".to_string(), json!(asset_id));

    let asset: StartingAsset =
        serde_json::from_value(json!(asset_data)).map_err(|e| {
            McpError::internal_error(format!("Invalid starting asset data: {}", e), None)
        })?;

    data.today.assets.push(asset);

    let new_finances = serde_json::to_value(&data.today).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize finances: {}", e), None)
    })?;

    sync.update_current_finances(new_finances)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Starting asset created successfully with ID: {}",
        asset_id
    ))]))
}

pub async fn update(
    sync: &SyncManager,
    params: StartingAssetsUpdateParams,
) -> Result<CallToolResult, McpError> {
    let mut data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let asset = data
        .today
        .assets
        .iter_mut()
        .find(|a| a.id == params.asset_id)
        .ok_or_else(|| {
            McpError::internal_error(format!("Starting asset not found: {}", params.asset_id), None)
        })?;

    merge_json_fields(asset, &params.data)?;

    let new_finances = serde_json::to_value(&data.today).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize finances: {}", e), None)
    })?;

    sync.update_current_finances(new_finances)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Starting asset {} updated successfully",
        params.asset_id
    ))]))
}

pub async fn delete(
    sync: &SyncManager,
    params: StartingAssetsDeleteParams,
) -> Result<CallToolResult, McpError> {
    let mut data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let idx = data
        .today
        .assets
        .iter()
        .position(|a| a.id == params.asset_id)
        .ok_or_else(|| {
            McpError::internal_error(format!("Starting asset not found: {}", params.asset_id), None)
        })?;

    data.today.assets.remove(idx);

    let new_finances = serde_json::to_value(&data.today).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize finances: {}", e), None)
    })?;

    sync.update_current_finances(new_finances)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Starting asset {} deleted successfully",
        params.asset_id
    ))]))
}
