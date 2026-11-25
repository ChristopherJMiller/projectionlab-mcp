//! Helpers for editing the raw ProjectionLab export JSON and building tool results.
//!
//! All write tools fetch the raw export, edit it in place with these helpers,
//! and send the edited document back. Working on raw JSON (rather than
//! round-tripping through typed structs) guarantees that fields this server
//! doesn't know about are preserved exactly.

use rmcp::{model::*, ErrorData as McpError};
use serde_json::{Map, Value};

pub type ToolResult = Result<CallToolResult, McpError>;

/// Build an MCP internal error from anything displayable.
pub fn err(msg: impl std::fmt::Display) -> McpError {
    McpError::internal_error(msg.to_string(), None)
}

/// Convert an anyhow error (with its context chain) into an MCP error.
pub fn anyhow_err(e: anyhow::Error) -> McpError {
    McpError::internal_error(format!("{:#}", e), None)
}

/// Successful result containing pretty-printed JSON.
pub fn ok_json(value: &Value) -> ToolResult {
    Ok(CallToolResult::success(vec![Content::text(
        serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
    )]))
}

/// Successful result containing plain text.
pub fn ok_text(text: impl Into<String>) -> ToolResult {
    Ok(CallToolResult::success(vec![Content::text(text.into())]))
}

/// Generate an id in ProjectionLab's `<prefix>_<millis>` style.
pub fn new_id(prefix: &str) -> String {
    format!("{}_{}", prefix, chrono::Utc::now().timestamp_millis())
}

/// Today's date as an ISO `YYYY-MM-DD` string.
pub fn today_iso() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn documents_mut(raw: &mut Value) -> Result<&mut Map<String, Value>, McpError> {
    raw.get_mut("documents")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| err("Export is missing `documents`"))
}

/// `documents.plans` (an array).
pub fn plans_mut(raw: &mut Value) -> Result<&mut Vec<Value>, McpError> {
    documents_mut(raw)?
        .get_mut("plans")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| err("Export is missing `documents.plans`"))
}

/// `documents.currentFinances` (an object).
pub fn current_finances_mut(raw: &mut Value) -> Result<&mut Value, McpError> {
    documents_mut(raw)?
        .get_mut("currentFinances")
        .filter(|v| v.is_object())
        .ok_or_else(|| err("Export is missing `documents.currentFinances`"))
}

/// Take ownership of `documents.plans` for sending to `restorePlans`.
pub fn take_plans(raw: &mut Value) -> Result<Value, McpError> {
    Ok(Value::Array(std::mem::take(plans_mut(raw)?)))
}

/// Take ownership of `documents.currentFinances` for `restoreCurrentFinances`.
pub fn take_current_finances(raw: &mut Value) -> Result<Value, McpError> {
    Ok(current_finances_mut(raw)?.take())
}

/// Find an element of an array of objects by its `id`.
pub fn find_by_id_mut<'a>(items: &'a mut [Value], id: &str) -> Option<&'a mut Value> {
    items
        .iter_mut()
        .find(|v| v.get("id").and_then(Value::as_str) == Some(id))
}

pub fn position_by_id(items: &[Value], id: &str) -> Option<usize> {
    items
        .iter()
        .position(|v| v.get("id").and_then(Value::as_str) == Some(id))
}

/// Find a plan by id within the raw export.
pub fn plan_mut<'a>(raw: &'a mut Value, plan_id: &str) -> Result<&'a mut Value, McpError> {
    find_by_id_mut(plans_mut(raw)?, plan_id)
        .ok_or_else(|| err(format!("Plan not found: {}", plan_id)))
}

/// `plan[section].events`, creating the container if absent.
pub fn section_events_mut<'a>(
    plan: &'a mut Value,
    section: &str,
) -> Result<&'a mut Vec<Value>, McpError> {
    let plan_obj = plan
        .as_object_mut()
        .ok_or_else(|| err("Plan is not a JSON object"))?;
    let container = plan_obj
        .entry(section.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let container_obj = container
        .as_object_mut()
        .ok_or_else(|| err(format!("Plan section `{}` is not an object", section)))?;
    container_obj
        .entry("events".to_string())
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| err(format!("Plan section `{}.events` is not an array", section)))
}

/// `obj[key]` as a mutable array, creating it if absent.
pub fn array_field_mut<'a>(obj: &'a mut Value, key: &str) -> Result<&'a mut Vec<Value>, McpError> {
    obj.as_object_mut()
        .ok_or_else(|| err("Expected a JSON object"))?
        .entry(key.to_string())
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| err(format!("`{}` is not an array", key)))
}

/// Shallow merge: each top-level key in `patch` replaces the key in `target`.
pub fn merge_shallow(target: &mut Value, patch: &Map<String, Value>) -> Result<(), McpError> {
    let obj = target
        .as_object_mut()
        .ok_or_else(|| err("Cannot merge into a non-object"))?;
    for (k, v) in patch {
        obj.insert(k.clone(), v.clone());
    }
    Ok(())
}

/// Reject attempts to change an entity's `id` through an update payload.
pub fn reject_id_change(patch: &Map<String, Value>, id: &str) -> Result<(), McpError> {
    match patch.get("id").and_then(Value::as_str) {
        Some(new_id) if new_id != id => Err(err(format!(
            "Changing `id` is not supported (attempted {} -> {})",
            id, new_id
        ))),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn export() -> Value {
        json!({
            "schema": 1,
            "exportedAt": 1,
            "documents": {
                "currentFinances": {"accounts": {}},
                "plans": [
                    {"id": "p1", "name": "One", "expenses": {"events": [{"id": "e1", "x": 1}]}, "unknown": {"keep": true}},
                    {"id": "p2", "name": "Two"}
                ],
                "settings": {}
            }
        })
    }

    #[test]
    fn edits_preserve_unknown_fields() {
        let mut raw = export();
        let plan = plan_mut(&mut raw, "p1").unwrap();
        let events = section_events_mut(plan, "expenses").unwrap();
        let ev = find_by_id_mut(events, "e1").unwrap();
        merge_shallow(ev, json!({"x": 2, "y": 3}).as_object().unwrap()).unwrap();

        assert_eq!(raw["documents"]["plans"][0]["expenses"]["events"][0], json!({"id": "e1", "x": 2, "y": 3}));
        assert_eq!(raw["documents"]["plans"][0]["unknown"], json!({"keep": true}));
    }

    #[test]
    fn section_container_created_on_demand() {
        let mut raw = export();
        let plan = plan_mut(&mut raw, "p2").unwrap();
        section_events_mut(plan, "income").unwrap().push(json!({"id": "i1"}));
        assert_eq!(raw["documents"]["plans"][1]["income"]["events"][0]["id"], "i1");
    }

    #[test]
    fn take_plans_returns_all_plans() {
        let mut raw = export();
        let plans = take_plans(&mut raw).unwrap();
        assert_eq!(plans.as_array().unwrap().len(), 2);
    }

    #[test]
    fn missing_plan_is_an_error() {
        let mut raw = export();
        assert!(plan_mut(&mut raw, "nope").is_err());
    }

    #[test]
    fn id_change_rejected() {
        let patch = json!({"id": "other"});
        assert!(reject_id_change(patch.as_object().unwrap(), "e1").is_err());
        let same = json!({"id": "e1"});
        assert!(reject_id_change(same.as_object().unwrap(), "e1").is_ok());
    }
}
