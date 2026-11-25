//! Model tests for the ProjectionLab V5 export format.
//!
//! Inline fixtures keep these runnable anywhere. Tests marked "local" also run
//! against `debug-data/full_export.json` (gitignored, produced by
//! `cargo run --bin export-data`) when it exists, and skip otherwise.

use projectionlab_mcp::models::account_history::flatten_history;
use projectionlab_mcp::models::{parse_export, FullExport};
use serde_json::{json, Value};

fn fixture() -> Value {
    json!({
        "schema": 1,
        "exportedAt": 1795638406435i64,
        "documents": {
            "currentFinances": {
                "schema": 5,
                "lastUpdated": 1795637259733i64,
                "location": {"country": "US", "state": "WA"},
                "household": {
                    "partnerStatus": "married",
                    "people": {
                        "me": {"id": "me", "name": "Alex", "birthYear": 1990, "birthMonth": 6, "character": {"hair": "short"}},
                        "spouse": {"id": "spouse", "name": "Sam", "birthYear": 1991, "birthMonth": 2}
                    }
                },
                "accounts": {
                    "acc_1": {"id": "acc_1", "kind": "account", "type": "savings", "name": "Checking", "owner": "me", "balance": 10000, "order": 0, "icon": "mdi-bank"},
                    "acc_2": {"id": "acc_2", "kind": "account", "type": "taxable", "name": "Brokerage", "owner": "joint", "balance": 50000, "costBasis": 30000, "order": 1},
                    "acc_3": {"id": "acc_3", "kind": "account", "type": "crypto", "name": "Old wallet", "balance": 999, "order": 2, "archivedOn": "2026-01-01"},
                    "home": {"id": "home", "kind": "real-asset", "type": "real-estate", "name": "House", "value": 400000, "purchasePrice": 350000, "order": 0},
                    "mtg": {"id": "mtg", "kind": "debt", "type": "mortgage", "name": "Mortgage", "balance": 300000, "interestRate": 6.5, "monthlyPayment": 2200, "securedByAssetId": "home", "order": 0, "futureField": {"x": 1}}
                }
            },
            "plans": [
                {
                    "id": "plan_a",
                    "name": "Base",
                    "icon": "mdi-home",
                    "schema": 5,
                    "startingConditionsType": "default",
                    "startingConditions": {},
                    "expenses": {"events": [
                        {"id": "e1", "kind": "expense", "type": "living-expenses", "name": "Living", "amount": 4000, "amountType": "today$", "frequency": "monthly", "owner": "joint",
                         "start": {"type": "keyword", "value": "beforeCurrentYear"}, "end": {"type": "milestone", "value": "lifeExpectancy"}, "spendingType": "essential", "yearlyChange": {"type": "match-inflation"}}
                    ]},
                    "income": {"events": []},
                    "priorities": {"events": []},
                    "assets": {"events": []},
                    "accounts": {"events": [
                        {"id": "pa1", "kind": "account", "type": "savings", "name": "Checking", "accountId": "acc_1", "balance": 10000, "liquidity": true}
                    ]},
                    "milestones": [
                        {"id": "retirement", "name": "Retirement", "criteria": [{"type": "age", "value": 60}]},
                        {"id": "ms_1", "name": "Custom", "removable": true, "criteria": [{"type": "year", "value": 2035}]}
                    ],
                    "variables": {"inflation": 3, "investmentReturn": 7, "bondSettings": {"type": "none"}},
                    "withdrawalStrategy": {"enabled": true, "strategy": "initial-%"},
                    "montecarlo": {"trials": 1000},
                    "meta": {"index": 0}
                },
                {
                    "id": "plan_b",
                    "name": "Custom start",
                    "startingConditionsType": "custom",
                    "startingConditions": {"accounts": {"acc_1": {"id": "acc_1", "kind": "account", "type": "savings", "name": "Checking", "balance": 12000}}}
                }
            ],
            "settings": {"schema": 5, "plan": {"activeId": "plan_b", "baselineId": "plan_a"}, "plugins": {"enabled": true, "apiKey": "secret"}, "theme": "dark"},
            "accountHistory": {
                "acc_2": {
                    "2025": {"kind": "account", "schema": 5, "balance": {"2025-06-01": 40000, "2025-12-01": 45000}, "costBasis": {"2025-06-01": 28000}},
                    "2026": {"kind": "account", "schema": 5, "balance": {"2026-03-01": 50000}}
                },
                "home": {"2026": {"kind": "real-asset", "schema": 5, "value": {"2026-01-01": 400000}}}
            }
        }
    })
}

fn parsed() -> FullExport {
    parse_export(&fixture()).expect("fixture parses")
}

#[test]
fn parses_v5_envelope() {
    let data = parsed();
    assert_eq!(data.schema, 1);
    assert_eq!(data.plans().len(), 2);
    assert_eq!(data.current_finances().accounts.len(), 5);
    assert_eq!(data.active_plan_id(), Some("plan_b"));
    assert_eq!(data.baseline_plan_id(), Some("plan_a"));
    let household = data.current_finances().household.as_ref().unwrap();
    assert_eq!(household.people.len(), 2);
    assert_eq!(household.people["me"].birth_year, Some(1990));
}

#[test]
fn net_worth_excludes_archived_and_subtracts_debt() {
    let cf = parsed().documents.current_finances;
    // 10000 + 50000 + 400000 − 300000 (archived crypto excluded)
    assert_eq!(cf.net_worth(), 160000.0);
    assert_eq!(cf.active_accounts().len(), 4);
    assert!(cf.find("acc_3").unwrap().is_archived());
}

#[test]
fn accounts_sort_like_projectionlab() {
    let cf = parsed().documents.current_finances;
    let order: Vec<&str> = cf.sorted_accounts().iter().map(|a| a.id.as_str()).collect();
    // savings, investments, real assets, debts
    assert_eq!(order, ["acc_1", "acc_2", "acc_3", "home", "mtg"]);
}

#[test]
fn value_field_and_buckets() {
    let cf = parsed().documents.current_finances;
    assert_eq!(cf.find("home").unwrap().value_field(), "value");
    assert_eq!(cf.find("mtg").unwrap().value_field(), "balance");
    assert_eq!(cf.find("acc_1").unwrap().bucket(), "cash");
    assert_eq!(cf.find("acc_2").unwrap().bucket(), "taxable");
    assert_eq!(cf.find("home").unwrap().bucket(), "realAssets");
    assert_eq!(cf.find("mtg").unwrap().bucket(), "debt");
}

#[test]
fn unknown_fields_are_preserved() {
    let cf = parsed().documents.current_finances;
    let mtg = cf.find("mtg").unwrap();
    assert_eq!(mtg.extra.get("futureField"), Some(&json!({"x": 1})));
    assert_eq!(mtg.extra.get("interestRate"), Some(&json!(6.5)));
    let reserialized = serde_json::to_value(mtg).unwrap();
    assert_eq!(reserialized["futureField"], json!({"x": 1}));
}

#[test]
fn default_plan_has_empty_starting_conditions() {
    let data = parsed();
    let a = data.find_plan("plan_a").unwrap();
    assert!(!a.uses_custom_starting_conditions());
    assert!(a.starting_conditions.accounts.is_empty());
    let b = data.find_plan("plan_b").unwrap();
    assert!(b.uses_custom_starting_conditions());
    assert_eq!(b.starting_conditions.find("acc_1").unwrap().balance, Some(12000.0));
}

#[test]
fn minimal_plan_parses() {
    // plan_b has no event containers, milestones, or variables at all.
    let data = parsed();
    let b = data.find_plan("plan_b").unwrap();
    assert!(b.expenses.events.is_empty());
    assert!(b.milestones.is_empty());
    assert!(b.variables.is_null());
}

#[test]
fn plan_event_summary_projects_fields() {
    let data = parsed();
    let e = &data.find_plan("plan_a").unwrap().expenses.events[0];
    assert_eq!(e.kind.as_deref(), Some("expense"));
    let s = e.summary(&["amount", "frequency", "missing"]);
    assert_eq!(s, json!({"id": "e1", "name": "Living", "type": "living-expenses", "amount": 4000, "frequency": "monthly"}));
}

#[test]
fn account_history_flattens_across_years() {
    let data = parsed();
    let h = &data.documents.account_history["acc_2"];
    let points = flatten_history(h, None, None);
    let dates: Vec<&str> = points.iter().map(|p| p.date.as_str()).collect();
    assert_eq!(dates, ["2025-06-01", "2025-12-01", "2026-03-01"]);
    assert_eq!(points[0].cost_basis, Some(28000.0));
    assert_eq!(points[2].balance, Some(50000.0));

    let filtered = flatten_history(h, Some("2025-07-01"), Some("2026-12-31"));
    assert_eq!(filtered.len(), 2);

    let home = flatten_history(&data.documents.account_history["home"], None, None);
    assert_eq!(home[0].value, Some(400000.0));
}

#[test]
fn legacy_export_reports_path() {
    let legacy = json!({"meta": {"version": "4.5.0"}, "today": {}, "plans": [], "settings": {}, "progress": {}});
    let e = parse_export(&legacy).unwrap_err().to_string();
    assert!(e.contains("schema"), "error should name the missing field: {}", e);
    assert!(e.contains("top-level keys"), "error should list top-level keys: {}", e);
}

#[test]
fn bad_field_error_names_json_path() {
    let mut v = fixture();
    v["documents"]["plans"][0]["expenses"]["events"][0]["id"] = json!(42);
    let e = parse_export(&v).unwrap_err().to_string();
    assert!(e.contains("documents.plans[0].expenses.events[0].id"), "{}", e);
}

// ---- Local export (skipped when absent) ----

fn local_export() -> Option<Value> {
    let text = std::fs::read_to_string("debug-data/full_export.json").ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    if v.get("documents").is_none() {
        eprintln!("debug-data/full_export.json is not a V5 export; skipping");
        return None;
    }
    Some(v)
}

#[test]
fn local_export_parses() {
    let Some(raw) = local_export() else { return };
    let data = parse_export(&raw).unwrap_or_else(|e| panic!("{:#}", e));
    assert!(!data.plans().is_empty());
}

/// Check that every value in `a` (the raw export) survives in `b` (the typed
/// model re-serialized). Numbers compare by value (5 == 5.0) and a raw `null`
/// may be absent. Keys the typed model adds as empty defaults are ignored.
fn semantically_equal(a: &Value, b: &Value, path: &str, diffs: &mut Vec<String>) {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            if x.as_f64() != y.as_f64() {
                diffs.push(format!("{}: {} != {}", path, x, y));
            }
        }
        (Value::Object(x), Value::Object(y)) => {
            for (k, va) in x {
                let vb = y.get(k).unwrap_or(&Value::Null);
                semantically_equal(va, vb, &format!("{}.{}", path, k), diffs);
            }
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            for (i, (va, vb)) in x.iter().zip(y).enumerate() {
                semantically_equal(va, vb, &format!("{}[{}]", path, i), diffs);
            }
        }
        _ if a == b => {}
        _ => diffs.push(format!("{}: {} != {}", path, a, b)),
    }
}

#[test]
fn local_export_typed_roundtrip_loses_nothing() {
    let Some(raw) = local_export() else { return };
    let data = parse_export(&raw).unwrap();
    let back = serde_json::to_value(&data).unwrap();
    let mut diffs = Vec::new();
    semantically_equal(&raw, &back, "$", &mut diffs);
    diffs.dedup();
    assert!(diffs.is_empty(), "typed round trip changed {} values, e.g.:\n{}", diffs.len(), diffs.iter().take(10).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn inline_fixture_typed_roundtrip_loses_nothing() {
    let raw = fixture();
    let back = serde_json::to_value(parsed()).unwrap();
    let mut diffs = Vec::new();
    semantically_equal(&raw, &back, "$", &mut diffs);
    assert!(diffs.is_empty(), "{:?}", diffs);
}
