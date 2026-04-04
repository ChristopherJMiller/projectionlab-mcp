/// Settings and Progress types for ProjectionLab
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub schema: f64,
    pub last_updated: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    pub plugins: PluginSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginSettings {
    pub enabled: bool,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub data: Vec<ProgressDataPoint>,
    pub last_updated: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProgressDataPoint {
    pub date: i64,
    pub net_worth: f64,
    pub savings: f64,
    pub taxable: f64,
    pub tax_deferred: f64,
    pub tax_free: f64,
    pub assets: f64,
    pub debt: f64,
    pub loans: f64,
    pub crypto: f64,
}
