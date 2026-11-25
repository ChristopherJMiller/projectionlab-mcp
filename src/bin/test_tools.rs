//! Live integration test for the MCP tool handlers against ProjectionLab.
//!
//! Usage:
//!   cargo run --bin test-tools
//!
//! Safety: every create/update/delete happens inside a temporary plan named
//! `__mcp_test_<timestamp>`, which is deleted at the end (stale ones from
//! earlier aborted runs are cleaned up first). Current Finances is only
//! touched with a *no-op* value write (an account's balance set to its current
//! value), which exercises the write-through path without changing data or
//! adding history points.

use anyhow::{bail, ensure, Context, Result};
use projectionlab_mcp::browser::BrowserSession;
use projectionlab_mcp::sync::SyncManager;
use projectionlab_mcp::tools::params::*;
use projectionlab_mcp::tools::plan_events::Section;
use projectionlab_mcp::tools::{accounts, history, plan_accounts, plan_events, plans, simulation};
use rmcp::model::CallToolResult;
use serde_json::{json, Map, Value};
use std::sync::Arc;

const TEST_PLAN_PREFIX: &str = "__mcp_test_";

/// Extract the JSON (or text) payload from a tool result.
fn payload(result: Result<CallToolResult, rmcp::ErrorData>) -> Result<Value> {
    let result = result.map_err(|e| anyhow::anyhow!("tool error: {}", e.message))?;
    let text = result
        .content
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.clone())
        .context("tool returned no text content")?;
    Ok(serde_json::from_str(&text).unwrap_or(Value::String(text)))
}

fn obj(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap_or_default()
}

macro_rules! step {
    ($name:expr) => {
        eprintln!("\n=== {} ===", $name);
    };
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    eprintln!("Starting browser session...");
    let session = BrowserSession::new().await?;
    let browser = Arc::new(tokio::sync::Mutex::new(Some(session)));
    let sync = SyncManager::new(browser.clone());

    let outcome = run(&sync).await;

    // Always try to remove test plans, even if a step failed.
    if let Err(e) = cleanup_test_plans(&sync).await {
        eprintln!("!! cleanup failed: {:#}", e);
    }
    if let Some(mut s) = browser.lock().await.take() {
        s.shutdown().await;
    }

    match outcome {
        Ok(()) => {
            eprintln!("\n=== ALL TESTS PASSED ===");
            Ok(())
        }
        Err(e) => {
            eprintln!("\n!! TEST FAILED: {:#}", e);
            Err(e)
        }
    }
}

async fn cleanup_test_plans(sync: &SyncManager) -> Result<()> {
    sync.invalidate().await;
    let data = sync.get_data().await?;
    let stale: Vec<String> = data
        .plans()
        .iter()
        .filter(|p| p.name.starts_with(TEST_PLAN_PREFIX))
        .map(|p| p.id.clone())
        .collect();
    drop(data);
    for id in stale {
        eprintln!("  removing test plan {}", id);
        payload(plans::delete(sync, PlansDeleteParams { plan_id: id, confirm: true }).await)?;
    }
    Ok(())
}

async fn run(sync: &SyncManager) -> Result<()> {
    step!("0. Export parses");
    let data = sync.refresh().await?;
    eprintln!(
        "  plans={} cf_accounts={} history_accounts={}",
        data.plans().len(),
        data.current_finances().accounts.len(),
        data.documents.account_history.len()
    );
    let original_active = data.active_plan_id().map(str::to_string);
    let source_plan_id = original_active
        .clone()
        .or_else(|| data.plans().first().map(|p| p.id.clone()))
        .context("no plans")?;
    drop(data);

    step!("0b. Clean up stale test plans");
    cleanup_test_plans(sync).await?;

    step!("1. Read-only tools");
    let list = payload(plans::list(sync).await)?;
    eprintln!("  plans_list: {} plans", list["total_plans"]);
    let accts = payload(
        accounts::list(sync, AccountsListParams { kind: None, account_type: None, owner: None, include_archived: false }).await,
    )?;
    eprintln!("  accounts_list: {} active items, assets={} debt={}", accts["total_count"], accts["total_assets"], accts["total_debt"]);
    let first_account = accts["accounts"][0]["id"].as_str().context("no accounts")?.to_string();
    payload(accounts::get(sync, AccountsGetParams { account_id: first_account.clone() }).await)?;
    let get_accounts = sync.call_api("getAccounts", vec![]).await?;
    ensure!(get_accounts.is_array(), "getAccounts should return an array");
    sync.call_api("validateApiKey", vec![]).await.context("validateApiKey")?;
    let hist = payload(
        history::account_history(sync, AccountHistoryParams { account_id: Some(first_account), start_date: None, end_date: None, include_archived: false }).await,
    )?;
    eprintln!("  account_history: {} points", hist["accounts"][0]["points"].as_array().map_or(0, |p| p.len()));
    let nw = payload(history::get_history(sync, ProgressGetHistoryParams { start_date: None, end_date: None }).await)?;
    eprintln!("  progress_get_history: {} points, latest net worth {}", nw["total_data_points"], nw["latest"]["netWorth"]);
    ensure!(nw["total_data_points"].as_u64().unwrap_or(0) > 0, "expected net worth history");
    let snap = payload(history::snapshot(sync, ProgressAddDataPointParams::default()).await)?;
    eprintln!("  snapshot_net_worth: current finances net worth {}", snap["current_finances_net_worth"]);

    step!("2. Create test plan");
    let test_name = format!("{}{}", TEST_PLAN_PREFIX, chrono::Utc::now().timestamp_millis());
    let created = payload(
        plans::create(sync, PlansCreateParams { name: test_name.clone(), icon: "mdi-test-tube".into(), clone_from: Some(source_plan_id.clone()) }).await,
    )?;
    let plan_id = created["plan_id"].as_str().context("plan_id")?.to_string();
    let data = sync.get_data().await?;
    let plan = data.find_plan(&plan_id).context("test plan missing after create")?;
    ensure!(plan.name == test_name, "name mismatch");
    let source = data.find_plan(&source_plan_id).unwrap();
    ensure!(plan.expenses.events.len() == source.expenses.events.len(), "clone should copy expenses");
    eprintln!("  created {} (cloned from {})", plan_id, source_plan_id);
    drop(data);

    step!("3. Plan metadata + variables");
    payload(plans::update_metadata(sync, PlansUpdateMetadataParams { plan_id: plan_id.clone(), updates: obj(json!({"name": format!("{} renamed", test_name)})) }).await)?;
    let vars = payload(plans::get_variables(sync, PlanGetParams { plan_id: plan_id.clone() }).await)?;
    let inflation = vars["variables"]["inflation"].clone();
    payload(plans::update_variables(sync, PlansUpdateVariablesParams { plan_id: plan_id.clone(), updates: obj(json!({"inflation": 9.87})) }).await)?;
    let vars = payload(plans::get_variables(sync, PlanGetParams { plan_id: plan_id.clone() }).await)?;
    ensure!(vars["variables"]["inflation"] == json!(9.87), "variables update not applied");
    payload(plans::update_variables(sync, PlansUpdateVariablesParams { plan_id: plan_id.clone(), updates: obj(json!({"inflation": inflation})) }).await)?;
    eprintln!("  metadata + variables round-trip OK");

    step!("4. Milestones CRUD");
    let ms = payload(
        plans::create_milestone(sync, MilestoneCreateParams {
            plan_id: plan_id.clone(),
            data: obj(json!({"name": "Test milestone", "icon": "mdi-flag", "color": "blue", "criteria": [{"type": "year", "value": 2040}]})),
        }).await,
    )?;
    let ms_id = ms["id"].as_str().context("milestone id")?.to_string();
    payload(plans::update_milestone(sync, MilestoneUpdateParams { plan_id: plan_id.clone(), milestone_id: ms_id.clone(), data: obj(json!({"name": "Test milestone 2"})) }).await)?;
    payload(plans::delete_milestone(sync, MilestoneDeleteParams { plan_id: plan_id.clone(), milestone_id: ms_id }).await)?;
    let builtin = plans::delete_milestone(sync, MilestoneDeleteParams { plan_id: plan_id.clone(), milestone_id: "retirement".into() }).await;
    ensure!(builtin.is_err(), "deleting a built-in milestone should fail");
    eprintln!("  create/update/delete OK; built-in delete refused");

    step!("5. Plan events CRUD (all sections)");
    for section in [Section::Expenses, Section::Income, Section::Priorities, Section::Assets] {
        let listed = payload(plan_events::list(sync, section, PlanEventsListParams { plan_id: plan_id.clone() }).await)?;
        let before = listed["total"].as_u64().unwrap_or(0);
        // Use an existing event of the section as a template, preferring the
        // test plan's own events. Skip auto-generated events and debt
        // repayments (ProjectionLab keeps one 'debt' expense per debt).
        let data = sync.get_data().await?;
        let template = data
            .plans()
            .iter()
            .filter(|p| p.id == plan_id)
            .chain(data.plans().iter().filter(|p| p.id != plan_id))
            .flat_map(|p| match section {
                Section::Expenses => p.expenses.events.iter(),
                Section::Income => p.income.events.iter(),
                Section::Priorities => p.priorities.events.iter(),
                Section::Assets => p.assets.events.iter(),
            })
            .find(|e| e.get("generated").is_none() && e.event_type != "debt" && e.get("debtId").is_none())
            .cloned();
        drop(data);
        let Some(template) = template else {
            eprintln!("  {}: no template event available, skipped", section.key());
            continue;
        };
        let mut event = obj(serde_json::to_value(&template)?);
        event.remove("id");
        event.insert("name".into(), json!("MCP test event"));
        let created = payload(plan_events::create(sync, section, PlanEventCreateParams { plan_id: plan_id.clone(), data: event }).await)?;
        let event_id = created["id"].as_str().context("event id")?.to_string();
        let updated = payload(
            plan_events::update(sync, section, PlanEventUpdateParams { plan_id: plan_id.clone(), event_id: event_id.clone(), data: obj(json!({"name": "MCP test event 2"})) }).await,
        )?;
        ensure!(updated["event"]["name"] == "MCP test event 2", "update not applied");
        ensure!(updated["event"]["kind"] == json!(section.kind()), "kind missing");
        payload(plan_events::delete(sync, section, PlanEventDeleteParams { plan_id: plan_id.clone(), event_id }).await)?;
        let listed = payload(plan_events::list(sync, section, PlanEventsListParams { plan_id: plan_id.clone() }).await)?;
        ensure!(listed["total"].as_u64().unwrap_or(0) == before, "event count changed after delete");
        eprintln!("  {}: create/update/delete OK", section.key());
    }

    step!("6. Plan account write-through (no-op value)");
    let pa = payload(plan_accounts::list(sync, PlanEventsListParams { plan_id: plan_id.clone() }).await)?;
    let linked = pa["accounts"]
        .as_array()
        .and_then(|a| a.iter().find(|e| e["currentFinancesBalance"].is_number() && e["currentFinancesArchived"] == json!(false)))
        .cloned();
    if let Some(linked) = linked {
        let event_id = linked["id"].as_str().unwrap().to_string();
        let balance = linked["currentFinancesBalance"].clone();
        let result = payload(
            plan_accounts::update(sync, PlanEventUpdateParams { plan_id: plan_id.clone(), event_id, data: obj(json!({"balance": balance})) }).await,
        )?;
        ensure!(result["current_finances"]["account_id"].is_string(), "expected Current Finances write-through: {}", result);
        let after = sync.get_data().await?;
        let cf_balance = after.current_finances().find(linked["accountId"].as_str().unwrap()).and_then(|a| a.balance);
        ensure!(cf_balance == balance.as_f64(), "Current Finances balance changed unexpectedly");
        eprintln!("  write-through OK ({} = {})", linked["name"], balance);
    } else {
        eprintln!("  no linked plan account; skipped");
    }

    step!("7. Simulation extraction");
    let sim = sync.run_js_async(&simulation::for_plan(&plan_id, simulation::SIMULATION_SUMMARY)).await?;
    let years = sim["yearCount"].as_u64().unwrap_or(0);
    ensure!(years > 0, "no simulated years: {}", sim);
    eprintln!("  {} years, outcome={}", years, sim["outcome"]["status"]);
    let start_age = sim["years"][0]["age"].as_i64().context("age")?;
    let snapshot = sync.run_js_async(&simulation::for_plan(&plan_id, &simulation::year_snapshot_extractor(start_age + 1))).await?;
    ensure!(snapshot["summary"].is_object(), "year snapshot missing summary");
    let range = sync.run_js_async(&simulation::for_plan(&plan_id, &simulation::year_range_extractor(start_age, start_age + 5))).await?;
    ensure!(range["years"].as_array().map_or(0, |y| y.len()) > 1, "year range too short");
    let mc = sync.run_js_async(simulation::MONTECARLO_RESULTS_JS).await?;
    eprintln!("  year snapshot + range OK; monte carlo hasResults={}", mc["hasResults"]);
    let selected = sync
        .run_js_async("const cb = arguments[arguments.length-1]; cb(document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s.get('plan').planId);")
        .await?;
    if let Some(orig) = &original_active {
        ensure!(selected.as_str() == Some(orig.as_str()), "plan selection not restored: {} != {}", selected, orig);
    }
    eprintln!("  original plan selection restored");

    step!("8. Delete test plan");
    payload(plans::delete(sync, PlansDeleteParams { plan_id: plan_id.clone(), confirm: true }).await)?;
    let data = sync.get_data().await?;
    if data.find_plan(&plan_id).is_some() {
        bail!("test plan still present after delete");
    }
    eprintln!("  deleted");
    Ok(())
}
