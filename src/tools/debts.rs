//! Debt tools: a `kind: "debt"` view over the Current Finances account tools.

use crate::models::current_finances::KIND_DEBT;
use crate::sync::SyncManager;

use super::accounts::{self, AccountFilter};
use super::doc::ToolResult;
use super::params::*;

pub async fn list(sync: &SyncManager, params: DebtsListParams) -> ToolResult {
    accounts::list_filtered(
        sync,
        AccountFilter {
            kind: Some(KIND_DEBT),
            account_type: params.debt_type.as_deref(),
            owner: params.owner.as_deref(),
            include_archived: params.include_archived,
        },
    )
    .await
}

pub async fn get(sync: &SyncManager, params: DebtsGetParams) -> ToolResult {
    accounts::get_kind(sync, &params.debt_id, Some(KIND_DEBT)).await
}

pub async fn create(sync: &SyncManager, params: DebtsCreateParams) -> ToolResult {
    accounts::create_item(sync, Some(KIND_DEBT), None, params.data).await
}

pub async fn update(sync: &SyncManager, params: DebtsUpdateParams) -> ToolResult {
    accounts::update_item(sync, &params.debt_id, Some(KIND_DEBT), params.data).await
}

pub async fn delete(sync: &SyncManager, params: DebtsDeleteParams) -> ToolResult {
    accounts::archive_item(sync, &params.debt_id, Some(KIND_DEBT), None).await
}
