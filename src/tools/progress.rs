//! Handler logic for progress tracking tools.

use crate::models;
use crate::sync::SyncManager;
use rmcp::{model::*, ErrorData as McpError};
use serde_json::json;

use super::params::*;

pub async fn add_data_point(
    sync: &SyncManager,
    params: ProgressAddDataPointParams,
) -> Result<CallToolResult, McpError> {
    let mut data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let data_point = models::ProgressDataPoint {
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

    sync.update_progress(progress_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(
        "Progress data point added successfully".to_string(),
    )]))
}

pub async fn get_history(
    sync: &SyncManager,
    params: ProgressGetHistoryParams,
) -> Result<CallToolResult, McpError> {
    let data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let filtered: Vec<&models::ProgressDataPoint> = data
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
