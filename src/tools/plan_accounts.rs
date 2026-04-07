//! Handler logic for plan-level account event tools.
//!
//! Plan account events reference Current Finances accounts via an `account_id`
//! field. ProjectionLab re-derives plan account balances from Current Finances
//! on export, so updating plan-level data alone doesn't persist. When a plan
//! account has a linked `account_id`, we propagate overlapping fields (balance,
//! growth rates, etc.) to the Current Finances account as well.

use crate::sync::SyncManager;
use rmcp::{model::*, ErrorData as McpError};
use serde_json::{json, Map as JsonMap, Value as JsonValue};

use super::accounts::merge_json_fields;
use super::params::*;

/// Fields that exist on both plan AccountEvent and Current Finances StartingAccount.
/// Updates to these fields must be propagated to the linked Current Finances account.
const SHARED_ACCOUNT_FIELDS: &[&str] = &[
    "balance",
    "investmentGrowthRate",
    "investmentGrowthType",
    "dividendRate",
    "dividendType",
    "liquid",
    "withdraw",
    "withdrawAge",
    "isPassiveIncome",
    "name",
    "title",
    "icon",
    "color",
    "costBasis",
    "subtitle",
    "yearlyFee",
    "yearlyFeeType",
    "dividendReinvestment",
    "dividendTaxType",
    "dividendsArePassiveIncome",
    "repurpose",
];

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
                "account_id": a.account_id,
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

    // Grab the linked Current Finances account ID before mutating
    let linked_account_id = account.account_id.clone();

    // Extract shared fields that need to propagate to Current Finances
    let shared_updates: JsonMap<String, JsonValue> = params
        .data
        .iter()
        .filter(|(k, _)| SHARED_ACCOUNT_FIELDS.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    merge_json_fields(account, &params.data)?;

    let plans_value = serde_json::to_value(&data.plans).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize plans: {}", e), None)
    })?;

    sync.update_plans(plans_value)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    // Propagate shared fields to the linked Current Finances account
    let mut cf_updated = false;
    if let Some(ref cf_id) = linked_account_id {
        if !shared_updates.is_empty() {
            // Re-fetch data since cache was invalidated by update_plans
            let mut data = sync
                .get_data()
                .await
                .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

            let found = try_merge_current_finances(&mut data.today, cf_id, &shared_updates)?;

            if found {
                let new_finances = serde_json::to_value(&data.today).map_err(|e| {
                    McpError::internal_error(
                        format!("Failed to serialize finances: {}", e),
                        None,
                    )
                })?;

                sync.update_current_finances(new_finances)
                    .await
                    .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

                cf_updated = true;
            }
        }
    }

    let msg = if cf_updated {
        format!(
            "Plan account event '{}' updated successfully (also synced to Current Finances account '{}')",
            params.event_id,
            linked_account_id.as_deref().unwrap_or("unknown")
        )
    } else {
        format!(
            "Plan account event '{}' updated successfully",
            params.event_id
        )
    };

    Ok(CallToolResult::success(vec![Content::text(msg)]))
}

/// Try to find and merge updates into the matching Current Finances account
/// (savings or investment). Returns true if the account was found and updated.
fn try_merge_current_finances(
    today: &mut crate::models::plan::StartingConditions,
    account_id: &str,
    updates: &JsonMap<String, JsonValue>,
) -> Result<bool, McpError> {
    if let Some(account) = today
        .savings_accounts
        .iter_mut()
        .find(|a| a.id == account_id)
    {
        merge_json_fields(account, updates)?;
        return Ok(true);
    }

    if let Some(account) = today
        .investment_accounts
        .iter_mut()
        .find(|a| a.id == account_id)
    {
        merge_json_fields(account, updates)?;
        return Ok(true);
    }

    Ok(false)
}
