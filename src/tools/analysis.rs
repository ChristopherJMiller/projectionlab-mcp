//! Projection analysis, tax strategy, what-if, plan comparison, baseline, and
//! Monte Carlo tools. The heavy lifting happens inside the ProjectionLab app
//! (see `simulation.rs`); these handlers validate input and shape requests.

use crate::sync::SyncManager;
use serde_json::{json, Map as JsonMap, Value as JsonValue};

use super::doc::{self, anyhow_err, err, ok_json, ToolResult};
use super::params::*;
use super::simulation;

async fn run(sync: &SyncManager, script: &str) -> ToolResult {
    let result = sync.run_js_async(script).await.map_err(anyhow_err)?;
    ok_json(&result)
}

pub async fn simulation_results(sync: &SyncManager, params: PlanGetParams) -> ToolResult {
    run(sync, &simulation::for_plan(&params.plan_id, simulation::SIMULATION_SUMMARY)).await
}

pub async fn year_snapshot(sync: &SyncManager, params: YearSnapshotParams) -> ToolResult {
    let extractor = simulation::year_snapshot_extractor(params.age);
    run(sync, &simulation::for_plan(&params.plan_id, &extractor)).await
}

pub async fn report(sync: &SyncManager, params: ReportParams) -> ToolResult {
    if let (Some(s), Some(e)) = (params.start_age, params.end_age) {
        if s > e {
            return Err(err("start_age must not be greater than end_age"));
        }
    }
    let default = ["netWorth", "income", "expenses", "taxes", "contributions", "drawdown"];
    let metrics: Vec<&str> = match &params.metrics {
        Some(m) if !m.is_empty() => m.iter().map(String::as_str).collect(),
        _ => default.to_vec(),
    };
    let extractor =
        simulation::report_extractor(&metrics, params.start_age, params.end_age, params.line_items);
    run(sync, &simulation::for_plan(&params.plan_id, &extractor)).await
}

pub async fn tax_analytics(sync: &SyncManager, params: TaxAnalyticsParams) -> ToolResult {
    let extractor =
        simulation::tax_analytics_extractor(params.start_age, params.end_age, params.detail_age);
    run(sync, &simulation::for_plan(&params.plan_id, &extractor)).await
}

pub async fn cash_flow(sync: &SyncManager, params: PlanYearParams) -> ToolResult {
    let extractor = simulation::cash_flow_extractor(params.age);
    run(sync, &simulation::for_plan(&params.plan_id, &extractor)).await
}

pub async fn estate(sync: &SyncManager, params: PlanYearParams) -> ToolResult {
    let extractor = simulation::estate_extractor(params.age);
    run(sync, &simulation::for_plan(&params.plan_id, &extractor)).await
}

pub async fn roth_conversions(sync: &SyncManager, params: PlanGetParams) -> ToolResult {
    run(sync, &simulation::for_plan(&params.plan_id, simulation::ROTH_CONVERSIONS)).await
}

pub async fn tax_strategy(sync: &SyncManager, params: PlanGetParams) -> ToolResult {
    run(sync, &simulation::for_plan(&params.plan_id, simulation::TAX_STRATEGY_INFO)).await
}

const TAX_STRATEGY_TYPES: &[&str] = &["bracket", "irmaa", "income", "federalPovertyLevel"];

/// Persist tax optimization settings into plan.variables.
pub async fn update_tax_strategy(sync: &SyncManager, params: UpdateTaxStrategyParams) -> ToolResult {
    let objects: Vec<(&str, &Option<JsonMap<String, JsonValue>>)> = vec![
        ("taxStrategy", &params.tax_strategy),
        ("rothConversionStrategy", &params.roth_conversions),
        ("drawdownBlendingStrategy", &params.withdrawal_shielding),
        ("gainHarvestingStrategy", &params.gain_harvesting),
        ("flexSpending", &params.flexible_spending),
    ];
    if objects.iter().all(|(_, v)| v.is_none()) && params.drawdown_order.is_none() {
        return Err(err("Nothing to update: pass at least one of tax_strategy, roth_conversions, withdrawal_shielding, gain_harvesting, flexible_spending, drawdown_order"));
    }
    if let Some(t) = params.tax_strategy.as_ref().and_then(|t| t.get("type")) {
        if !t.as_str().is_some_and(|t| TAX_STRATEGY_TYPES.contains(&t)) {
            return Err(err(format!(
                "tax_strategy.type must be one of {}",
                TAX_STRATEGY_TYPES.join(", ")
            )));
        }
    }

    let mut raw = sync.get_raw().await.map_err(anyhow_err)?;
    let plan = doc::plan_mut(&mut raw, &params.plan_id)?;
    let variables = plan
        .as_object_mut()
        .ok_or_else(|| err("Plan is not an object"))?
        .entry("variables")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| err("plan.variables is not an object"))?;
    let mut changed = Vec::new();
    for (key, patch) in &objects {
        if let Some(patch) = patch {
            let target = variables
                .entry(key.to_string())
                .or_insert_with(|| json!({ "enabled": false }));
            if !target.is_object() {
                *target = json!({ "enabled": false });
            }
            doc::merge_shallow(target, patch)?;
            changed.push(*key);
        }
    }
    if let Some(order) = &params.drawdown_order {
        variables.insert("drawdownOrder".into(), JsonValue::Array(order.clone()));
        changed.push("drawdownOrder");
    }
    let plans = doc::take_plans(&mut raw)?;
    sync.restore_plans(plans).await.map_err(anyhow_err)?;

    // Report what ProjectionLab kept (it normalizes invalid strategy settings).
    let after = sync.get_raw().await.map_err(anyhow_err)?;
    let stored: JsonMap<String, JsonValue> = after
        .pointer("/documents/plans")
        .and_then(JsonValue::as_array)
        .and_then(|p| p.iter().find(|p| p.get("id").and_then(JsonValue::as_str) == Some(params.plan_id.as_str())))
        .and_then(|p| p.get("variables"))
        .map(|v| {
            changed
                .iter()
                .map(|k| (k.to_string(), v.get(*k).cloned().unwrap_or(JsonValue::Null)))
                .collect()
        })
        .unwrap_or_default();
    ok_json(&json!({
        "message": format!("Updated {} on plan {}", changed.join(", "), params.plan_id),
        "stored": stored,
        "next": "Use plans_get_roth_conversions / plans_get_tax_analytics to see the effect, or plans_optimize_tax_strategy to compare alternatives.",
    }))
}

const SECTIONS: &[&str] = &["expenses", "income", "priorities", "assets", "accounts"];

/// Translate tool-level scenarios into the camelCase shape the in-app runner expects.
fn scenario_json(sc: &WhatIfScenario) -> Result<JsonValue, rmcp::ErrorData> {
    let mut out = json!({ "name": sc.name });
    if let Some(d) = &sc.description {
        out["description"] = json!(d);
    }
    if let Some(v) = &sc.variables {
        out["variables"] = json!(v);
    }
    if let Some(v) = &sc.withdrawal_strategy {
        out["withdrawalStrategy"] = json!(v);
    }
    if let Some(events) = &sc.events {
        let mut list = Vec::new();
        for ch in events {
            if !SECTIONS.contains(&ch.section.as_str()) {
                return Err(err(format!(
                    "Scenario '{}': unknown section '{}' (expected {})",
                    sc.name,
                    ch.section,
                    SECTIONS.join(", ")
                )));
            }
            let ops = [ch.add.is_some(), ch.patch.is_some(), ch.remove]
                .iter()
                .filter(|b| **b)
                .count();
            if ops != 1 {
                return Err(err(format!(
                    "Scenario '{}': each event change needs exactly one of add, patch, or remove",
                    sc.name
                )));
            }
            if ch.add.is_none() && ch.id.is_none() {
                return Err(err(format!(
                    "Scenario '{}': patch/remove need the event id",
                    sc.name
                )));
            }
            list.push(json!({
                "section": ch.section, "id": ch.id, "patch": ch.patch, "add": ch.add, "remove": ch.remove,
            }));
        }
        out["events"] = JsonValue::Array(list);
    }
    if let Some(m) = &sc.milestones {
        out["milestones"] = json!(m);
    }
    if let Some(a) = &sc.accounts {
        out["accounts"] = json!(a);
    }
    Ok(out)
}

pub async fn what_if(sync: &SyncManager, params: WhatIfParams) -> ToolResult {
    if params.scenarios.is_empty() {
        return Err(err("Provide at least one scenario"));
    }
    if params.scenarios.len() > 8 {
        return Err(err("At most 8 scenarios per call"));
    }
    let scenarios = params
        .scenarios
        .iter()
        .map(scenario_json)
        .collect::<Result<Vec<_>, _>>()?;
    run(sync, &simulation::what_if(&params.plan_id, &JsonValue::Array(scenarios))).await
}

pub async fn optimize(sync: &SyncManager, params: OptimizeParams) -> ToolResult {
    const MODULES: &[&str] = &["tax", "roth", "withdrawal_shielding", "gain_harvesting"];
    if !MODULES.contains(&params.module.as_str()) {
        return Err(err(format!("module must be one of {}", MODULES.join(", "))));
    }
    run(sync, &simulation::optimize(&params.plan_id, &params.module)).await
}

pub async fn compare_plans(sync: &SyncManager, params: ComparePlansParams) -> ToolResult {
    let others = match params.other_plan_ids {
        Some(ids) if !ids.is_empty() => ids,
        _ => {
            let data = sync.get_data().await.map_err(anyhow_err)?;
            match data.baseline_plan_id() {
                Some(b) if b != params.plan_id => vec![b.to_string()],
                _ => {
                    return Err(err(
                        "No other plan to compare with: pass other_plan_ids (this plan is the baseline, or none is set)",
                    ))
                }
            }
        }
    };
    let mut ids = vec![params.plan_id];
    ids.extend(others);
    if ids.len() > 6 {
        return Err(err("Compare at most 6 plans at once"));
    }
    run(sync, &simulation::compare_plans(&ids)).await
}

pub async fn set_baseline(sync: &SyncManager, params: PlanGetParams) -> ToolResult {
    {
        let data = sync.get_data().await.map_err(anyhow_err)?;
        let plan = data
            .find_plan(&params.plan_id)
            .ok_or_else(|| err(format!("Plan not found: {}", params.plan_id)))?;
        if !plan.is_baseline_eligible() {
            let eligible: Vec<String> = data
                .plans()
                .iter()
                .filter(|p| p.is_baseline_eligible())
                .map(|p| format!("{} ({})", p.name, p.id))
                .collect();
            return Err(err(format!(
                "Plan '{}' can't be the baseline: ProjectionLab requires a plan that projects from now (variables.projectFrom = \"now\") and starts from Current Finances (startingConditionsType = \"default\"). Eligible plans: {}",
                plan.name,
                if eligible.is_empty() { "none".to_string() } else { eligible.join(", ") }
            )));
        }
    }
    let result = sync
        .run_js_write(&simulation::set_baseline(&params.plan_id))
        .await
        .map_err(anyhow_err)?;
    let data = sync.get_data().await.map_err(anyhow_err)?;
    let stored = data.baseline_plan_id().map(str::to_string);
    if stored.as_deref() != Some(params.plan_id.as_str()) {
        return Err(err(format!(
            "ProjectionLab did not keep the baseline change (baseline is {:?})",
            stored
        )));
    }
    ok_json(&json!({
        "message": format!("Baseline plan set to {}", params.plan_id),
        "previous": result.get("previous"),
    }))
}

pub async fn monte_carlo(sync: &SyncManager, params: MonteCarloParams) -> ToolResult {
    let settings = params.settings.map(JsonValue::Object);
    run(sync, &simulation::monte_carlo(&params.plan_id, settings.as_ref())).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scenario(events: Vec<WhatIfEventChange>) -> WhatIfScenario {
        WhatIfScenario {
            name: "s".into(),
            description: None,
            variables: Some(json!({"inflation": 4}).as_object().unwrap().clone()),
            withdrawal_strategy: Some(json!({"enabled": true}).as_object().unwrap().clone()),
            events: Some(events),
            milestones: None,
            accounts: None,
        }
    }

    #[test]
    fn scenario_uses_camel_case_keys() {
        let v = scenario_json(&scenario(vec![])).unwrap();
        assert_eq!(v["withdrawalStrategy"], json!({"enabled": true}));
        assert_eq!(v["variables"]["inflation"], json!(4));
    }

    #[test]
    fn event_change_requires_exactly_one_operation() {
        let bad = WhatIfEventChange { section: "expenses".into(), id: Some("e".into()), patch: Some(JsonMap::new()), add: None, remove: true };
        assert!(scenario_json(&scenario(vec![bad])).is_err());
        let unknown = WhatIfEventChange { section: "nope".into(), id: Some("e".into()), patch: Some(JsonMap::new()), add: None, remove: false };
        assert!(scenario_json(&scenario(vec![unknown])).is_err());
        let no_id = WhatIfEventChange { section: "income".into(), id: None, patch: None, add: None, remove: true };
        assert!(scenario_json(&scenario(vec![no_id])).is_err());
        let ok = WhatIfEventChange { section: "income".into(), id: None, patch: None, add: Some(JsonMap::new()), remove: false };
        assert!(scenario_json(&scenario(vec![ok])).is_ok());
    }
}
