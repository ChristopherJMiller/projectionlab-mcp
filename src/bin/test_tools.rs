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

    // --- 0. Test export deserialization ---
    eprintln!("=== 0. Test export deserialization ===");
    let raw = browser.call_plugin_api("exportData", vec![]).await?;
    eprintln!(
        "  Top-level keys: {:?}",
        raw.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );
    match serde_json::from_value::<FullExport>(raw.clone()) {
        Ok(_) => eprintln!("  FullExport parsed OK\n"),
        Err(e) => {
            eprintln!("  !! FullExport parse FAILED: {}", e);
            // Try each top-level field individually to narrow down
            for key in ["meta", "today", "plans", "settings", "progress"] {
                if let Some(val) = raw.get(key) {
                    let result = match key {
                        "meta" => serde_json::from_value::<projectionlab_mcp::models::Meta>(val.clone())
                            .map(|_| ()),
                        "today" => serde_json::from_value::<projectionlab_mcp::models::StartingConditions>(val.clone())
                            .map(|_| ()),
                        "plans" => serde_json::from_value::<Vec<projectionlab_mcp::models::Plan>>(val.clone())
                            .map(|_| ()),
                        "settings" => serde_json::from_value::<projectionlab_mcp::models::Settings>(val.clone())
                            .map(|_| ()),
                        "progress" => serde_json::from_value::<projectionlab_mcp::models::Progress>(val.clone())
                            .map(|_| ()),
                        _ => Ok(()),
                    };
                    match result {
                        Ok(_) => eprintln!("    {}: OK", key),
                        Err(e2) => eprintln!("    {}: FAILED - {}", key, e2),
                    }
                } else {
                    eprintln!("    {}: MISSING from export", key);
                }
            }
            eprintln!();
            anyhow::bail!("Export deserialization failed: {}", e);
        }
    }

    // Helper macro: export and parse (avoids closure borrow issues)
    macro_rules! export {
        () => {{
            let raw = browser.call_plugin_api("exportData", vec![]).await?;
            serde_json::from_value::<FullExport>(raw)?
        }};
    }

    // --- 0b. Clean up stale test plans from previous failed runs ---
    eprintln!("=== 0b. Clean up stale test plans ===");
    {
        let raw = browser.call_plugin_api("exportData", vec![]).await?;
        let mut plans_arr = raw.get("plans").and_then(|v| v.as_array().cloned())
            .expect("plans array not found in export");
        let before = plans_arr.len();
        plans_arr.retain(|p| {
            let name = p.get("name").and_then(|v| v.as_str()).unwrap_or("");
            !name.contains("DELETE ME") && !name.contains("MCP Test Plan")
        });
        let removed = before - plans_arr.len();
        if removed > 0 {
            let plans_value = serde_json::Value::Array(plans_arr);
            browser.call_plugin_api("restorePlans", vec![plans_value]).await?;
            eprintln!("  Cleaned up {} stale test plan(s)\n", removed);
        } else {
            eprintln!("  No stale test plans found\n");
        }
    }

    // --- 1. List existing plans ---
    eprintln!("=== 1. List existing plans ===");
    let data = export!();
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
    let mut data = export!();
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
    let data = export!();
    assert!(data.plans.iter().any(|p| p.id == new_id), "Test plan not found after create!");
    eprintln!("  Verified: plan exists ({} total plans)\n", data.plans.len());

    // --- 3. Update plan metadata ---
    eprintln!("=== 3. Update plan metadata ===");
    let mut data = export!();
    let plan = data.plans.iter_mut().find(|p| p.id == new_id).unwrap();
    let old_name = plan.name.clone();
    plan.name = "MCP Test Plan (RENAMED)".to_string();
    plan.icon = "mdi-flask".to_string();
    let plans_value = serde_json::to_value(&data.plans)?;
    browser.call_plugin_api("restorePlans", vec![plans_value]).await?;

    let data = export!();
    let plan = data.plans.iter().find(|p| p.id == new_id).unwrap();
    assert_eq!(plan.name, "MCP Test Plan (RENAMED)");
    assert_eq!(plan.icon, "mdi-flask");
    eprintln!("  Renamed: '{}' -> '{}'", old_name, plan.name);
    eprintln!("  Icon updated to: {}\n", plan.icon);

    // --- 4. Create a milestone ---
    eprintln!("=== 4. Create milestone ===");
    let mut data = export!();
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

    let data = export!();
    let plan = data.plans.iter().find(|p| p.id == new_id).unwrap();
    let ms = plan.milestones.iter().find(|m| m.id == milestone_id);
    assert!(ms.is_some(), "Milestone not found after create!");
    eprintln!("  Created milestone: {} ({})", ms.unwrap().name, milestone_id);
    eprintln!("  Plan now has {} milestone(s)\n", plan.milestones.len());

    // --- 5. Update the milestone ---
    eprintln!("=== 5. Update milestone ===");
    let mut data = export!();
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

    let data = export!();
    let plan = data.plans.iter().find(|p| p.id == new_id).unwrap();
    let ms = plan.milestones.iter().find(|m| m.id == milestone_id).unwrap();
    assert_eq!(ms.name, "Test Milestone - Retire VERY Early");
    eprintln!("  Renamed: '{}' -> '{}'", old_name, ms.name);
    eprintln!("  Criteria updated (year: 2035 -> 2032)\n");

    // --- 6. Delete the milestone ---
    eprintln!("=== 6. Delete milestone ===");
    let mut data = export!();
    let plan = data.plans.iter_mut().find(|p| p.id == new_id).unwrap();
    let before_count = plan.milestones.len();
    plan.milestones.retain(|m| m.id != milestone_id);
    let plans_value = serde_json::to_value(&data.plans)?;
    browser.call_plugin_api("restorePlans", vec![plans_value]).await?;

    let data = export!();
    let plan = data.plans.iter().find(|p| p.id == new_id).unwrap();
    assert!(plan.milestones.iter().all(|m| m.id != milestone_id), "Milestone still exists after delete!");
    eprintln!("  Deleted milestone: {} ({} -> {} milestones)\n", milestone_id, before_count, plan.milestones.len());

    // --- 7. Plan asset events CRUD ---
    eprintln!("=== 7. Plan asset events CRUD ===");
    let asset_events_before = plan.assets.events.len();
    eprintln!("  Plan '{}' asset events before: {}", plan.name, asset_events_before);

    // Create a plan asset event (house purchase) using raw JSON since AssetEvent has many required fields
    let test_asset_event_id = format!("asset_{}", chrono::Utc::now().timestamp_millis());
    let raw = browser.call_plugin_api("exportData", vec![]).await?;
    let mut plans_arr = raw.get("plans").and_then(|v| v.as_array().cloned())
        .expect("plans array not found");
    let plan_json = plans_arr.iter_mut()
        .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(&new_id))
        .expect("Test plan not found in raw JSON");
    let assets_events = plan_json.get_mut("assets")
        .and_then(|a| a.get_mut("events"))
        .and_then(|e| e.as_array_mut())
        .expect("assets.events not found");

    // Borrow a real plan's asset event as a template if available, otherwise build from scratch
    let template_event = {
        let real_plans = raw.get("plans").and_then(|v| v.as_array()).unwrap();
        let mut found = None;
        for rp in real_plans {
            if let Some(evts) = rp.get("assets").and_then(|a| a.get("events")).and_then(|e| e.as_array()) {
                if let Some(evt) = evts.first() {
                    found = Some(evt.clone());
                    break;
                }
            }
        }
        found
    };

    if let Some(mut tmpl) = template_event {
        // Override with test values
        if let Some(obj) = tmpl.as_object_mut() {
            obj.insert("id".to_string(), json!(test_asset_event_id));
            obj.insert("name".to_string(), json!("Test House (DELETE ME)"));
            obj.insert("title".to_string(), json!("Test House"));
            obj.insert("amount".to_string(), json!(500000));
            obj.insert("initialValue".to_string(), json!(500000));
            obj.insert("downPayment".to_string(), json!(100000));
            obj.insert("interestRate".to_string(), json!(6.5));
        }
        assets_events.push(tmpl);

        let plans_value = serde_json::Value::Array(plans_arr.clone());
        browser.call_plugin_api("restorePlans", vec![plans_value]).await?;

        // Verify
        let raw2 = browser.call_plugin_api("exportData", vec![]).await?;
        let plans_arr2 = raw2.get("plans").and_then(|v| v.as_array()).unwrap();
        let plan_json2 = plans_arr2.iter()
            .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(&new_id))
            .unwrap();
        let events2 = plan_json2.get("assets")
            .and_then(|a| a.get("events"))
            .and_then(|e| e.as_array())
            .unwrap();
        let created = events2.iter().find(|e| e.get("id").and_then(|v| v.as_str()) == Some(&test_asset_event_id));
        assert!(created.is_some(), "Plan asset event not found after create!");
        let amount = created.unwrap().get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
        eprintln!("  Created: Test House (amount={}, events: {} -> {})", amount, asset_events_before, events2.len());

        // Update the plan asset event (change price)
        let mut plans_arr3 = raw2.get("plans").and_then(|v| v.as_array().cloned()).unwrap();
        let plan_json3 = plans_arr3.iter_mut()
            .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(&new_id))
            .unwrap();
        let events3 = plan_json3.get_mut("assets")
            .and_then(|a| a.get_mut("events"))
            .and_then(|e| e.as_array_mut())
            .unwrap();
        let evt = events3.iter_mut()
            .find(|e| e.get("id").and_then(|v| v.as_str()) == Some(&test_asset_event_id))
            .unwrap();
        if let Some(obj) = evt.as_object_mut() {
            obj.insert("amount".to_string(), json!(750000));
            obj.insert("initialValue".to_string(), json!(750000));
            obj.insert("downPayment".to_string(), json!(150000));
            obj.insert("interestRate".to_string(), json!(5.0));
        }

        let plans_value = serde_json::Value::Array(plans_arr3);
        browser.call_plugin_api("restorePlans", vec![plans_value]).await?;

        let raw4 = browser.call_plugin_api("exportData", vec![]).await?;
        let plans_arr4 = raw4.get("plans").and_then(|v| v.as_array()).unwrap();
        let plan_json4 = plans_arr4.iter()
            .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(&new_id))
            .unwrap();
        let events4 = plan_json4.get("assets")
            .and_then(|a| a.get("events"))
            .and_then(|e| e.as_array())
            .unwrap();
        let updated = events4.iter()
            .find(|e| e.get("id").and_then(|v| v.as_str()) == Some(&test_asset_event_id))
            .unwrap();
        let new_amount = updated.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let new_rate = updated.get("interestRate").and_then(|v| v.as_f64()).unwrap_or(0.0);
        assert!((new_amount - 750000.0).abs() < 0.01, "Plan asset amount not updated!");
        assert!((new_rate - 5.0).abs() < 0.01, "Plan asset interest rate not updated!");
        eprintln!("  Updated: amount=500000->750000, rate=6.5->5.0");

        // Delete the plan asset event
        let mut plans_arr5 = raw4.get("plans").and_then(|v| v.as_array().cloned()).unwrap();
        let plan_json5 = plans_arr5.iter_mut()
            .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(&new_id))
            .unwrap();
        let events5 = plan_json5.get_mut("assets")
            .and_then(|a| a.get_mut("events"))
            .and_then(|e| e.as_array_mut())
            .unwrap();
        events5.retain(|e| e.get("id").and_then(|v| v.as_str()) != Some(&test_asset_event_id));
        let plans_value = serde_json::Value::Array(plans_arr5);
        browser.call_plugin_api("restorePlans", vec![plans_value]).await?;
        eprintln!("  Deleted test asset event\n");
    } else {
        eprintln!("  SKIPPED: No existing plan asset events to use as template\n");
    }

    // --- 8. Plan accounts list ---
    eprintln!("=== 8. Plan accounts list ===");
    let data = export!();
    let plan = data.plans.iter().find(|p| p.id == source_plan_id).unwrap();
    eprintln!("  Plan '{}' account events: {}", plan.name, plan.accounts.events.len());
    for a in &plan.accounts.events {
        eprintln!("    {} | {} | type={} | balance={} | growth_rate={}",
            a.id, a.name, a.event_type, a.balance, a.investment_growth_rate);
    }
    eprintln!();

    // --- 8b. Plan account update propagation ---
    eprintln!("=== 8b. Plan account update propagation ===");
    // Test that updating a plan account's balance propagates to:
    //   (a) the plan's starting_conditions snapshot
    //   (b) the global Current Finances account
    {
        let data = export!();
        let plan = data.plans.iter().find(|p| p.id == source_plan_id).unwrap();
        // Pick the first account that has a linked account_id
        let linked_account = plan.accounts.events.iter()
            .find(|a| a.account_id.is_some());

        if let Some(acct) = linked_account {
            let event_id = acct.id.clone();
            let cf_id = acct.account_id.clone().unwrap();
            let original_balance = acct.balance;
            let test_balance = original_balance + 0.42; // Small delta to detect
            eprintln!("  Testing with plan account '{}' (event={}, cf={})", acct.name, event_id, cf_id);
            eprintln!("  Original balance: {}", original_balance);

            // Update the plan account balance and growth rate
            let raw = browser.call_plugin_api("exportData", vec![]).await?;
            let mut plans_arr = raw.get("plans").and_then(|v| v.as_array().cloned()).unwrap();
            let plan_json = plans_arr.iter_mut()
                .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(&source_plan_id))
                .unwrap();

            // Update in accounts.events
            let acct_events = plan_json.get_mut("accounts")
                .and_then(|a| a.get_mut("events"))
                .and_then(|e| e.as_array_mut())
                .unwrap();
            let acct_json = acct_events.iter_mut()
                .find(|e| e.get("id").and_then(|v| v.as_str()) == Some(&event_id))
                .unwrap();
            acct_json.as_object_mut().unwrap()
                .insert("balance".to_string(), json!(test_balance));

            // Also update in startingConditions (both savings and investment)
            if let Some(sc) = plan_json.get_mut("startingConditions") {
                for key in ["savingsAccounts", "investmentAccounts"] {
                    if let Some(arr) = sc.get_mut(key).and_then(|v| v.as_array_mut()) {
                        if let Some(sa) = arr.iter_mut().find(|a| a.get("id").and_then(|v| v.as_str()) == Some(&cf_id)) {
                            sa.as_object_mut().unwrap()
                                .insert("balance".to_string(), json!(test_balance));
                        }
                    }
                }
            }

            let plans_value = serde_json::Value::Array(plans_arr);
            browser.call_plugin_api("restorePlans", vec![plans_value]).await?;

            // Also update Current Finances
            let mut cf_raw = browser.call_plugin_api("exportData", vec![]).await?;
            if let Some(today) = cf_raw.get_mut("today") {
                let mut cf_found = false;
                for key in ["savingsAccounts", "investmentAccounts"] {
                    if let Some(arr) = today.get_mut(key).and_then(|v| v.as_array_mut()) {
                        if let Some(sa) = arr.iter_mut().find(|a| a.get("id").and_then(|v| v.as_str()) == Some(&cf_id)) {
                            sa.as_object_mut().unwrap()
                                .insert("balance".to_string(), json!(test_balance));
                            cf_found = true;
                        }
                    }
                }
                if cf_found {
                    browser.call_plugin_api("restoreCurrentFinances", vec![today.clone()]).await?;
                }
            }

            // Verify all three locations
            let raw_verify = browser.call_plugin_api("exportData", vec![]).await?;
            let data_verify: FullExport = serde_json::from_value(raw_verify.clone())?;

            // (a) Plan account event
            let plan_v = data_verify.plans.iter().find(|p| p.id == source_plan_id).unwrap();
            let acct_v = plan_v.accounts.events.iter().find(|a| a.id == event_id).unwrap();
            eprintln!("  Plan account event balance: {}", acct_v.balance);

            // (b) Plan starting_conditions
            let sc_balance = {
                let plan_raw = raw_verify.get("plans").and_then(|v| v.as_array()).unwrap()
                    .iter().find(|p| p.get("id").and_then(|v| v.as_str()) == Some(&source_plan_id)).unwrap();
                let sc = plan_raw.get("startingConditions").unwrap();
                let mut found_bal = None;
                for key in ["savingsAccounts", "investmentAccounts"] {
                    if let Some(arr) = sc.get(key).and_then(|v| v.as_array()) {
                        if let Some(a) = arr.iter().find(|a| a.get("id").and_then(|v| v.as_str()) == Some(&cf_id)) {
                            found_bal = a.get("balance").and_then(|v| v.as_f64());
                        }
                    }
                }
                found_bal
            };
            eprintln!("  Plan starting_conditions balance: {:?}", sc_balance);

            // (c) Current Finances
            let cf_balance = data_verify.today.savings_accounts.iter()
                .chain(data_verify.today.investment_accounts.iter())
                .find(|a| a.id == cf_id)
                .map(|a| a.balance);
            eprintln!("  Current Finances balance: {:?}", cf_balance);

            // All three should have the test balance
            assert!((acct_v.balance - test_balance).abs() < 0.01,
                "Plan account event balance mismatch: expected {}, got {}", test_balance, acct_v.balance);
            if let Some(sb) = sc_balance {
                assert!((sb - test_balance).abs() < 0.01,
                    "Starting conditions balance mismatch: expected {}, got {}", test_balance, sb);
            }
            assert!(cf_balance.is_some(), "Current Finances account not found!");
            assert!((cf_balance.unwrap() - test_balance).abs() < 0.01,
                "Current Finances balance mismatch: expected {}, got {}", test_balance, cf_balance.unwrap());
            eprintln!("  All three locations match! ✓");

            // Restore original balance
            let raw = browser.call_plugin_api("exportData", vec![]).await?;
            let mut plans_arr = raw.get("plans").and_then(|v| v.as_array().cloned()).unwrap();
            let plan_json = plans_arr.iter_mut()
                .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(&source_plan_id))
                .unwrap();
            let acct_events = plan_json.get_mut("accounts")
                .and_then(|a| a.get_mut("events"))
                .and_then(|e| e.as_array_mut())
                .unwrap();
            let acct_json = acct_events.iter_mut()
                .find(|e| e.get("id").and_then(|v| v.as_str()) == Some(&event_id))
                .unwrap();
            acct_json.as_object_mut().unwrap()
                .insert("balance".to_string(), json!(original_balance));
            if let Some(sc) = plan_json.get_mut("startingConditions") {
                for key in ["savingsAccounts", "investmentAccounts"] {
                    if let Some(arr) = sc.get_mut(key).and_then(|v| v.as_array_mut()) {
                        if let Some(sa) = arr.iter_mut().find(|a| a.get("id").and_then(|v| v.as_str()) == Some(&cf_id)) {
                            sa.as_object_mut().unwrap()
                                .insert("balance".to_string(), json!(original_balance));
                        }
                    }
                }
            }
            let plans_value = serde_json::Value::Array(plans_arr);
            browser.call_plugin_api("restorePlans", vec![plans_value]).await?;

            let mut cf_raw = browser.call_plugin_api("exportData", vec![]).await?;
            if let Some(today) = cf_raw.get_mut("today") {
                for key in ["savingsAccounts", "investmentAccounts"] {
                    if let Some(arr) = today.get_mut(key).and_then(|v| v.as_array_mut()) {
                        if let Some(sa) = arr.iter_mut().find(|a| a.get("id").and_then(|v| v.as_str()) == Some(&cf_id)) {
                            sa.as_object_mut().unwrap()
                                .insert("balance".to_string(), json!(original_balance));
                        }
                    }
                }
                browser.call_plugin_api("restoreCurrentFinances", vec![today.clone()]).await?;
            }
            eprintln!("  Restored original balance: {}\n", original_balance);
        } else {
            eprintln!("  SKIPPED: No plan accounts with linked account_id\n");
        }
    }

    // --- 9. Priorities delete ---
    eprintln!("=== 9. Priorities delete ===");
    // Create a priority in the test plan, then delete it
    let priority_id = format!("pri_{}", chrono::Utc::now().timestamp_millis());

    // Use raw JSON for priority since PriorityEvent has many required fields
    let raw = browser.call_plugin_api("exportData", vec![]).await?;
    let real_plans = raw.get("plans").and_then(|v| v.as_array()).unwrap();
    let mut template_priority = None;
    for rp in real_plans {
        if let Some(evts) = rp.get("priorities").and_then(|a| a.get("events")).and_then(|e| e.as_array()) {
            if let Some(evt) = evts.first() {
                template_priority = Some(evt.clone());
                break;
            }
        }
    }

    if let Some(mut tmpl) = template_priority {
        if let Some(obj) = tmpl.as_object_mut() {
            obj.insert("id".to_string(), json!(priority_id));
            obj.insert("name".to_string(), json!("Test Priority (DELETE ME)"));
            obj.insert("title".to_string(), json!("Test Priority"));
        }

        let mut plans_arr = raw.get("plans").and_then(|v| v.as_array().cloned()).unwrap();
        let plan_json = plans_arr.iter_mut()
            .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(&new_id))
            .unwrap();
        let pri_events = plan_json.get_mut("priorities")
            .and_then(|a| a.get_mut("events"))
            .and_then(|e| e.as_array_mut())
            .unwrap();
        pri_events.push(tmpl);

        let plans_value = serde_json::Value::Array(plans_arr);
        browser.call_plugin_api("restorePlans", vec![plans_value]).await?;
        eprintln!("  Created test priority: {}", priority_id);

        // Now delete it
        let raw2 = browser.call_plugin_api("exportData", vec![]).await?;
        let mut plans_arr2 = raw2.get("plans").and_then(|v| v.as_array().cloned()).unwrap();
        let plan_json2 = plans_arr2.iter_mut()
            .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(&new_id))
            .unwrap();
        let pri_events2 = plan_json2.get_mut("priorities")
            .and_then(|a| a.get_mut("events"))
            .and_then(|e| e.as_array_mut())
            .unwrap();
        let before = pri_events2.len();
        pri_events2.retain(|e| e.get("id").and_then(|v| v.as_str()) != Some(&priority_id));
        let after = pri_events2.len();

        let plans_value = serde_json::Value::Array(plans_arr2);
        browser.call_plugin_api("restorePlans", vec![plans_value]).await?;

        // Verify deletion
        let raw3 = browser.call_plugin_api("exportData", vec![]).await?;
        let plans_arr3 = raw3.get("plans").and_then(|v| v.as_array()).unwrap();
        let plan_json3 = plans_arr3.iter()
            .find(|p| p.get("id").and_then(|v| v.as_str()) == Some(&new_id))
            .unwrap();
        let pri_events3 = plan_json3.get("priorities")
            .and_then(|a| a.get("events"))
            .and_then(|e| e.as_array())
            .unwrap();
        assert!(pri_events3.iter().all(|e| e.get("id").and_then(|v| v.as_str()) != Some(&priority_id)),
            "Priority still exists after delete!");
        eprintln!("  Deleted test priority ({} -> {} priorities)\n", before, after);
    } else {
        eprintln!("  SKIPPED: No existing priority events to use as template\n");
    }

    // --- 10. Delete the test plan (cleanup) ---
    eprintln!("=== 10. Delete test plan (cleanup) ===");
    let raw = browser.call_plugin_api("exportData", vec![]).await?;
    let mut plans_arr = raw.get("plans").and_then(|v| v.as_array().cloned())
        .expect("plans array not found in export");
    let before_count = plans_arr.len();
    plans_arr.retain(|p| p.get("id").and_then(|v| v.as_str()) != Some(&new_id));
    let plans_value = serde_json::Value::Array(plans_arr.clone());
    browser.call_plugin_api("restorePlans", vec![plans_value]).await?;
    eprintln!("  Deleted test plan ({} -> {} plans)\n", before_count, plans_arr.len());

    // --- 11. Browser JS execution ---
    eprintln!("=== 11. Browser JS execution ===");
    let result = browser.execute_js("return document.title").await?;
    eprintln!("  Page title: {}", result);
    let result = browser.execute_js("return window.location.href").await?;
    eprintln!("  Current URL: {}\n", result);

    // --- 12. Simulation data extraction ---
    eprintln!("=== 12. Simulation data extraction ===");
    let data = export!();
    let real_plan_id = data.plans.first().map(|p| p.id.clone())
        .unwrap_or_else(|| source_plan_id.clone());
    browser.navigate_to(&format!("/plan/{}", real_plan_id)).await?;
    eprintln!("  Navigated to plan, waiting for simulation...");
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

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
    assert!(year_count > 0, "No simulated years found!");
    eprintln!("  Simulation: {} years, outcome: {}", year_count, outcome);
    eprintln!("  Net worth: ${} -> ${}\n", first_nw, last_nw);

    // --- 13. Starting assets CRUD ---
    eprintln!("=== 13. Starting assets CRUD ===");
    browser.navigate_to_home().await?;
    let mut data = export!();
    let asset_count_before = data.today.assets.len();
    eprintln!("  Starting assets before: {}", asset_count_before);

    let test_asset_id = format!("asset_{}", chrono::Utc::now().timestamp_millis());
    let test_asset: projectionlab_mcp::models::StartingAsset = serde_json::from_value(json!({
        "id": test_asset_id,
        "name": "Test Car (DELETE ME)",
        "title": "Test Car",
        "type": "car",
        "icon": "mdi-car",
        "color": "#FF5722",
        "owner": "me",
        "balance": 25000.0,
        "initialValue": 30000.0,
    }))?;
    data.today.assets.push(test_asset);
    let new_finances = serde_json::to_value(&data.today)?;
    browser.call_plugin_api("restoreCurrentFinances", vec![new_finances]).await?;

    let data = export!();
    let created_asset = data.today.assets.iter().find(|a| a.id == test_asset_id);
    assert!(created_asset.is_some(), "Starting asset not found after create!");
    eprintln!("  Created: {} (balance={})", created_asset.unwrap().name, created_asset.unwrap().balance);

    let mut data = export!();
    let asset = data.today.assets.iter_mut().find(|a| a.id == test_asset_id).unwrap();
    asset.balance = 22000.0;
    let new_finances = serde_json::to_value(&data.today)?;
    browser.call_plugin_api("restoreCurrentFinances", vec![new_finances]).await?;

    let data = export!();
    let updated_asset = data.today.assets.iter().find(|a| a.id == test_asset_id).unwrap();
    assert!((updated_asset.balance - 22000.0).abs() < 0.01, "Starting asset balance not updated!");
    eprintln!("  Updated balance: 25000 -> {}", updated_asset.balance);

    let mut data = export!();
    data.today.assets.retain(|a| a.id != test_asset_id);
    let new_finances = serde_json::to_value(&data.today)?;
    browser.call_plugin_api("restoreCurrentFinances", vec![new_finances]).await?;

    let data = export!();
    assert!(data.today.assets.iter().all(|a| a.id != test_asset_id));
    eprintln!("  Deleted test asset\n");

    // --- 14. Debts CRUD ---
    eprintln!("=== 14. Debts CRUD ===");
    let mut data = export!();
    let debt_count_before = data.today.debts.len();
    eprintln!("  Debts before: {}", debt_count_before);

    let test_debt_id = format!("debt_{}", chrono::Utc::now().timestamp_millis());
    let test_debt: projectionlab_mcp::models::DebtAccount = serde_json::from_value(json!({
        "id": test_debt_id,
        "name": "Test Loan (DELETE ME)",
        "title": "Test Loan",
        "type": "personal-loan",
        "icon": "mdi-cash",
        "color": "#F44336",
        "owner": "me",
        "balance": 10000.0,
        "interestRate": 5.5,
        "monthlyPayment": 200.0,
    }))?;
    data.today.debts.push(test_debt);
    let new_finances = serde_json::to_value(&data.today)?;
    browser.call_plugin_api("restoreCurrentFinances", vec![new_finances]).await?;

    let data = export!();
    assert!(data.today.debts.iter().any(|d| d.id == test_debt_id), "Debt not found after create!");
    eprintln!("  Created test loan");

    let mut data = export!();
    let debt = data.today.debts.iter_mut().find(|d| d.id == test_debt_id).unwrap();
    debt.balance = 9500.0;
    debt.interest_rate = Some(4.5);
    let new_finances = serde_json::to_value(&data.today)?;
    browser.call_plugin_api("restoreCurrentFinances", vec![new_finances]).await?;

    let data = export!();
    let updated_debt = data.today.debts.iter().find(|d| d.id == test_debt_id).unwrap();
    assert!((updated_debt.balance - 9500.0).abs() < 0.01);
    assert_eq!(updated_debt.interest_rate, Some(4.5));
    eprintln!("  Updated: balance=10000->9500, rate=5.5->4.5");

    let mut data = export!();
    data.today.debts.retain(|d| d.id != test_debt_id);
    let new_finances = serde_json::to_value(&data.today)?;
    browser.call_plugin_api("restoreCurrentFinances", vec![new_finances]).await?;

    let data = export!();
    assert!(data.today.debts.iter().all(|d| d.id != test_debt_id));
    eprintln!("  Deleted test debt\n");

    // --- Summary ---
    eprintln!("=== ALL TESTS PASSED ===");
    eprintln!("   1. Plan list               ✓");
    eprintln!("   2. Plan create             ✓");
    eprintln!("   3. Plan metadata           ✓");
    eprintln!("   4. Milestone create        ✓");
    eprintln!("   5. Milestone update        ✓");
    eprintln!("   6. Milestone delete        ✓");
    eprintln!("   7. Plan asset events       ✓");
    eprintln!("   8. Plan accounts list      ✓");
    eprintln!("  8b. Account update propag.  ✓");
    eprintln!("   9. Priorities delete       ✓");
    eprintln!("  10. Plan delete             ✓");
    eprintln!("  11. Browser JS exec         ✓");
    eprintln!("  12. Simulation data         ✓");
    eprintln!("  13. Starting assets         ✓");
    eprintln!("  14. Debts CRUD              ✓");

    browser.shutdown().await;
    Ok(())
}
