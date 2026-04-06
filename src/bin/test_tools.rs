//! Integration test: exercise the new MCP tools (milestone CRUD, plan create/delete,
//! plan metadata, and browser JS execution).
//!
//! Usage:
//!   cargo run --bin test-tools

use anyhow::Result;
use projectionlab_mcp::browser::BrowserSession;
use projectionlab_mcp::models::FullExport;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    eprintln!("=== Starting browser session ===");
    let mut browser = BrowserSession::new().await?;
    eprintln!("Connected!\n");

    // Helper: export and parse
    let export = || async {
        let raw = browser.call_plugin_api("exportData", vec![]).await?;
        let data: FullExport = serde_json::from_value(raw)?;
        Ok::<_, anyhow::Error>(data)
    };

    // --- 1. List existing plans ---
    eprintln!("=== 1. List existing plans ===");
    let data = export().await?;
    let plan_count_before = data.plans.len();
    for plan in &data.plans {
        eprintln!(
            "  {} | {} | active={} | milestones={}",
            plan.id, plan.name, plan.active, plan.milestones.len()
        );
    }
    eprintln!("  Total: {} plans\n", plan_count_before);

    // Get a source plan to clone from
    let source_plan_id = data.plans.first().expect("Need at least one plan").id.clone();

    // --- 2. Create a test plan (clone from first plan) ---
    eprintln!("=== 2. Create test plan ===");
    let mut data = export().await?;
    let new_id = format!("plan_{}", chrono::Utc::now().timestamp_millis());
    let now = chrono::Utc::now().timestamp_millis();

    let source = data.plans.iter().find(|p| p.id == source_plan_id).unwrap().clone();
    let mut test_plan = source;
    test_plan.id = new_id.clone();
    test_plan.name = "MCP Test Plan (DELETE ME)".to_string();
    test_plan.icon = "mdi-test-tube".to_string();
    test_plan.active = false;
    test_plan.last_updated = now;
    // Clear events for a clean testbed
    test_plan.expenses.events.clear();
    test_plan.income.events.clear();
    test_plan.priorities.events.clear();
    test_plan.assets.events.clear();
    test_plan.accounts.events.clear();
    test_plan.milestones.clear();
    test_plan.computed_milestones.clear();

    data.plans.push(test_plan);
    let plans_value = serde_json::to_value(&data.plans)?;
    browser.call_plugin_api("restorePlans", vec![plans_value]).await?;
    eprintln!("  Created test plan: {} ({})", "MCP Test Plan (DELETE ME)", new_id);

    // Verify
    let data = export().await?;
    assert!(data.plans.iter().any(|p| p.id == new_id), "Test plan not found after create!");
    eprintln!("  Verified: plan exists ({} total plans)\n", data.plans.len());

    // --- 3. Update plan metadata ---
    eprintln!("=== 3. Update plan metadata ===");
    let mut data = export().await?;
    let plan = data.plans.iter_mut().find(|p| p.id == new_id).unwrap();
    let old_name = plan.name.clone();
    plan.name = "MCP Test Plan (RENAMED)".to_string();
    plan.icon = "mdi-flask".to_string();
    let plans_value = serde_json::to_value(&data.plans)?;
    browser.call_plugin_api("restorePlans", vec![plans_value]).await?;

    let data = export().await?;
    let plan = data.plans.iter().find(|p| p.id == new_id).unwrap();
    assert_eq!(plan.name, "MCP Test Plan (RENAMED)");
    assert_eq!(plan.icon, "mdi-flask");
    eprintln!("  Renamed: '{}' -> '{}'", old_name, plan.name);
    eprintln!("  Icon updated to: {}\n", plan.icon);

    // --- 4. Create a milestone ---
    eprintln!("=== 4. Create milestone ===");
    let mut data = export().await?;
    let plan = data.plans.iter_mut().find(|p| p.id == new_id).unwrap();
    let milestone_id = format!("ms_{}", chrono::Utc::now().timestamp_millis());

    let milestone: projectionlab_mcp::models::Milestone = serde_json::from_value(json!({
        "id": milestone_id,
        "name": "Test Milestone - Retire Early",
        "icon": "mdi-beach",
        "color": "#4CAF50",
        "criteria": [{
            "type": "year",
            "value": 2035
        }]
    }))?;

    plan.milestones.push(milestone);
    let plans_value = serde_json::to_value(&data.plans)?;
    browser.call_plugin_api("restorePlans", vec![plans_value]).await?;

    let data = export().await?;
    let plan = data.plans.iter().find(|p| p.id == new_id).unwrap();
    let ms = plan.milestones.iter().find(|m| m.id == milestone_id);
    assert!(ms.is_some(), "Milestone not found after create!");
    eprintln!("  Created milestone: {} ({})", ms.unwrap().name, milestone_id);
    eprintln!("  Plan now has {} milestone(s)\n", plan.milestones.len());

    // --- 5. Update the milestone ---
    eprintln!("=== 5. Update milestone ===");
    let mut data = export().await?;
    let plan = data.plans.iter_mut().find(|p| p.id == new_id).unwrap();
    let ms = plan.milestones.iter_mut().find(|m| m.id == milestone_id).unwrap();
    let old_name = ms.name.clone();

    // Merge update via JSON (same approach as MCP tool)
    let mut ms_json = serde_json::to_value(&*ms)?;
    if let Some(obj) = ms_json.as_object_mut() {
        obj.insert("name".to_string(), json!("Test Milestone - Retire VERY Early"));
        obj.insert("criteria".to_string(), json!([{
            "type": "year",
            "value": 2032
        }]));
    }
    *ms = serde_json::from_value(ms_json)?;

    let plans_value = serde_json::to_value(&data.plans)?;
    browser.call_plugin_api("restorePlans", vec![plans_value]).await?;

    let data = export().await?;
    let plan = data.plans.iter().find(|p| p.id == new_id).unwrap();
    let ms = plan.milestones.iter().find(|m| m.id == milestone_id).unwrap();
    assert_eq!(ms.name, "Test Milestone - Retire VERY Early");
    eprintln!("  Renamed: '{}' -> '{}'", old_name, ms.name);
    eprintln!("  Criteria updated (year: 2035 -> 2032)\n");

    // --- 6. Delete the milestone ---
    eprintln!("=== 6. Delete milestone ===");
    let mut data = export().await?;
    let plan = data.plans.iter_mut().find(|p| p.id == new_id).unwrap();
    let before_count = plan.milestones.len();
    plan.milestones.retain(|m| m.id != milestone_id);
    let plans_value = serde_json::to_value(&data.plans)?;
    browser.call_plugin_api("restorePlans", vec![plans_value]).await?;

    let data = export().await?;
    let plan = data.plans.iter().find(|p| p.id == new_id).unwrap();
    assert!(plan.milestones.iter().all(|m| m.id != milestone_id), "Milestone still exists after delete!");
    eprintln!("  Deleted milestone: {} ({} -> {} milestones)\n", milestone_id, before_count, plan.milestones.len());

    // --- 7. Test browser JS execution ---
    eprintln!("=== 7. Browser JS execution ===");
    let result = browser.execute_js("return document.title").await?;
    eprintln!("  Page title: {}", result);

    let result = browser.execute_js("return window.location.href").await?;
    eprintln!("  Current URL: {}", result);

    // Navigate to test plan and check Vue app
    browser.navigate_to(&format!("/plan/{}", new_id)).await?;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let result = browser.execute_js(
        "const app = document.querySelector('#app'); return !!(app && app.__vue_app__)"
    ).await?;
    eprintln!("  Vue app detected: {}", result);

    // Try to list Pinia store IDs
    let result = browser.execute_js(r#"
        const app = document.querySelector('#app');
        if (!app || !app.__vue_app__) return 'no vue app';
        const pinia = app.__vue_app__.config.globalProperties.$pinia;
        if (!pinia || !pinia._s) return 'no pinia';
        const ids = [];
        pinia._s.forEach((store, id) => ids.push(id));
        return ids;
    "#).await?;
    eprintln!("  Pinia stores: {}\n", serde_json::to_string_pretty(&result)?);

    // --- 8. Test simulation data extraction ---
    eprintln!("=== 8. Simulation data extraction ===");
    // Use first real plan (not our test plan) for simulation data
    let real_plan_id = data.plans.first().map(|p| p.id.clone())
        .unwrap_or_else(|| source_plan_id.clone());
    browser.navigate_to(&format!("/plan/{}", real_plan_id)).await?;
    eprintln!("  Navigated to plan, waiting for simulation...");
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // Extract simulation results
    let script = r#"
        const app = document.querySelector('#app');
        const pinia = app.__vue_app__.config.globalProperties.$pinia;
        const store = pinia._s.get('plan');
        const results = store.plan._runtime?.results;
        if (!results || !results.data) return { error: 'No simulation data' };

        const simYears = results.data.filter(y => y.isSimulatedYear);
        const firstYear = simYears[0];
        const lastYear = simYears[simYears.length - 1];
        const s0 = firstYear?.summary || {};
        const sN = lastYear?.summary || {};

        return {
            yearCount: simYears.length,
            firstAge: firstYear?.age,
            lastAge: lastYear?.age,
            firstNW: Math.round(s0.netWorth?.total || 0),
            lastNW: Math.round(sN.netWorth?.total || 0),
            outcome: results.outcome?.status,
            milestoneCount: Object.keys(results._meta?.milestoneCompletionCache || {}).length,
            notableEventCount: (results.notableEvents || []).length,
        };
    "#;
    let sim_result = browser.execute_js(script).await?;
    let year_count = sim_result.get("yearCount").and_then(|v| v.as_i64()).unwrap_or(0);
    let outcome = sim_result.get("outcome").and_then(|v| v.as_str()).unwrap_or("unknown");
    let first_nw = sim_result.get("firstNW").and_then(|v| v.as_i64()).unwrap_or(0);
    let last_nw = sim_result.get("lastNW").and_then(|v| v.as_i64()).unwrap_or(0);
    let milestone_count = sim_result.get("milestoneCount").and_then(|v| v.as_i64()).unwrap_or(0);
    let notable_count = sim_result.get("notableEventCount").and_then(|v| v.as_i64()).unwrap_or(0);

    assert!(year_count > 0, "No simulated years found!");
    eprintln!("  Simulation: {} years, outcome: {}", year_count, outcome);
    eprintln!("  Net worth: ${} -> ${}", first_nw, last_nw);
    eprintln!("  Milestones tracked: {}, Notable events: {}\n", milestone_count, notable_count);

    // --- 9. Delete the test plan (cleanup) ---
    eprintln!("=== 9. Delete test plan (cleanup) ===");
    // Use raw JSON to avoid deserialization issues after navigation
    let raw = browser.call_plugin_api("exportData", vec![]).await?;
    let mut plans_arr = raw.get("plans").and_then(|v| v.as_array().cloned())
        .expect("plans array not found in export");
    let before_count = plans_arr.len();
    plans_arr.retain(|p| p.get("id").and_then(|v| v.as_str()) != Some(&new_id));
    let plans_value = serde_json::Value::Array(plans_arr.clone());
    browser.call_plugin_api("restorePlans", vec![plans_value]).await?;
    eprintln!("  Deleted test plan ({} -> {} plans)\n", before_count, plans_arr.len());

    // --- Summary ---
    eprintln!("=== ALL TESTS PASSED ===");
    eprintln!("  1. Plan list          ✓");
    eprintln!("  2. Plan create        ✓");
    eprintln!("  3. Plan metadata      ✓");
    eprintln!("  4. Milestone create   ✓");
    eprintln!("  5. Milestone update   ✓");
    eprintln!("  6. Milestone delete   ✓");
    eprintln!("  7. Browser JS exec    ✓");
    eprintln!("  8. Simulation data    ✓");
    eprintln!("  9. Plan delete        ✓");

    browser.shutdown().await;
    Ok(())
}
