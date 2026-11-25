//! A ProjectionLab plan (V5).

use super::current_finances::CurrentFinances;
use super::events::{Milestone, PlanEvent};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<i64>,

    /// "default" (plan starts from global Current Finances) or "custom"
    /// (plan carries its own `startingConditions` snapshot).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub starting_conditions_type: Option<String>,
    /// Empty for "default" plans.
    #[serde(default)]
    pub starting_conditions: CurrentFinances,

    #[serde(default)]
    pub expenses: EventContainer,
    #[serde(default)]
    pub income: EventContainer,
    #[serde(default)]
    pub priorities: EventContainer,
    /// Planned real-asset purchases (kind "real-asset").
    #[serde(default)]
    pub assets: EventContainer,
    /// Plan-level account settings (kind "account"); `accountId` links to Current Finances.
    #[serde(default)]
    pub accounts: EventContainer,
    #[serde(default)]
    pub milestones: Vec<Milestone>,

    #[serde(default)]
    pub variables: Value,
    #[serde(default)]
    pub withdrawal_strategy: Value,
    #[serde(default)]
    pub montecarlo: Value,
    #[serde(default)]
    pub meta: Value,

    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Plan {
    pub fn uses_custom_starting_conditions(&self) -> bool {
        self.starting_conditions_type.as_deref() == Some("custom")
    }

    /// ProjectionLab only allows plans that project from today and start from
    /// Current Finances to be the comparison baseline.
    pub fn is_baseline_eligible(&self) -> bool {
        self.variables.get("projectFrom").and_then(Value::as_str) == Some("now")
            && self.starting_conditions_type.as_deref().unwrap_or("default") == "default"
    }

    pub fn withdrawal_strategy_name(&self) -> Option<&str> {
        self.withdrawal_strategy.get("strategy").and_then(Value::as_str)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct EventContainer {
    #[serde(default)]
    pub events: Vec<PlanEvent>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
