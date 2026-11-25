//! Current Finances tools: accounts, debts, and real assets.
//!
//! ProjectionLab V5 keeps all three in one `currentFinances.accounts` map,
//! distinguished by `kind` ("account", "debt", "real-asset"). The debts_* and
//! real_assets_* tools are kind-filtered views over these same handlers.
//!
//! Write strategy:
//! - value fields (balance, costBasis, value, purchasePrice) → `updateAccountValues`,
//!   which also makes ProjectionLab record account history;
//! - other fields → `updateAccount` (with `force`, so new optional fields may be set);
//! - create / unarchive → edit the raw Current Finances document and `restoreCurrentFinances`;
//! - removal → archive (`archivedOn`). The V5 Plugin API cannot hard-delete
//!   accounts (restoreCurrentFinances merges accounts rather than replacing the map).

use crate::models::current_finances::{
    CurrentFinancesAccount, ACCOUNT_TYPES, DEBT_TYPES, KIND_ACCOUNT, KIND_DEBT, KIND_REAL_ASSET,
    REAL_ASSET_TYPES,
};
use crate::models::account_history::flatten_history;
use crate::models::FullExport;
use crate::sync::SyncManager;
use serde_json::{json, Map as JsonMap, Value as JsonValue};

use super::doc::{self, anyhow_err, err, ok_json, ToolResult};
use super::params::*;

/// Fields that must go through `updateAccountValues`.
pub const VALUE_FIELDS: &[&str] = &["balance", "costBasis", "value", "purchasePrice"];

/// Optional fields that may be added to an account that doesn't have them yet.
/// Anything else must already exist on the account (guards against typos).
const KNOWN_OPTIONAL_FIELDS: &[&str] = &[
    "name",
    "owner",
    "icon",
    "subtype",
    "order",
    "interestRate",
    "monthlyPayment",
    "securedByAssetId",
];

/// Filter for listing Current Finances items.
#[derive(Default)]
pub struct AccountFilter<'a> {
    pub kind: Option<&'a str>,
    pub account_type: Option<&'a str>,
    pub owner: Option<&'a str>,
    pub include_archived: bool,
}

impl AccountFilter<'_> {
    fn matches(&self, a: &CurrentFinancesAccount) -> bool {
        if !self.include_archived && a.is_archived() {
            return false;
        }
        if let Some(kind) = self.kind {
            if a.kind != kind {
                return false;
            }
        }
        if let Some(t) = self.account_type {
            let ok = match t {
                "investment" => a.kind == KIND_ACCOUNT && a.account_type != "savings",
                other => a.account_type == other,
            };
            if !ok {
                return false;
            }
        }
        if let Some(owner) = self.owner {
            if a.owner.as_deref() != Some(owner) {
                return false;
            }
        }
        true
    }
}

/// Compact JSON summary of an item for list output.
pub fn account_summary(a: &CurrentFinancesAccount) -> JsonValue {
    let mut out = json!({
        "id": a.id,
        "name": a.name,
        "kind": a.kind,
        "type": a.account_type,
    });
    let obj = out.as_object_mut().expect("object literal");
    let mut put = |k: &str, v: Option<JsonValue>| {
        if let Some(v) = v {
            obj.insert(k.to_string(), v);
        }
    };
    put("subtype", a.subtype.clone().map(JsonValue::from));
    put("owner", a.owner.clone().map(JsonValue::from));
    put("balance", a.balance.map(JsonValue::from));
    put("costBasis", a.cost_basis.map(JsonValue::from));
    put("value", a.value.map(JsonValue::from));
    put("purchasePrice", a.purchase_price.map(JsonValue::from));
    for key in ["interestRate", "monthlyPayment", "securedByAssetId"] {
        put(key, a.extra.get(key).cloned());
    }
    put("archivedOn", a.archived_on.clone().map(JsonValue::from));
    out
}

pub async fn list_filtered(sync: &SyncManager, filter: AccountFilter<'_>) -> ToolResult {
    let data = sync.get_data().await.map_err(anyhow_err)?;
    let cf = data.current_finances();
    let items: Vec<&CurrentFinancesAccount> = cf
        .sorted_accounts()
        .into_iter()
        .filter(|a| filter.matches(a))
        .collect();

    let total_assets: f64 = items.iter().filter(|a| !a.is_debt()).map(|a| a.current_value()).sum();
    let total_debt: f64 = items.iter().filter(|a| a.is_debt()).map(|a| a.current_value()).sum();

    ok_json(&json!({
        "total_count": items.len(),
        "total_assets": (total_assets * 100.0).round() / 100.0,
        "total_debt": (total_debt * 100.0).round() / 100.0,
        "accounts": items.iter().map(|a| account_summary(a)).collect::<Vec<_>>(),
    }))
}

pub async fn list(sync: &SyncManager, params: AccountsListParams) -> ToolResult {
    list_filtered(
        sync,
        AccountFilter {
            kind: params.kind.as_deref(),
            account_type: params.account_type.as_deref(),
            owner: params.owner.as_deref(),
            include_archived: params.include_archived,
        },
    )
    .await
}

/// Look up an account in parsed data, optionally requiring a kind.
fn find_account<'a>(
    data: &'a FullExport,
    account_id: &str,
    kind: Option<&str>,
) -> Result<&'a CurrentFinancesAccount, rmcp::ErrorData> {
    let account = data
        .current_finances()
        .find(account_id)
        .ok_or_else(|| err(format!("Account not found: {}", account_id)))?;
    if let Some(kind) = kind {
        if account.kind != kind {
            return Err(err(format!(
                "Account {} is a `{}`, not a `{}`",
                account_id, account.kind, kind
            )));
        }
    }
    Ok(account)
}

pub async fn get_kind(sync: &SyncManager, account_id: &str, kind: Option<&str>) -> ToolResult {
    let data = sync.get_data().await.map_err(anyhow_err)?;
    let account = find_account(&data, account_id, kind)?;

    let recent_history = data
        .documents
        .account_history
        .get(account_id)
        .map(|h| {
            let points = flatten_history(h, None, None);
            let skip = points.len().saturating_sub(5);
            points.into_iter().skip(skip).collect::<Vec<_>>()
        })
        .unwrap_or_default();

    ok_json(&json!({
        "account": account,
        "recent_history": recent_history,
    }))
}

pub async fn get(sync: &SyncManager, params: AccountsGetParams) -> ToolResult {
    get_kind(sync, &params.account_id, None).await
}

fn types_for_kind(kind: &str) -> Option<&'static [&'static str]> {
    match kind {
        KIND_ACCOUNT => Some(ACCOUNT_TYPES),
        KIND_DEBT => Some(DEBT_TYPES),
        KIND_REAL_ASSET => Some(REAL_ASSET_TYPES),
        _ => None,
    }
}

/// Resolve the kind for a new item, validating the type against it.
fn resolve_kind(kind: Option<&str>, account_type: &str) -> Result<&'static str, rmcp::ErrorData> {
    let all_types = || {
        format!(
            "Account types: {}. Debt types: {}. Real asset types: {}.",
            ACCOUNT_TYPES.join(", "),
            DEBT_TYPES.join(", "),
            REAL_ASSET_TYPES.join(", ")
        )
    };
    if let Some(kind) = kind {
        let kind = [KIND_ACCOUNT, KIND_DEBT, KIND_REAL_ASSET]
            .into_iter()
            .find(|k| *k == kind)
            .ok_or_else(|| err(format!("Invalid kind `{}` (expected account, debt, or real-asset)", kind)))?;
        let types = types_for_kind(kind).unwrap_or_default();
        if !types.contains(&account_type) {
            return Err(err(format!(
                "`{}` is not a valid {} type. Valid: {}",
                account_type,
                kind,
                types.join(", ")
            )));
        }
        return Ok(kind);
    }
    let candidates: Vec<&'static str> = [KIND_ACCOUNT, KIND_DEBT, KIND_REAL_ASSET]
        .into_iter()
        .filter(|k| types_for_kind(k).unwrap_or_default().contains(&account_type))
        .collect();
    match candidates.as_slice() {
        [one] => Ok(one),
        [] => Err(err(format!("Unknown type `{}`. {}", account_type, all_types()))),
        _ => Err(err(format!(
            "Type `{}` is ambiguous; pass kind ({})",
            account_type,
            candidates.join(" or ")
        ))),
    }
}

/// Create a Current Finances item. `kind` may be forced by the debts_/real_assets_ wrappers.
pub async fn create_item(
    sync: &SyncManager,
    kind: Option<&str>,
    account_type: Option<&str>,
    data: JsonMap<String, JsonValue>,
) -> ToolResult {
    // Resolve type: explicit param, else data.type.
    let account_type = match account_type {
        None => data
            .get("type")
            .and_then(JsonValue::as_str)
            .map(str::to_string)
            .ok_or_else(|| {
                err("Missing account type: pass a ProjectionLab type such as \"taxable\", \"401k\", \"mortgage\" or \"real-estate\"")
            })?,
        Some(t) => t.to_string(),
    };

    let kind = resolve_kind(kind, &account_type)?.to_string();

    let prefix = match kind.as_str() {
        KIND_DEBT => "debt",
        KIND_REAL_ASSET => "asset",
        _ => "acc",
    };
    let id = data
        .get("id")
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| doc::new_id(prefix));

    let mut raw = sync.get_raw().await.map_err(anyhow_err)?;
    let cf = doc::current_finances_mut(&mut raw)?;
    let accounts = cf
        .as_object_mut()
        .ok_or_else(|| err("currentFinances is not an object"))?
        .entry("accounts")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| err("currentFinances.accounts is not an object"))?;
    if accounts.contains_key(&id) {
        return Err(err(format!("An account with id {} already exists", id)));
    }

    // Place the new item after existing items of the same kind.
    let next_order = accounts
        .values()
        .filter(|a| a.get("kind").and_then(JsonValue::as_str) == Some(kind.as_str()))
        .filter_map(|a| a.get("order").and_then(JsonValue::as_i64))
        .max()
        .map_or(0, |m| m + 1);

    let mut account = json!({
        "id": id,
        "kind": kind,
        "type": account_type,
        "name": account_type,
        "order": next_order,
    });
    match kind.as_str() {
        KIND_REAL_ASSET => {
            account["value"] = json!(0);
        }
        _ => {
            account["balance"] = json!(0);
            account["owner"] = json!("me");
        }
    }
    doc::merge_shallow(&mut account, &data)?;
    account["id"] = json!(id);
    account["kind"] = json!(kind);
    account["type"] = json!(account_type);
    accounts.insert(id.clone(), account.clone());

    let cf = doc::take_current_finances(&mut raw)?;
    sync.restore_current_finances(cf).await.map_err(anyhow_err)?;

    ok_json(&json!({
        "message": format!("Created {} `{}` ({})", kind, account["name"].as_str().unwrap_or(""), id),
        "account": account,
    }))
}

pub async fn create(sync: &SyncManager, params: AccountsCreateParams) -> ToolResult {
    create_item(
        sync,
        params.kind.as_deref(),
        Some(params.account_type.as_str()),
        params.data,
    )
    .await
}

/// Update a Current Finances item: value fields via updateAccountValues
/// (records history), everything else via updateAccount.
pub async fn update_item(
    sync: &SyncManager,
    account_id: &str,
    kind: Option<&str>,
    data: JsonMap<String, JsonValue>,
) -> ToolResult {
    doc::reject_id_change(&data, account_id)?;
    if data.contains_key("kind") {
        return Err(err("Changing `kind` is not supported"));
    }
    if data.contains_key("archivedOn") {
        return Err(err("Use accounts_archive / accounts_unarchive to change archive status"));
    }
    {
        let parsed = sync.get_data().await.map_err(anyhow_err)?;
        let account = find_account(&parsed, account_id, kind)?;
        if account.is_archived() && data.keys().any(|k| VALUE_FIELDS.contains(&k.as_str())) {
            return Err(err(format!(
                "Account {} is archived; unarchive it before updating its values",
                account_id
            )));
        }
        let existing = serde_json::to_value(account).unwrap_or_default();
        let unknown: Vec<&str> = data
            .keys()
            .map(String::as_str)
            .filter(|k| {
                existing.get(*k).is_none()
                    && !KNOWN_OPTIONAL_FIELDS.contains(k)
                    && !VALUE_FIELDS.contains(k)
            })
            .collect();
        if !unknown.is_empty() {
            return Err(err(format!(
                "Unknown field(s) for account {}: {}. Existing fields: {}. Addable fields: {}. (Use update_account with force=true to set arbitrary fields.)",
                account_id,
                unknown.join(", "),
                existing.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>().join(", ")).unwrap_or_default(),
                KNOWN_OPTIONAL_FIELDS.join(", ")
            )));
        }
    }

    let (values, other): (JsonMap<_, _>, JsonMap<_, _>) = data
        .into_iter()
        .partition(|(k, _)| VALUE_FIELDS.contains(&k.as_str()));

    if !other.is_empty() {
        sync.update_account(account_id, JsonValue::Object(other.clone()), true)
            .await
            .map_err(anyhow_err)?;
    }
    if !values.is_empty() {
        sync.update_account_values(account_id, JsonValue::Object(values.clone()))
            .await
            .map_err(anyhow_err)?;
    }

    ok_json(&json!({
        "message": format!("Account {} updated", account_id),
        "updated_values": values,
        "updated_fields": other,
    }))
}

pub async fn update(sync: &SyncManager, params: AccountsUpdateParams) -> ToolResult {
    update_item(sync, &params.account_id, None, params.data).await
}

pub async fn update_balance(sync: &SyncManager, params: AccountsUpdateBalanceParams) -> ToolResult {
    let data = sync.get_data().await.map_err(anyhow_err)?;
    let account = find_account(&data, &params.account_id, None)?;
    let field = account.value_field();
    let previous = account.current_value();
    drop(data);

    sync.update_account_values(&params.account_id, json!({ field: params.balance }))
        .await
        .map_err(anyhow_err)?;

    ok_json(&json!({
        "message": format!("Account {} {} updated to {}", params.account_id, field, params.balance),
        "previous": previous,
        "current": params.balance,
    }))
}

pub async fn archive_item(
    sync: &SyncManager,
    account_id: &str,
    kind: Option<&str>,
    date: Option<String>,
) -> ToolResult {
    let date = date.unwrap_or_else(doc::today_iso);
    chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d")
        .map_err(|_| err(format!("Invalid date `{}` (expected YYYY-MM-DD)", date)))?;
    {
        let data = sync.get_data().await.map_err(anyhow_err)?;
        let account = find_account(&data, account_id, kind)?;
        if account.is_archived() {
            return Err(err(format!(
                "Account {} is already archived (on {})",
                account_id,
                account.archived_on.as_deref().unwrap_or("?")
            )));
        }
    }
    sync.update_account(account_id, json!({ "archivedOn": date }), true)
        .await
        .map_err(anyhow_err)?;
    ok_json(&json!({
        "message": format!(
            "Account {} archived as of {}. It no longer counts toward net worth; its history is kept. Use accounts_unarchive to restore it.",
            account_id, date
        ),
    }))
}

pub async fn archive(sync: &SyncManager, params: AccountsArchiveParams) -> ToolResult {
    archive_item(sync, &params.account_id, None, params.date).await
}

pub async fn unarchive(sync: &SyncManager, params: AccountsUnarchiveParams) -> ToolResult {
    let mut raw = sync.get_raw().await.map_err(anyhow_err)?;
    let cf = doc::current_finances_mut(&mut raw)?;
    let account = cf
        .get_mut("accounts")
        .and_then(|a| a.get_mut(&params.account_id))
        .and_then(JsonValue::as_object_mut)
        .ok_or_else(|| err(format!("Account not found: {}", params.account_id)))?;
    if account.remove("archivedOn").is_none() {
        return Err(err(format!("Account {} is not archived", params.account_id)));
    }
    let cf = doc::take_current_finances(&mut raw)?;
    sync.restore_current_finances(cf).await.map_err(anyhow_err)?;
    ok_json(&json!({ "message": format!("Account {} unarchived", params.account_id) }))
}

pub async fn sync_balances(sync: &SyncManager, params: SyncAccountBalancesParams) -> ToolResult {
    let data = sync.get_data().await.map_err(anyhow_err)?;

    let mut updated = Vec::new();
    let mut unchanged = Vec::new();
    let mut failed = Vec::new();

    // Resolve everything against one snapshot before writing.
    let mut plan: Vec<(String, &'static str, f64, f64)> = Vec::new();
    for mapping in &params.mappings {
        match data.current_finances().find(&mapping.pl_account_id) {
            None => failed.push(json!({"id": mapping.pl_account_id, "error": "not found"})),
            Some(a) if a.is_archived() => {
                failed.push(json!({"id": mapping.pl_account_id, "error": "account is archived"}))
            }
            Some(a) if (a.current_value() - mapping.balance).abs() < 0.005 => {
                unchanged.push(json!({"id": a.id, "name": a.name, "balance": mapping.balance}))
            }
            Some(a) => plan.push((a.id.clone(), a.value_field(), a.current_value(), mapping.balance)),
        }
    }
    let names: std::collections::HashMap<String, String> = data
        .current_finances()
        .accounts
        .values()
        .map(|a| (a.id.clone(), a.name.clone()))
        .collect();
    drop(data);

    for (id, field, previous, balance) in plan {
        match sync.update_account_values(&id, json!({ field: balance })).await {
            Ok(()) => updated.push(json!({
                "id": id,
                "name": names.get(&id),
                "previous": previous,
                "current": balance,
                "delta": balance - previous,
            })),
            Err(e) => failed.push(json!({"id": id, "error": format!("{:#}", e)})),
        }
    }

    ok_json(&json!({
        "updated_count": updated.len(),
        "unchanged_count": unchanged.len(),
        "failed_count": failed.len(),
        "updated": updated,
        "unchanged": unchanged,
        "failed": failed,
        "note": "ProjectionLab records account and net worth history automatically for these updates.",
    }))
}
