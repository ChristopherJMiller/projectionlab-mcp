//! Per-account value history (`documents.accountHistory` in a V5 export).
//!
//! ProjectionLab records history automatically whenever a current value is
//! edited. Each account's history is sharded by calendar year; each series is a
//! map of ISO date → value holding only the dates on which the value changed.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// History for one account, keyed by year ("2025", "2026", ...).
pub type AccountHistory = BTreeMap<String, AccountHistoryYear>;

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AccountHistoryYear {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<f64>,
    /// ISO date → balance (accounts and debts)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub balance: BTreeMap<String, f64>,
    /// ISO date → cost basis (taxable-style accounts)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cost_basis: BTreeMap<String, f64>,
    /// ISO date → value (real assets)
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub value: BTreeMap<String, f64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// One merged history point across all of an account's series.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct HistoryPoint {
    pub date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_basis: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
}

/// Flatten an account's year shards into date-ordered points, optionally
/// restricted to an inclusive ISO date range.
pub fn flatten_history(
    history: &AccountHistory,
    start: Option<&str>,
    end: Option<&str>,
) -> Vec<HistoryPoint> {
    fn entry<'a>(points: &'a mut BTreeMap<String, HistoryPoint>, date: &str) -> &'a mut HistoryPoint {
        points.entry(date.to_string()).or_insert_with(|| HistoryPoint {
            date: date.to_string(),
            balance: None,
            cost_basis: None,
            value: None,
        })
    }
    let mut points: BTreeMap<String, HistoryPoint> = BTreeMap::new();
    for year in history.values() {
        for (d, v) in &year.balance {
            entry(&mut points, d).balance = Some(*v);
        }
        for (d, v) in &year.cost_basis {
            entry(&mut points, d).cost_basis = Some(*v);
        }
        for (d, v) in &year.value {
            entry(&mut points, d).value = Some(*v);
        }
    }
    points
        .into_values()
        .filter(|p| start.is_none_or(|s| p.date.as_str() >= s))
        .filter(|p| end.is_none_or(|e| p.date.as_str() <= e))
        .collect()
}
