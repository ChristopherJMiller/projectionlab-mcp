/// ProjectionLab data models
///
/// This module contains all the data structures for ProjectionLab plans,
/// including events (expenses, income, priorities, assets, accounts),
/// milestones, settings, and configuration.

// Module declarations
pub mod common;
pub mod milestone;
pub mod expenses;
pub mod income;
pub mod priorities;
pub mod assets;
pub mod accounts;
pub mod debt;
pub mod starting_asset;
pub mod plan;
pub mod settings;
pub mod resources;

// Re-export types used by server, resources, and tests
pub use debt::DebtAccount;
pub use milestone::Milestone;
pub use expenses::ExpenseEvent;
pub use income::IncomeEvent;
pub use priorities::PriorityEvent;
pub use plan::{Plan, StartingConditions, StartingAccount};
pub use settings::{Progress, ProgressDataPoint, Settings};
pub use resources::{
    OverviewResource, AccountsSummaryResource, AccountSummary, AccountDetailsResource,
    ExpensesSummaryResource, ExpenseSummary,
};

// Re-exports used by tests and the library API only
#[allow(unused_imports)]
pub use starting_asset::StartingAsset;
#[allow(unused_imports)]
pub use plan::Variables;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Complete export from ProjectionLab API (exportData response)
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FullExport {
    pub meta: Meta,
    pub today: StartingConditions,
    pub plans: Vec<Plan>,
    pub settings: settings::Settings,
    pub progress: Progress,
}

/// Metadata about the export
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub version: String,
    pub last_updated: i64,
}

// Backward compatibility aliases
#[allow(dead_code)]
pub type CurrentFinances = StartingConditions;
pub type SavingsAccount = StartingAccount;
pub type InvestmentAccount = StartingAccount;
