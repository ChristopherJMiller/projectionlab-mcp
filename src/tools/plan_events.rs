//! Generic CRUD for plan event sections: expenses, income, priorities, and
//! planned real-asset purchases. (Plan-level account settings live in
//! `plan_accounts`, since edits there also propagate to Current Finances.)

use crate::models::PlanEvent;
use crate::sync::SyncManager;
use serde_json::{json, Value as JsonValue};

use super::doc::{self, anyhow_err, err, ok_json, ToolResult};
use super::params::*;

/// A plan section holding events.
#[derive(Clone, Copy, Debug)]
pub enum Section {
    Expenses,
    Income,
    Priorities,
    Assets,
}

impl Section {
    /// Key of the section container on the plan object.
    pub fn key(self) -> &'static str {
        match self {
            Section::Expenses => "expenses",
            Section::Income => "income",
            Section::Priorities => "priorities",
            Section::Assets => "assets",
        }
    }

    /// The `kind` ProjectionLab V5 stamps on every event in this section.
    pub fn kind(self) -> &'static str {
        match self {
            Section::Expenses => "expense",
            Section::Income => "income",
            Section::Priorities => "priority",
            Section::Assets => "real-asset",
        }
    }

    fn id_prefix(self) -> &'static str {
        match self {
            Section::Expenses => "exp",
            Section::Income => "inc",
            Section::Priorities => "pri",
            Section::Assets => "asset",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Section::Expenses => "Expense",
            Section::Income => "Income event",
            Section::Priorities => "Priority",
            Section::Assets => "Asset event",
        }
    }

    /// Fields included in list summaries.
    fn summary_fields(self) -> &'static [&'static str] {
        match self {
            Section::Expenses => &[
                "amount", "amountType", "frequency", "owner", "start", "end", "spendingType",
                "debtId", "interestRate", "monthlyPayment", "hidden",
            ],
            Section::Income => &[
                "amount", "amountType", "frequency", "owner", "start", "end", "taxExempt",
                "taxCharacter", "withholdingMode", "withholdingRate", "hidden",
            ],
            Section::Priorities => &[
                "owner", "accountId", "goalIntent", "start", "end", "amount", "amountType",
                "frequency", "contribution", "contributionType", "employerMatch",
                "employerMatchType", "employerMatchLimit", "mode", "hidden",
            ],
            Section::Assets => &[
                "owner", "start", "end", "value", "valueType", "purchasePrice",
                "purchasePriceType", "downPayment", "downPaymentType", "interestRate",
                "monthlyPayment", "paymentMethod",
            ],
        }
    }

    fn events(self, plan: &crate::models::Plan) -> &[PlanEvent] {
        match self {
            Section::Expenses => &plan.expenses.events,
            Section::Income => &plan.income.events,
            Section::Priorities => &plan.priorities.events,
            Section::Assets => &plan.assets.events,
        }
    }
}

pub async fn list(sync: &SyncManager, section: Section, params: PlanEventsListParams) -> ToolResult {
    let data = sync.get_data().await.map_err(anyhow_err)?;
    let plan = data
        .find_plan(&params.plan_id)
        .ok_or_else(|| err(format!("Plan not found: {}", params.plan_id)))?;
    let events: Vec<JsonValue> = section
        .events(plan)
        .iter()
        .map(|e| e.summary(section.summary_fields()))
        .collect();
    ok_json(&json!({
        "plan_id": plan.id,
        "plan_name": plan.name,
        "section": section.key(),
        "total": events.len(),
        "events": events,
    }))
}

pub async fn create(sync: &SyncManager, section: Section, params: PlanEventCreateParams) -> ToolResult {
    let mut event = JsonValue::Object(params.data);
    let event_id = event
        .get("id")
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| doc::new_id(section.id_prefix()));
    event["id"] = json!(event_id);
    if event.get("kind").is_none() {
        event["kind"] = json!(section.kind());
    }
    for required in ["name", "type"] {
        if event.get(required).and_then(JsonValue::as_str).is_none() {
            return Err(err(format!(
                "{} requires a `{}` string. See schema_help topic `{}`.",
                section.label(),
                required,
                section.kind()
            )));
        }
    }

    let mut raw = sync.get_raw().await.map_err(anyhow_err)?;
    let plan = doc::plan_mut(&mut raw, &params.plan_id)?;
    let events = doc::section_events_mut(plan, section.key())?;
    if doc::position_by_id(events, &event_id).is_some() {
        return Err(err(format!("An event with id {} already exists", event_id)));
    }
    events.push(event);
    let plans = doc::take_plans(&mut raw)?;
    sync.restore_plans(plans).await.map_err(anyhow_err)?;

    // ProjectionLab normalizes plans on commit and may silently drop events it
    // considers invalid (e.g. a second debt expense for the same debtId).
    if !event_exists(sync, section, &params.plan_id, &event_id).await? {
        return Err(err(format!(
            "ProjectionLab accepted the plan but did not keep the new {} (id {}). It was likely invalid or a duplicate (e.g. a second 'debt' expense for the same debtId). Check the fields against an existing event via schema_help topic `{}`.",
            section.label().to_lowercase(),
            event_id,
            section.kind()
        )));
    }

    ok_json(&json!({
        "message": format!("{} created", section.label()),
        "id": event_id,
    }))
}

/// Re-read (fresh, since writes invalidate the cache) and check for an event.
async fn event_exists(
    sync: &SyncManager,
    section: Section,
    plan_id: &str,
    event_id: &str,
) -> Result<bool, rmcp::ErrorData> {
    let data = sync.get_data().await.map_err(anyhow_err)?;
    Ok(data
        .find_plan(plan_id)
        .is_some_and(|p| section.events(p).iter().any(|e| e.id == event_id)))
}

pub async fn update(sync: &SyncManager, section: Section, params: PlanEventUpdateParams) -> ToolResult {
    doc::reject_id_change(&params.data, &params.event_id)?;
    let mut raw = sync.get_raw().await.map_err(anyhow_err)?;
    let plan = doc::plan_mut(&mut raw, &params.plan_id)?;
    let events = doc::section_events_mut(plan, section.key())?;
    let event = doc::find_by_id_mut(events, &params.event_id)
        .ok_or_else(|| err(format!("{} not found: {}", section.label(), params.event_id)))?;
    doc::merge_shallow(event, &params.data)?;
    let updated = event.clone();
    let plans = doc::take_plans(&mut raw)?;
    sync.restore_plans(plans).await.map_err(anyhow_err)?;

    ok_json(&json!({
        "message": format!("{} {} updated", section.label(), params.event_id),
        "event": updated,
    }))
}

pub async fn delete(sync: &SyncManager, section: Section, params: PlanEventDeleteParams) -> ToolResult {
    let mut raw = sync.get_raw().await.map_err(anyhow_err)?;
    let plan = doc::plan_mut(&mut raw, &params.plan_id)?;
    let events = doc::section_events_mut(plan, section.key())?;
    let idx = doc::position_by_id(events, &params.event_id)
        .ok_or_else(|| err(format!("{} not found: {}", section.label(), params.event_id)))?;
    let removed = events.remove(idx);
    let plans = doc::take_plans(&mut raw)?;
    sync.restore_plans(plans).await.map_err(anyhow_err)?;

    if event_exists(sync, section, &params.plan_id, &params.event_id).await? {
        return Err(err(format!(
            "ProjectionLab accepted the plan but the {} {} is still present (it may be auto-generated, e.g. medicare).",
            section.label().to_lowercase(),
            params.event_id
        )));
    }

    ok_json(&json!({
        "message": format!(
            "{} '{}' ({}) deleted",
            section.label(),
            removed.get("name").and_then(JsonValue::as_str).unwrap_or(""),
            params.event_id
        ),
    }))
}
