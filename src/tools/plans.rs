//! Plan management, metadata, variables, and milestone tools.

use crate::sync::SyncManager;
use serde_json::{json, Value as JsonValue};

use super::doc::{self, anyhow_err, err, ok_json, ToolResult};
use super::params::*;

pub async fn list(sync: &SyncManager) -> ToolResult {
    let data = sync.get_data().await.map_err(anyhow_err)?;
    let active = data.active_plan_id();
    let baseline = data.baseline_plan_id();

    let plans: Vec<JsonValue> = data
        .plans()
        .iter()
        .map(|plan| {
            json!({
                "id": plan.id,
                "name": plan.name,
                "icon": plan.icon,
                "is_active": active == Some(plan.id.as_str()),
                "is_baseline": baseline == Some(plan.id.as_str()),
                "starting_conditions": plan.starting_conditions_type.as_deref().unwrap_or("default"),
                "last_updated": plan.last_updated,
                "milestones": plan.milestones.len(),
                "expenses": plan.expenses.events.len(),
                "income": plan.income.events.len(),
                "priorities": plan.priorities.events.len(),
                "assets": plan.assets.events.len(),
                "accounts": plan.accounts.events.len(),
                "withdrawal_strategy": plan.withdrawal_strategy_name(),
            })
        })
        .collect();

    ok_json(&json!({
        "total_plans": plans.len(),
        "active_plan_id": active,
        "baseline_plan_id": baseline,
        "plans": plans,
    }))
}

/// Find a plan in the raw export (read-only).
fn raw_plan<'a>(raw: &'a JsonValue, plan_id: &str) -> Result<&'a JsonValue, rmcp::ErrorData> {
    raw.pointer("/documents/plans")
        .and_then(JsonValue::as_array)
        .and_then(|plans| {
            plans
                .iter()
                .find(|p| p.get("id").and_then(JsonValue::as_str) == Some(plan_id))
        })
        .ok_or_else(|| err(format!("Plan not found: {}", plan_id)))
}

pub async fn get(sync: &SyncManager, params: PlanGetParams) -> ToolResult {
    let raw = sync.get_raw().await.map_err(anyhow_err)?;
    ok_json(raw_plan(&raw, &params.plan_id)?)
}

pub async fn get_variables(sync: &SyncManager, params: PlanGetParams) -> ToolResult {
    let raw = sync.get_raw().await.map_err(anyhow_err)?;
    let plan = raw_plan(&raw, &params.plan_id)?;
    ok_json(&json!({
        "plan_id": params.plan_id,
        "plan_name": plan.get("name"),
        "variables": plan.get("variables"),
        "withdrawal_strategy": plan.get("withdrawalStrategy"),
        "montecarlo": plan.get("montecarlo"),
    }))
}

pub async fn update_variables(sync: &SyncManager, params: PlansUpdateVariablesParams) -> ToolResult {
    let mut raw = sync.get_raw().await.map_err(anyhow_err)?;
    let plan = doc::plan_mut(&mut raw, &params.plan_id)?;
    let variables = plan
        .as_object_mut()
        .ok_or_else(|| err("Plan is not an object"))?
        .entry("variables")
        .or_insert_with(|| json!({}));
    doc::merge_shallow(variables, &params.updates)?;
    let plans = doc::take_plans(&mut raw)?;
    sync.restore_plans(plans).await.map_err(anyhow_err)?;

    ok_json(&json!({
        "message": format!("Plan '{}' variables updated", params.plan_id),
        "updated_keys": params.updates.keys().collect::<Vec<_>>(),
    }))
}

/// Next free `meta.index` (ProjectionLab's plan ordering).
fn next_plan_index(plans: &[JsonValue]) -> i64 {
    plans
        .iter()
        .filter_map(|p| p.pointer("/meta/index").and_then(JsonValue::as_i64))
        .max()
        .map_or(0, |m| m + 1)
}

/// Re-identify a copied plan.
fn stamp_new_plan(plan: &mut JsonValue, id: &str, name: &str, icon: Option<&str>, index: i64) {
    plan["id"] = json!(id);
    plan["name"] = json!(name);
    if let Some(icon) = icon {
        plan["icon"] = json!(icon);
    }
    plan["lastUpdated"] = json!(chrono::Utc::now().timestamp_millis());
    if let Some(meta) = plan.get_mut("meta").and_then(JsonValue::as_object_mut) {
        meta.insert("index".into(), json!(index));
    }
}

pub async fn clone_plan(sync: &SyncManager, params: PlansCloneParams) -> ToolResult {
    create(
        sync,
        PlansCreateParams {
            name: params.new_name,
            icon: String::new(),
            clone_from: Some(params.source_plan_id),
        },
    )
    .await
}

pub async fn create(sync: &SyncManager, params: PlansCreateParams) -> ToolResult {
    let new_id = doc::new_id("plan");
    let mut raw = sync.get_raw().await.map_err(anyhow_err)?;
    let plans = doc::plans_mut(&mut raw)?;
    let index = next_plan_index(plans);
    let icon = (!params.icon.is_empty()).then_some(params.icon.as_str());

    let new_plan = if let Some(source_id) = &params.clone_from {
        let mut plan = plans
            .iter()
            .find(|p| p.get("id").and_then(JsonValue::as_str) == Some(source_id.as_str()))
            .ok_or_else(|| err(format!("Source plan not found: {}", source_id)))?
            .clone();
        stamp_new_plan(&mut plan, &new_id, &params.name, icon, index);
        plan
    } else {
        // Start from an existing plan's structure (variables, withdrawal strategy,
        // built-in milestones, per-account settings) with user events removed.
        let mut plan = plans
            .first()
            .ok_or_else(|| err("No existing plans to use as a template. Use clone_from instead."))?
            .clone();
        stamp_new_plan(&mut plan, &new_id, &params.name, icon.or(Some("mdi-file-document-outline")), index);
        for section in ["expenses", "income", "priorities", "assets"] {
            if let Some(events) = plan.pointer_mut(&format!("/{}/events", section)) {
                *events = json!([]);
            }
        }
        if let Some(milestones) = plan.get_mut("milestones").and_then(JsonValue::as_array_mut) {
            // Keep built-in milestones (retirement, lifeExpectancy, ...) which
            // the withdrawal strategy and other settings reference.
            milestones.retain(|m| m.get("removable").and_then(JsonValue::as_bool) != Some(true));
        }
        plan["startingConditionsType"] = json!("default");
        plan["startingConditions"] = json!({});
        plan
    };

    plans.push(new_plan);
    let plans = doc::take_plans(&mut raw)?;
    sync.restore_plans(plans).await.map_err(anyhow_err)?;

    let data = sync.get_data().await.map_err(anyhow_err)?;
    if data.find_plan(&new_id).is_none() {
        return Err(err(format!(
            "ProjectionLab accepted the request but plan {} does not appear in the export afterwards",
            new_id
        )));
    }

    ok_json(&json!({
        "message": format!("Plan '{}' created", params.name),
        "plan_id": new_id,
    }))
}

pub async fn delete(sync: &SyncManager, params: PlansDeleteParams) -> ToolResult {
    if !params.confirm {
        return Err(err("Deletion not confirmed. Set confirm=true to delete the plan."));
    }
    let data = sync.get_data().await.map_err(anyhow_err)?;
    if data.plans().len() <= 1 {
        return Err(err("Refusing to delete the only plan"));
    }
    let warnings: Vec<&str> = [
        (data.active_plan_id() == Some(params.plan_id.as_str()), "this was the active plan"),
        (data.baseline_plan_id() == Some(params.plan_id.as_str()), "this was the baseline plan"),
    ]
    .into_iter()
    .filter_map(|(hit, msg)| hit.then_some(msg))
    .collect();
    drop(data);

    let mut raw = sync.get_raw().await.map_err(anyhow_err)?;
    let plans = doc::plans_mut(&mut raw)?;
    let idx = doc::position_by_id(plans, &params.plan_id)
        .ok_or_else(|| err(format!("Plan not found: {}", params.plan_id)))?;
    let removed = plans.remove(idx);
    let plans = doc::take_plans(&mut raw)?;
    sync.restore_plans(plans).await.map_err(anyhow_err)?;

    let data = sync.get_data().await.map_err(anyhow_err)?;
    if data.find_plan(&params.plan_id).is_some() {
        return Err(err(format!(
            "ProjectionLab accepted the request but plan {} is still present",
            params.plan_id
        )));
    }

    ok_json(&json!({
        "message": format!(
            "Plan '{}' ({}) deleted",
            removed.get("name").and_then(JsonValue::as_str).unwrap_or(""),
            params.plan_id
        ),
        "warnings": warnings,
    }))
}

pub async fn update_metadata(sync: &SyncManager, params: PlansUpdateMetadataParams) -> ToolResult {
    const ALLOWED_KEYS: &[&str] = &["name", "icon"];
    if params.updates.contains_key("active") {
        return Err(err(
            "ProjectionLab V5 removed the per-plan `active` flag. The plan selected in the UI is settings.plan.activeId (see plans_list).",
        ));
    }
    for key in params.updates.keys() {
        if !ALLOWED_KEYS.contains(&key.as_str()) {
            return Err(err(format!(
                "Key '{}' is not allowed in metadata updates. Allowed keys: {}",
                key,
                ALLOWED_KEYS.join(", ")
            )));
        }
        if !params.updates[key].is_string() {
            return Err(err(format!("`{}` must be a string", key)));
        }
    }

    let mut raw = sync.get_raw().await.map_err(anyhow_err)?;
    let plan = doc::plan_mut(&mut raw, &params.plan_id)?;
    doc::merge_shallow(plan, &params.updates)?;
    let plans = doc::take_plans(&mut raw)?;
    sync.restore_plans(plans).await.map_err(anyhow_err)?;

    ok_json(&json!({ "message": format!("Plan '{}' metadata updated", params.plan_id) }))
}

// ---- Milestones ----

pub async fn get_milestones(sync: &SyncManager, params: PlanGetParams) -> ToolResult {
    let raw = sync.get_raw().await.map_err(anyhow_err)?;
    let plan = raw_plan(&raw, &params.plan_id)?;
    ok_json(&json!({
        "plan_id": params.plan_id,
        "plan_name": plan.get("name"),
        "milestones": plan.get("milestones"),
        "note": "Built-in milestones (retirement, lifeExpectancy, fi, ...) have no `removable` flag and cannot be deleted. Projected completion dates come from plans_get_simulation_results (milestoneCompletions).",
    }))
}

pub async fn create_milestone(sync: &SyncManager, params: MilestoneCreateParams) -> ToolResult {
    let mut milestone = JsonValue::Object(params.data);
    let id = milestone
        .get("id")
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| doc::new_id("ms"));
    milestone["id"] = json!(id);
    if milestone.get("removable").is_none() {
        milestone["removable"] = json!(true);
    }
    if milestone.get("createdOn").is_none() {
        milestone["createdOn"] = json!(doc::today_iso());
    }
    if milestone.get("name").and_then(JsonValue::as_str).is_none()
        || !milestone.get("criteria").is_some_and(JsonValue::is_array)
    {
        return Err(err("Milestone requires `name` and a `criteria` array. See schema_help topic `milestone`."));
    }

    let mut raw = sync.get_raw().await.map_err(anyhow_err)?;
    let plan = doc::plan_mut(&mut raw, &params.plan_id)?;
    let milestones = doc::array_field_mut(plan, "milestones")?;
    if doc::position_by_id(milestones, &id).is_some() {
        return Err(err(format!("A milestone with id {} already exists", id)));
    }
    milestones.push(milestone);
    let plans = doc::take_plans(&mut raw)?;
    sync.restore_plans(plans).await.map_err(anyhow_err)?;

    ok_json(&json!({ "message": "Milestone created", "id": id }))
}

pub async fn update_milestone(sync: &SyncManager, params: MilestoneUpdateParams) -> ToolResult {
    doc::reject_id_change(&params.data, &params.milestone_id)?;
    let mut raw = sync.get_raw().await.map_err(anyhow_err)?;
    let plan = doc::plan_mut(&mut raw, &params.plan_id)?;
    let milestones = doc::array_field_mut(plan, "milestones")?;
    let milestone = doc::find_by_id_mut(milestones, &params.milestone_id)
        .ok_or_else(|| err(format!("Milestone not found: {}", params.milestone_id)))?;
    doc::merge_shallow(milestone, &params.data)?;
    let plans = doc::take_plans(&mut raw)?;
    sync.restore_plans(plans).await.map_err(anyhow_err)?;

    ok_json(&json!({ "message": format!("Milestone '{}' updated", params.milestone_id) }))
}

pub async fn delete_milestone(sync: &SyncManager, params: MilestoneDeleteParams) -> ToolResult {
    let mut raw = sync.get_raw().await.map_err(anyhow_err)?;
    let plan = doc::plan_mut(&mut raw, &params.plan_id)?;
    let milestones = doc::array_field_mut(plan, "milestones")?;
    let idx = doc::position_by_id(milestones, &params.milestone_id)
        .ok_or_else(|| err(format!("Milestone not found: {}", params.milestone_id)))?;
    if milestones[idx].get("removable").and_then(JsonValue::as_bool) != Some(true) {
        return Err(err(format!(
            "Milestone '{}' is a built-in milestone and cannot be deleted (you can hide it with milestones_update {{\"hidden\": true}})",
            params.milestone_id
        )));
    }
    milestones.remove(idx);
    let plans = doc::take_plans(&mut raw)?;
    sync.restore_plans(plans).await.map_err(anyhow_err)?;

    ok_json(&json!({ "message": format!("Milestone '{}' deleted", params.milestone_id) }))
}
