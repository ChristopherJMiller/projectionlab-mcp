//! User settings document.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<PluginSettings>,
    /// Which plan is selected in the UI and which is the comparison baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<PlanSelection>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanSelection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_id: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}
