//! Real asset tools (homes, cars, valuables): a `kind: "real-asset"` view over
//! the Current Finances account tools.

use crate::models::current_finances::KIND_REAL_ASSET;
use crate::sync::SyncManager;

use super::accounts::{self, AccountFilter};
use super::doc::ToolResult;
use super::params::*;

pub async fn list(sync: &SyncManager, params: StartingAssetsListParams) -> ToolResult {
    accounts::list_filtered(
        sync,
        AccountFilter {
            kind: Some(KIND_REAL_ASSET),
            account_type: params.asset_type.as_deref(),
            owner: params.owner.as_deref(),
            include_archived: params.include_archived,
        },
    )
    .await
}

pub async fn get(sync: &SyncManager, params: StartingAssetsGetParams) -> ToolResult {
    accounts::get_kind(sync, &params.asset_id, Some(KIND_REAL_ASSET)).await
}

pub async fn create(sync: &SyncManager, params: StartingAssetsCreateParams) -> ToolResult {
    accounts::create_item(sync, Some(KIND_REAL_ASSET), None, params.data).await
}

pub async fn update(sync: &SyncManager, params: StartingAssetsUpdateParams) -> ToolResult {
    accounts::update_item(sync, &params.asset_id, Some(KIND_REAL_ASSET), params.data).await
}

pub async fn delete(sync: &SyncManager, params: StartingAssetsDeleteParams) -> ToolResult {
    accounts::archive_item(sync, &params.asset_id, Some(KIND_REAL_ASSET), None).await
}
