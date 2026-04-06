//! Handler logic for Current Finances account tools (savings, investment, debt).

use crate::models;
use crate::sync::SyncManager;
use rmcp::{model::*, ErrorData as McpError};
use serde_json::{json, Map as JsonMap, Value as JsonValue};

use super::params::*;

pub async fn list(
    sync: &SyncManager,
    params: AccountsListParams,
) -> Result<CallToolResult, McpError> {
    let data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let mut accounts = Vec::new();

    for account in &data.today.savings_accounts {
        if let Some(ref filter_type) = params.account_type {
            if filter_type != "savings" {
                continue;
            }
        }
        if let Some(ref filter_owner) = params.owner {
            let owner_str = format!("{:?}", account.owner).to_lowercase();
            if &owner_str != filter_owner {
                continue;
            }
        }
        accounts.push(json!({
            "id": account.id,
            "name": account.name,
            "type": "savings",
            "balance": account.balance,
            "owner": account.owner,
            "liquid": account.liquid,
        }));
    }

    for account in &data.today.investment_accounts {
        if let Some(ref filter_type) = params.account_type {
            if filter_type != "investment" && filter_type != &account.account_type {
                continue;
            }
        }
        if let Some(ref filter_owner) = params.owner {
            let owner_str = format!("{:?}", account.owner).to_lowercase();
            if &owner_str != filter_owner {
                continue;
            }
        }
        accounts.push(json!({
            "id": account.id,
            "name": account.name,
            "type": account.account_type,
            "balance": account.balance,
            "owner": account.owner,
            "liquid": account.liquid,
            "cost_basis": account.cost_basis,
        }));
    }

    for debt in &data.today.debts {
        if let Some(ref filter_type) = params.account_type {
            if filter_type != "debt" && filter_type != &debt.debt_type {
                continue;
            }
        }
        if let Some(ref filter_owner) = params.owner {
            let owner_str = format!("{:?}", debt.owner).to_lowercase();
            if &owner_str != filter_owner {
                continue;
            }
        }
        accounts.push(json!({
            "id": debt.id,
            "name": debt.name,
            "type": debt.debt_type,
            "balance": debt.balance,
            "owner": debt.owner,
            "interest_rate": debt.interest_rate,
            "monthly_payment": debt.monthly_payment,
        }));
    }

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&json!({
            "total_count": accounts.len(),
            "accounts": accounts,
        }))
        .unwrap_or_default(),
    )]))
}

pub async fn get(
    sync: &SyncManager,
    params: AccountsGetParams,
) -> Result<CallToolResult, McpError> {
    let data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    if let Some(account) = data
        .today
        .savings_accounts
        .iter()
        .find(|a| a.id == params.account_id)
    {
        return Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "id": account.id,
                "name": account.name,
                "type": "savings",
                "balance": account.balance,
                "owner": account.owner,
                "liquid": account.liquid,
                "icon": account.icon,
                "color": account.color,
            }))
            .unwrap_or_default(),
        )]));
    }

    if let Some(account) = data
        .today
        .investment_accounts
        .iter()
        .find(|a| a.id == params.account_id)
    {
        return Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "id": account.id,
                "name": account.name,
                "type": account.account_type,
                "balance": account.balance,
                "owner": account.owner,
                "liquid": account.liquid,
                "icon": account.icon,
                "color": account.color,
                "cost_basis": account.cost_basis,
            }))
            .unwrap_or_default(),
        )]));
    }

    if let Some(debt) = data
        .today
        .debts
        .iter()
        .find(|d| d.id == params.account_id)
    {
        return Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&json!({
                "id": debt.id,
                "name": debt.name,
                "type": debt.debt_type,
                "balance": debt.balance,
                "owner": debt.owner,
                "icon": debt.icon,
                "color": debt.color,
                "interest_rate": debt.interest_rate,
                "monthly_payment": debt.monthly_payment,
            }))
            .unwrap_or_default(),
        )]));
    }

    Err(McpError::internal_error(
        format!("Account not found: {}", params.account_id),
        None,
    ))
}

pub async fn create(
    sync: &SyncManager,
    params: AccountsCreateParams,
) -> Result<CallToolResult, McpError> {
    let mut data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let account_id = params
        .data
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("acc_{}", chrono::Utc::now().timestamp_millis()));

    let mut new_account = params.data.clone();
    new_account.insert("id".to_string(), json!(account_id));

    match params.account_type.as_str() {
        "savings" => {
            let account: models::SavingsAccount =
                serde_json::from_value(json!(new_account)).map_err(|e| {
                    McpError::internal_error(
                        format!("Invalid savings account data: {}", e),
                        None,
                    )
                })?;
            data.today.savings_accounts.push(account);
        }
        "investment" => {
            let account: models::InvestmentAccount =
                serde_json::from_value(json!(new_account)).map_err(|e| {
                    McpError::internal_error(
                        format!("Invalid investment account data: {}", e),
                        None,
                    )
                })?;
            data.today.investment_accounts.push(account);
        }
        "debt" => {
            let debt: models::DebtAccount =
                serde_json::from_value(json!(new_account)).map_err(|e| {
                    McpError::internal_error(format!("Invalid debt account data: {}", e), None)
                })?;
            data.today.debts.push(debt);
        }
        _ => {
            return Err(McpError::internal_error(
                format!(
                    "Invalid account type: {}. Must be 'savings', 'investment', or 'debt'",
                    params.account_type
                ),
                None,
            ));
        }
    }

    let new_finances = serde_json::to_value(&data.today).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize finances: {}", e), None)
    })?;

    sync.update_current_finances(new_finances)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Account created successfully with ID: {}",
        account_id
    ))]))
}

pub async fn update(
    sync: &SyncManager,
    params: AccountsUpdateParams,
) -> Result<CallToolResult, McpError> {
    let mut data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let mut found = false;

    if let Some(account) = data
        .today
        .savings_accounts
        .iter_mut()
        .find(|a| a.id == params.account_id)
    {
        merge_json_fields(account, &params.data)?;
        found = true;
    }

    if !found {
        if let Some(account) = data
            .today
            .investment_accounts
            .iter_mut()
            .find(|a| a.id == params.account_id)
        {
            merge_json_fields(account, &params.data)?;
            found = true;
        }
    }

    if !found {
        if let Some(debt) = data
            .today
            .debts
            .iter_mut()
            .find(|d| d.id == params.account_id)
        {
            merge_json_fields(debt, &params.data)?;
            found = true;
        }
    }

    if !found {
        return Err(McpError::internal_error(
            format!("Account not found: {}", params.account_id),
            None,
        ));
    }

    let new_finances = serde_json::to_value(&data.today).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize finances: {}", e), None)
    })?;

    sync.update_current_finances(new_finances)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Account {} updated successfully",
        params.account_id
    ))]))
}

pub async fn update_balance(
    sync: &SyncManager,
    params: AccountsUpdateBalanceParams,
) -> Result<CallToolResult, McpError> {
    let mut data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let mut found = false;

    if let Some(account) = data
        .today
        .savings_accounts
        .iter_mut()
        .find(|a| a.id == params.account_id)
    {
        account.balance = params.balance;
        found = true;
    }

    if !found {
        if let Some(account) = data
            .today
            .investment_accounts
            .iter_mut()
            .find(|a| a.id == params.account_id)
        {
            account.balance = params.balance;
            found = true;
        }
    }

    if !found {
        if let Some(debt) = data
            .today
            .debts
            .iter_mut()
            .find(|d| d.id == params.account_id)
        {
            debt.balance = params.balance;
            found = true;
        }
    }

    if !found {
        return Err(McpError::internal_error(
            format!("Account not found: {}", params.account_id),
            None,
        ));
    }

    let new_finances = serde_json::to_value(&data.today).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize finances: {}", e), None)
    })?;

    sync.update_current_finances(new_finances)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Account {} balance updated to {}",
        params.account_id, params.balance
    ))]))
}

pub async fn delete(
    sync: &SyncManager,
    params: AccountsDeleteParams,
) -> Result<CallToolResult, McpError> {
    let mut data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let mut found = false;

    if let Some(idx) = data
        .today
        .savings_accounts
        .iter()
        .position(|a| a.id == params.account_id)
    {
        data.today.savings_accounts.remove(idx);
        found = true;
    }

    if !found {
        if let Some(idx) = data
            .today
            .investment_accounts
            .iter()
            .position(|a| a.id == params.account_id)
        {
            data.today.investment_accounts.remove(idx);
            found = true;
        }
    }

    if !found {
        if let Some(idx) = data
            .today
            .debts
            .iter()
            .position(|d| d.id == params.account_id)
        {
            data.today.debts.remove(idx);
            found = true;
        }
    }

    if !found {
        return Err(McpError::internal_error(
            format!("Account not found: {}", params.account_id),
            None,
        ));
    }

    let new_finances = serde_json::to_value(&data.today).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize finances: {}", e), None)
    })?;

    sync.update_current_finances(new_finances)
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    Ok(CallToolResult::success(vec![Content::text(format!(
        "Account {} deleted successfully",
        params.account_id
    ))]))
}

pub async fn sync_balances(
    sync: &SyncManager,
    params: SyncAccountBalancesParams,
) -> Result<CallToolResult, McpError> {
    let mut data = sync
        .get_data()
        .await
        .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;

    let mut updated = Vec::new();
    let mut not_found = Vec::new();

    for mapping in &params.mappings {
        let mut found = false;

        if let Some(account) = data
            .today
            .savings_accounts
            .iter_mut()
            .find(|a| a.id == mapping.pl_account_id)
        {
            account.balance = mapping.balance;
            found = true;
        }

        if !found {
            if let Some(account) = data
                .today
                .investment_accounts
                .iter_mut()
                .find(|a| a.id == mapping.pl_account_id)
            {
                account.balance = mapping.balance;
                found = true;
            }
        }

        if !found {
            if let Some(debt) = data
                .today
                .debts
                .iter_mut()
                .find(|d| d.id == mapping.pl_account_id)
            {
                debt.balance = mapping.balance;
                found = true;
            }
        }

        if found {
            updated.push(&mapping.pl_account_id);
        } else {
            not_found.push(&mapping.pl_account_id);
        }
    }

    if !updated.is_empty() {
        let new_finances = serde_json::to_value(&data.today).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize finances: {}", e), None)
        })?;

        sync.update_current_finances(new_finances)
            .await
            .map_err(|e| McpError::internal_error(format!("{:#}", e), None))?;
    }

    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(&json!({
            "updated": updated.len(),
            "not_found": not_found.len(),
            "updated_ids": updated,
            "not_found_ids": not_found,
        }))
        .unwrap_or_default(),
    )]))
}

/// Serialize a value to JSON, merge in partial updates, and deserialize back.
/// Used for partial-update (merge-patch) operations.
pub fn merge_json_fields<T: serde::Serialize + serde::de::DeserializeOwned>(
    target: &mut T,
    updates: &JsonMap<String, JsonValue>,
) -> Result<(), McpError> {
    let mut target_json = serde_json::to_value(&*target).map_err(|e| {
        McpError::internal_error(format!("Failed to serialize for merge: {}", e), None)
    })?;

    if let Some(obj) = target_json.as_object_mut() {
        for (key, value) in updates {
            obj.insert(key.clone(), value.clone());
        }
    }

    *target = serde_json::from_value(target_json).map_err(|e| {
        McpError::internal_error(format!("Failed to deserialize after merge: {}", e), None)
    })?;

    Ok(())
}
