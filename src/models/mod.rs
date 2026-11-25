//! ProjectionLab data models (V5 export format).
//!
//! ProjectionLab's `exportData` returns a versioned envelope:
//!
//! ```text
//! { schema: 1, exportedAt, documents: { currentFinances, plans, settings, accountHistory } }
//! ```
//!
//! These models are deliberately *thin*: only identity fields (id, name, kind,
//! type) and the handful of values the server computes with are typed. Every
//! other field is preserved verbatim in a flattened `extra`/`fields` map.
//! ProjectionLab's schema evolves frequently, and a strict model turns every
//! upstream change into a total outage. All write paths operate on the raw
//! export JSON (see `tools::doc`), so nothing is ever lost in a round trip.

pub mod account_history;
pub mod current_finances;
pub mod events;
pub mod plan;
pub mod settings;

pub use account_history::{AccountHistory, AccountHistoryYear};
pub use current_finances::{CurrentFinances, CurrentFinancesAccount, Household, Person};
pub use events::{Milestone, PlanEvent};
pub use plan::{EventContainer, Plan};
pub use settings::{PlanSelection, PluginSettings, Settings};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Complete export from the ProjectionLab Plugin API (`exportData` response).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FullExport {
    /// Envelope schema version (1 as of ProjectionLab V5).
    pub schema: i64,
    /// Export timestamp (ms since epoch).
    pub exported_at: i64,
    pub documents: Documents,
}

/// The documents contained in an export.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Documents {
    pub current_finances: CurrentFinances,
    #[serde(default)]
    pub plans: Vec<Plan>,
    pub settings: Settings,
    /// Per-account value history, keyed by account id.
    #[serde(default)]
    pub account_history: BTreeMap<String, AccountHistory>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl FullExport {
    pub fn current_finances(&self) -> &CurrentFinances {
        &self.documents.current_finances
    }

    pub fn plans(&self) -> &[Plan] {
        &self.documents.plans
    }

    pub fn settings(&self) -> &Settings {
        &self.documents.settings
    }

    pub fn find_plan(&self, plan_id: &str) -> Option<&Plan> {
        self.documents.plans.iter().find(|p| p.id == plan_id)
    }

    /// The plan currently selected in the ProjectionLab UI.
    pub fn active_plan_id(&self) -> Option<&str> {
        self.documents.settings.plan.as_ref()?.active_id.as_deref()
    }

    /// The plan marked as the comparison baseline in ProjectionLab.
    pub fn baseline_plan_id(&self) -> Option<&str> {
        self.documents.settings.plan.as_ref()?.baseline_id.as_deref()
    }
}

/// Parse an export, reporting the exact JSON path of any failure.
pub fn parse_export(value: &Value) -> anyhow::Result<FullExport> {
    serde_path_to_error::deserialize(value).map_err(|e| {
        let top_keys = value
            .as_object()
            .map(|o| o.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        anyhow::anyhow!(
            "Failed to parse ProjectionLab export at `{}`: {} (top-level keys: {:?})",
            e.path(),
            e.inner(),
            top_keys
        )
    })
}
