/// Starting asset types for ProjectionLab starting conditions
///
/// Distinct from plan-level `AssetEvent` — these represent assets in
/// the user's current financial state (starting conditions), not
/// future planned asset events.
use super::common::Owner;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An asset in starting conditions (e.g., car, real estate, valuables)
///
/// Uses `#[serde(flatten)]` to capture any fields not explicitly defined,
/// ensuring forward compatibility with new ProjectionLab schema versions.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StartingAsset {
    pub id: String,
    pub name: String,
    pub title: String,
    #[serde(rename = "type")]
    pub asset_type: String,
    pub icon: String,
    pub color: String,
    pub owner: Owner,

    // Value
    pub balance: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_value: Option<f64>,

    // Loan/financing (if asset has attached debt)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interest_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monthly_payment: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compounding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<String>,

    // Metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquid: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<f64>,

    /// Catch-all for any fields not explicitly defined above.
    /// Ensures we never lose data during round-trip serialization.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}
