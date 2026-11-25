//! Plan-level account settings (`plan.accounts.events`, kind "account").
//!
//! Each plan account event links to a Current Finances account via `accountId`.
//! A balance can live in up to three places:
//!   1. the plan account event itself,
//!   2. the plan's own `startingConditions.accounts[accountId]` (custom plans only),
//!   3. the global Current Finances account.
//!
//! `update` writes shared fields through to all of them so simulations and the
//! dashboard agree. Current Finances value edits go through updateAccountValues,
//! which also records account history.

use crate::sync::SyncManager;
use serde_json::{json, Map as JsonMap, Value as JsonValue};

use super::accounts::VALUE_FIELDS;
use super::doc::{self, anyhow_err, err, ok_json, ToolResult};
use super::params::*;

/// Fields shared between a plan account event and its Current Finances account.
const SHARED_ACCOUNT_FIELDS: &[&str] = &["balance", "costBasis", "name", "icon", "owner"];

const SUMMARY_FIELDS: &[&str] = &[
    "accountId",
    "owner",
    "balance",
    "costBasis",
    "liquidity",
    "withdrawAge",
    "investmentGrowthType",
    "investmentGrowthRate",
    "dividendType",
    "dividendRate",
    "yearlyFee",
    "yearlyFeeType",
    "hidden",
];

pub async fn list(sync: &SyncManager, params: PlanEventsListParams) -> ToolResult {
    let data = sync.get_data().await.map_err(anyhow_err)?;
    let plan = data
        .find_plan(&params.plan_id)
        .ok_or_else(|| err(format!("Plan not found: {}", params.plan_id)))?;
    let cf = data.current_finances();

    let accounts: Vec<JsonValue> = plan
        .accounts
        .events
        .iter()
        .map(|e| {
            let mut summary = e.summary(SUMMARY_FIELDS);
            if let Some(linked) = e.get_str("accountId") {
                if let Some(cf_account) = cf.find(linked) {
                    summary["currentFinancesBalance"] = json!(cf_account.balance);
                    summary["currentFinancesArchived"] = json!(cf_account.is_archived());
                }
                if let Some(sc) = plan.starting_conditions.find(linked) {
                    summary["planStartingBalance"] = json!(sc.balance);
                }
            }
            summary
        })
        .collect();

    ok_json(&json!({
        "plan_id": plan.id,
        "plan_name": plan.name,
        "starting_conditions_type": plan.starting_conditions_type,
        "total_accounts": accounts.len(),
        "accounts": accounts,
    }))
}

pub async fn update(sync: &SyncManager, params: PlanEventUpdateParams) -> ToolResult {
    doc::reject_id_change(&params.data, &params.event_id)?;

    let shared: JsonMap<String, JsonValue> = params
        .data
        .iter()
        .filter(|(k, _)| SHARED_ACCOUNT_FIELDS.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    // 1 + 2: plan account event and the plan's starting-conditions snapshot.
    let mut raw = sync.get_raw().await.map_err(anyhow_err)?;
    let plan = doc::plan_mut(&mut raw, &params.plan_id)?;
    let linked_id = {
        let events = doc::section_events_mut(plan, "accounts")?;
        let event = doc::find_by_id_mut(events, &params.event_id).ok_or_else(|| {
            err(format!("Plan account event not found: {}", params.event_id))
        })?;
        doc::merge_shallow(event, &params.data)?;
        event
            .get("accountId")
            .and_then(JsonValue::as_str)
            .map(str::to_string)
    };
    let mut snapshot_updated = false;
    if let (Some(linked), false) = (&linked_id, shared.is_empty()) {
        if let Some(sc_account) = plan
            .get_mut("startingConditions")
            .and_then(|sc| sc.get_mut("accounts"))
            .and_then(|a| a.get_mut(linked))
        {
            doc::merge_shallow(sc_account, &shared)?;
            snapshot_updated = true;
        }
    }
    let plans = doc::take_plans(&mut raw)?;
    sync.restore_plans(plans).await.map_err(anyhow_err)?;

    // 3: Current Finances.
    let mut cf_result = JsonValue::Null;
    if let (Some(linked), false) = (&linked_id, shared.is_empty()) {
        let data = sync.get_data().await.map_err(anyhow_err)?;
        match data.current_finances().find(linked) {
            None => cf_result = json!({"skipped": "linked Current Finances account not found"}),
            Some(a) if a.is_archived() => {
                cf_result = json!({"skipped": "linked Current Finances account is archived"})
            }
            Some(_) => {
                drop(data);
                let (values, fields): (JsonMap<_, _>, JsonMap<_, _>) = shared
                    .into_iter()
                    .partition(|(k, _)| VALUE_FIELDS.contains(&k.as_str()));
                if !fields.is_empty() {
                    sync.update_account(linked, JsonValue::Object(fields.clone()), true)
                        .await
                        .map_err(anyhow_err)?;
                }
                if !values.is_empty() {
                    sync.update_account_values(linked, JsonValue::Object(values.clone()))
                        .await
                        .map_err(anyhow_err)?;
                }
                cf_result = json!({"account_id": linked, "values": values, "fields": fields});
            }
        }
    }

    ok_json(&json!({
        "message": format!("Plan account event {} updated", params.event_id),
        "linked_account_id": linked_id,
        "plan_starting_conditions_updated": snapshot_updated,
        "current_finances": cf_result,
    }))
}
