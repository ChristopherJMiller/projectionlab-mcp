//! Net worth and account history.
//!
//! ProjectionLab V5 replaced the manual "progress" data points with history
//! that is recorded automatically whenever a current value changes:
//! - per-account history ships in the export (`documents.accountHistory`);
//! - the aggregated net worth series lives in the `net-worth-series` Pinia
//!   store (Firestore shards by year), which we read via the browser.

use crate::models::account_history::flatten_history;
use crate::sync::SyncManager;
use serde::Serialize;
use serde_json::{json, Value as JsonValue};
use std::collections::{BTreeMap, BTreeSet};

use super::doc::{anyhow_err, err, ok_json, ToolResult};
use super::params::*;

// ---- Per-account history (from the export) ----

fn validate_iso_dates(dates: &[&Option<String>]) -> Result<(), rmcp::ErrorData> {
    for d in dates.iter().copied().flatten() {
        chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d")
            .map_err(|_| err(format!("Invalid date `{}` (expected YYYY-MM-DD)", d)))?;
    }
    Ok(())
}

pub async fn account_history(sync: &SyncManager, params: AccountHistoryParams) -> ToolResult {
    validate_iso_dates(&[&params.start_date, &params.end_date])?;
    let data = sync.get_data().await.map_err(anyhow_err)?;
    let cf = data.current_finances();
    let start = params.start_date.as_deref();
    let end = params.end_date.as_deref();

    let ids: Vec<String> = match &params.account_id {
        Some(id) => {
            if cf.find(id).is_none() && !data.documents.account_history.contains_key(id) {
                return Err(err(format!("Account not found: {}", id)));
            }
            vec![id.clone()]
        }
        None => cf
            .sorted_accounts()
            .into_iter()
            .filter(|a| params.include_archived || !a.is_archived())
            .map(|a| a.id.clone())
            .collect(),
    };

    let accounts: Vec<JsonValue> = ids
        .iter()
        .map(|id| {
            let account = cf.find(id);
            let points = data
                .documents
                .account_history
                .get(id)
                .map(|h| flatten_history(h, start, end))
                .unwrap_or_default();
            json!({
                "id": id,
                "name": account.map(|a| a.name.as_str()),
                "kind": account.map(|a| a.kind.as_str()),
                "type": account.map(|a| a.account_type.as_str()),
                "archivedOn": account.and_then(|a| a.archived_on.as_deref()),
                "points": points,
            })
        })
        .collect();

    ok_json(&json!({
        "start_date": start,
        "end_date": end,
        "accounts": accounts,
        "note": "Points are recorded only when a value changes; a value holds until the next point.",
    }))
}

// ---- Aggregated net worth series (from the browser store) ----

const NET_WORTH_SERIES_JS: &str = r#"
    const callback = arguments[arguments.length - 1];
    const started = Date.now();
    let store = null;
    (function poll() {
        try {
            if (!store) {
                const app = document.querySelector('#app');
                const pinia = app && app.__vue_app__ && app.__vue_app__.config.globalProperties.$pinia;
                store = pinia ? pinia._s.get('net-worth-series') : null;
                if (!store) {
                    if (Date.now() - started > 20000) {
                        callback({"__error": "ProjectionLab's net-worth-series store was not found (the app may still be loading, or it changed)."});
                    } else {
                        setTimeout(poll, 200);
                    }
                    return;
                }
                store.ensureSeriesLoaded();
            }
            if (store.status === 'error') {
                callback({"__error": "ProjectionLab failed to load net worth history"});
            } else if (store.hasLoadedSeries && store.status === 'ready') {
                callback({
                    settled: store.isSeriesSettled,
                    shards: JSON.parse(JSON.stringify(store.seriesShardsByYear || {})),
                });
            } else if (Date.now() - started > 40000) {
                callback({"__error": "Timed out waiting for net worth history to load"});
            } else {
                setTimeout(poll, 250);
            }
        } catch (e) {
            callback({"__error": e.message || String(e)});
        }
    })();
"#;

/// Groups of per-bucket asset series within a shard.
const ASSET_GROUPS: &[&str] = &["assets", "liquidAssets", "investableAssets"];
/// Scalar (date → amount) liability series within a shard.
const DEBT_SERIES: &[&str] = &["otherDebt", "realAssetLoans", "liquidDebt", "investableRealAssetLoans"];

/// One point of the reconstructed net worth series.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NetWorthPoint {
    /// ISO date (YYYY-MM-DD)
    pub date: String,
    pub net_worth: f64,
    pub liquid_net_worth: f64,
    pub investable_assets: f64,
    /// Cash / savings accounts
    pub savings: f64,
    pub taxable: f64,
    pub tax_deferred: f64,
    pub tax_free: f64,
    pub crypto: f64,
    /// Real assets (homes, cars, ...)
    pub assets: f64,
    /// Debt not secured by a real asset
    pub debt: f64,
    /// Loans secured by real assets (mortgages, auto loans)
    pub loans: f64,
    /// Any asset buckets not covered by the fields above
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub other_buckets: BTreeMap<String, f64>,
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn collect_series(target: &mut BTreeMap<String, BTreeMap<String, f64>>, key: String, series: &JsonValue) {
    if let Some(points) = series.as_object() {
        let entry = target.entry(key).or_default();
        for (date, v) in points {
            if let Some(v) = v.as_f64() {
                entry.insert(date.clone(), v);
            }
        }
    }
}

/// Rebuild a dense net worth series from ProjectionLab's sparse yearly shards.
/// Each sub-series only records dates where it changed, so values carry forward.
pub fn compute_net_worth_series(shards: &JsonValue) -> Vec<NetWorthPoint> {
    let mut series: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();
    for shard in shards.as_object().into_iter().flat_map(|m| m.values()) {
        for group in ASSET_GROUPS {
            if let Some(buckets) = shard.get(*group).and_then(JsonValue::as_object) {
                for (bucket, points) in buckets {
                    collect_series(&mut series, format!("{}.{}", group, bucket), points);
                }
            }
        }
        for key in DEBT_SERIES {
            if let Some(points) = shard.get(*key) {
                collect_series(&mut series, (*key).to_string(), points);
            }
        }
    }

    let dates: BTreeSet<&String> = series.values().flat_map(|s| s.keys()).collect();
    let mut current: BTreeMap<&str, f64> = BTreeMap::new();
    let mut out = Vec::with_capacity(dates.len());

    for date in dates {
        for (key, points) in &series {
            if let Some(v) = points.get(date) {
                current.insert(key.as_str(), *v);
            }
        }
        let get = |k: &str| current.get(k).copied().unwrap_or(0.0);
        let sum_group = |group: &str| -> f64 {
            let prefix = format!("{}.", group);
            current
                .iter()
                .filter(|(k, _)| k.starts_with(&prefix))
                .map(|(_, v)| *v)
                .sum()
        };

        let mut other_buckets = BTreeMap::new();
        for (k, v) in &current {
            if let Some(bucket) = k.strip_prefix("assets.") {
                if !["cash", "taxable", "taxDeferred", "taxFree", "crypto", "realAssets"].contains(&bucket) {
                    other_buckets.insert(bucket.to_string(), round2(*v));
                }
            }
        }

        let total_assets = sum_group("assets");
        out.push(NetWorthPoint {
            date: date.clone(),
            net_worth: round2(total_assets - get("otherDebt") - get("realAssetLoans")),
            liquid_net_worth: round2(sum_group("liquidAssets") - get("liquidDebt")),
            investable_assets: round2(sum_group("investableAssets")),
            savings: round2(get("assets.cash")),
            taxable: round2(get("assets.taxable")),
            tax_deferred: round2(get("assets.taxDeferred")),
            tax_free: round2(get("assets.taxFree")),
            crypto: round2(get("assets.crypto")),
            assets: round2(get("assets.realAssets")),
            debt: round2(get("otherDebt")),
            loans: round2(get("realAssetLoans")),
            other_buckets,
        });
    }
    out
}

/// Load ProjectionLab's net worth history and rebuild it as dense points.
/// Returns the points and whether the series has caught up with the latest edits.
pub async fn load_net_worth_series(sync: &SyncManager) -> anyhow::Result<(Vec<NetWorthPoint>, bool)> {
    let result = sync.run_js_async(NET_WORTH_SERIES_JS).await?;
    let settled = result.get("settled").and_then(JsonValue::as_bool).unwrap_or(true);
    let shards = result.get("shards").cloned().unwrap_or(JsonValue::Null);
    Ok((compute_net_worth_series(&shards), settled))
}

pub async fn net_worth_history(sync: &SyncManager, params: NetWorthHistoryParams) -> ToolResult {
    validate_iso_dates(&[&params.start_date, &params.end_date])?;
    let (points, settled) = load_net_worth_series(sync).await.map_err(anyhow_err)?;
    let filtered: Vec<&NetWorthPoint> = points
        .iter()
        .filter(|p| params.start_date.as_deref().is_none_or(|s| p.date.as_str() >= s))
        .filter(|p| params.end_date.as_deref().is_none_or(|e| p.date.as_str() <= e))
        .collect();

    ok_json(&json!({
        "source": "ProjectionLab net worth history (recorded automatically on value changes)",
        "settled": settled,
        "total_data_points": filtered.len(),
        "latest": points.last(),
        "data": filtered,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn net_worth_series_carries_values_forward_across_years() {
        let shards = json!({
            "2025": {
                "assets": {"cash": {"2025-12-01": 100.0}, "taxable": {"2025-11-01": 1000.0}},
                "liquidAssets": {"cash": {"2025-12-01": 100.0}},
                "otherDebt": {"2025-11-01": 50.0}
            },
            "2026": {
                "assets": {"cash": {"2026-01-15": 150.0}, "realAssets": {"2026-02-01": 5000.0}},
                "realAssetLoans": {"2026-02-01": 4000.0},
                "schema": 5
            }
        });
        let points = compute_net_worth_series(&shards);
        let dates: Vec<&str> = points.iter().map(|p| p.date.as_str()).collect();
        assert_eq!(dates, ["2025-11-01", "2025-12-01", "2026-01-15", "2026-02-01"]);

        // 2025-11-01: taxable 1000, debt 50
        assert_eq!(points[0].net_worth, 950.0);
        // 2025-12-01: + cash 100
        assert_eq!(points[1].net_worth, 1050.0);
        assert_eq!(points[1].liquid_net_worth, 100.0);
        // 2026-01-15: cash 150 (taxable carried forward)
        assert_eq!(points[2].net_worth, 1100.0);
        assert_eq!(points[2].taxable, 1000.0);
        // 2026-02-01: + home 5000 − mortgage 4000
        assert_eq!(points[3].net_worth, 2100.0);
        assert_eq!(points[3].assets, 5000.0);
        assert_eq!(points[3].loans, 4000.0);
    }

    #[test]
    fn unknown_asset_buckets_are_reported() {
        let shards = json!({"2026": {"assets": {"collectibles": {"2026-01-01": 7.0}}}});
        let points = compute_net_worth_series(&shards);
        assert_eq!(points[0].net_worth, 7.0);
        assert_eq!(points[0].other_buckets.get("collectibles"), Some(&7.0));
    }

    #[test]
    fn empty_shards_yield_no_points() {
        assert!(compute_net_worth_series(&json!({})).is_empty());
        assert!(compute_net_worth_series(&JsonValue::Null).is_empty());
    }
}
