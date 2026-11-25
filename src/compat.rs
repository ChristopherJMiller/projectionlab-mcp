//! Compatibility and drift detection against the live ProjectionLab app.
//!
//! The server depends on several ProjectionLab surfaces that can change
//! without notice: the Plugin API, the export envelope and document schemas,
//! a handful of Pinia stores (simulation results, net worth history, saving),
//! and app routes. This module captures those surfaces from the running app,
//! checks them against what the server needs, and diffs them against the
//! committed baseline in `compat/projectionlab.json` so changes (including new
//! features) are noticed early.

use crate::models::parse_export;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Baseline captured from the ProjectionLab version this server was last updated for.
pub const BASELINE_JSON: &str = include_str!("../compat/projectionlab.json");

/// The export envelope schema this server understands.
pub const SUPPORTED_ENVELOPE_SCHEMA: i64 = 1;

/// Plugin API methods the server calls.
pub const REQUIRED_PLUGIN_METHODS: &[&str] = &[
    "exportData",
    "getAccounts",
    "updateAccount",
    "updateAccountValues",
    "restoreCurrentFinances",
    "restorePlans",
    "restoreSettings",
    "validateApiKey",
];

/// Pinia store members the server reads or calls, by store id.
pub const REQUIRED_STORE_MEMBERS: &[(&str, &[&str])] = &[
    ("plan", &["plans", "planId", "selectPlan", "simulationRun"]),
    ("worker", &["simulationState", "monteCarloState"]),
    ("monte-carlo", &["montecarlo"]),
    (
        "net-worth-series",
        &["ensureSeriesLoaded", "seriesShardsByYear", "hasLoadedSeries", "status", "isSeriesSettled"],
    ),
    (
        "appDataSaving",
        &["savePendingChanges", "hasUnsavedChanges", "saveError", "dirtyDocuments"],
    ),
];

/// App routes the server navigates to.
pub const REQUIRED_ROUTES: &[&str] = &["/plan/overview", "/settings/plugins"];

/// Fields of a simulated year the simulation tools read.
pub const REQUIRED_SIMULATION_YEAR_FIELDS: &[&str] =
    &["age", "calendarYear", "isSimulatedYear", "summary", "endingValues"];

/// Snapshot of what the server last verified against.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Baseline {
    /// ProjectionLab app version (from the meta store / version registry)
    pub app_version: String,
    /// Document schema versions for that app version (from the version registry)
    pub schemas: BTreeMap<String, f64>,
    pub envelope_schema: i64,
    pub plugin_api_version: i64,
    pub plugin_api_methods: Vec<String>,
    /// All app route paths; new ones usually mean new features.
    pub routes: Vec<String>,
    /// Normalized export field paths → observed JSON types (no values, ids, or dates).
    pub export_shape: BTreeMap<String, String>,
}

impl Baseline {
    pub fn bundled() -> Baseline {
        serde_json::from_str(BASELINE_JSON).expect("compat/projectionlab.json is valid")
    }
}

/// JavaScript that captures the live app surface (run with execute_async).
pub const APP_SURFACE_JS: &str = r#"
    const callback = arguments[arguments.length - 1];
    const required = __REQUIRED__;
    (async () => {
        const sleep = ms => new Promise(r => setTimeout(r, ms));
        let gp = null;
        for (const t0 = Date.now(); Date.now() - t0 < 20000; await sleep(200)) {
            const app = document.querySelector('#app');
            gp = app && app.__vue_app__ && app.__vue_app__.config.globalProperties;
            if (gp && gp.$pinia && window.projectionlabPluginAPI) break;
        }
        const out = { stores: {}, simulationYearFields: null };
        const api = window.projectionlabPluginAPI;
        out.pluginApi = api ? {
            version: api.version,
            methods: Object.keys(api).filter(k => typeof api[k] === 'function').sort(),
        } : null;
        if (gp) {
            const pinia = gp.$pinia;
            out.appVersion = pinia.state.value.meta?.meta?.version ?? null;
            out.routes = [...new Set(gp.$router.getRoutes().map(r => r.path))].sort();
            for (const [id, members] of Object.entries(required)) {
                const store = pinia._s.get(id);
                out.stores[id] = store
                    ? { present: true, missing: members.filter(m => typeof store[m] === 'undefined') }
                    : { present: false, missing: members };
            }
            const run = pinia._s.get('plan')?.simulationRun;
            const year = run && Array.isArray(run.data) ? (run.data.find(y => y.isSimulatedYear) || run.data[0]) : null;
            if (year) out.simulationYearFields = Object.keys(year);
        }
        try {
            const res = await fetch('/version-registry.json', { cache: 'no-store' });
            out.versionRegistry = res.ok ? await res.json() : null;
        } catch (e) {
            out.versionRegistry = null;
        }
        return out;
    })().then(r => callback(r), e => callback({"__error": (e && e.message) || String(e)}));
"#;

/// Build the surface-capture script with the required store members embedded.
pub fn app_surface_script() -> String {
    let required: BTreeMap<&str, &[&str]> = REQUIRED_STORE_MEMBERS.iter().copied().collect();
    APP_SURFACE_JS.replace(
        "__REQUIRED__",
        &serde_json::to_string(&required).expect("serializable"),
    )
}

// ---- Export shape ----

/// Is this object key an identifier, date, or year rather than a field name?
fn is_id_like(key: &str) -> bool {
    let is_hex_uuid = key.len() == 36
        && key.char_indices().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        });
    let all_digits = !key.is_empty() && key.chars().all(|c| c.is_ascii_digit());
    let is_date = key.len() == 10
        && key.char_indices().all(|(i, c)| match i {
            4 | 7 => c == '-',
            _ => c.is_ascii_digit(),
        });
    // prefix_1234567890 style ids (acc_…, plan_…, ms_…)
    let prefixed = key.split_once('_').is_some_and(|(p, n)| {
        !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase()) && n.len() >= 10 && n.chars().all(|c| c.is_ascii_digit())
    });
    // namespace:<uuid> style ids (goal:…)
    let namespaced = key.split_once(':').is_some_and(|(_, rest)| is_id_like(rest));
    is_hex_uuid || all_digits || is_date || prefixed || namespaced
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn walk_shape(v: &Value, path: &str, out: &mut BTreeMap<String, BTreeSet<&'static str>>) {
    if !path.is_empty() {
        out.entry(path.to_string()).or_default().insert(type_name(v));
    }
    let join = |seg: &str| if path.is_empty() { seg.to_string() } else { format!("{}.{}", path, seg) };
    match v {
        Value::Array(items) => {
            for item in items {
                walk_shape(item, &join("[]"), out);
            }
        }
        Value::Object(map) => {
            for (k, child) in map {
                let seg = if is_id_like(k) { "{id}" } else { k.as_str() };
                walk_shape(child, &join(seg), out);
            }
        }
        _ => {}
    }
}

/// Normalized field paths of an export (values, ids, and dates are never included).
pub fn export_shape(export: &Value) -> BTreeMap<String, String> {
    let mut raw = BTreeMap::new();
    walk_shape(export, "", &mut raw);
    raw.into_iter()
        .map(|(k, types)| (k, types.into_iter().collect::<Vec<_>>().join("|")))
        .collect()
}

// ---- Report ----

#[derive(Debug, Default, Serialize)]
pub struct ShapeDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub type_changed: Vec<String>,
}

impl ShapeDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.type_changed.is_empty()
    }
}

/// Diff two shapes. Fields under a newly-seen parent are collapsed to that parent.
pub fn diff_shapes(baseline: &BTreeMap<String, String>, current: &BTreeMap<String, String>) -> ShapeDiff {
    let collapse = |paths: Vec<&String>, other: &BTreeMap<String, String>| -> Vec<String> {
        paths
            .iter()
            .filter(|p| {
                // keep only the top-most new/removed path
                !p.rmatch_indices('.')
                    .any(|(i, _)| !other.contains_key(&p[..i]) && paths.iter().any(|q| q.as_str() == &p[..i]))
            })
            .map(|p| p.to_string())
            .collect()
    };
    let added: Vec<&String> = current.keys().filter(|k| !baseline.contains_key(*k)).collect();
    let removed: Vec<&String> = baseline.keys().filter(|k| !current.contains_key(*k)).collect();
    ShapeDiff {
        added: collapse(added, baseline),
        removed: collapse(removed, current),
        type_changed: current
            .iter()
            .filter_map(|(k, t)| {
                baseline
                    .get(k)
                    .filter(|bt| *bt != t)
                    .map(|bt| format!("{}: {} -> {}", k, bt, t))
            })
            .collect(),
    }
}

/// Result of a compatibility check.
#[derive(Debug, Default, Serialize)]
pub struct Report {
    /// Overall status: "ok", "drift" (changes noticed, nothing required is broken), or "broken".
    pub status: &'static str,
    pub baseline_app_version: String,
    pub app_version: Option<String>,
    pub latest_registry_version: Option<String>,
    /// Required surfaces that are missing or incompatible: tools will fail.
    pub errors: Vec<String>,
    /// Changes likely to need attention (new app versions, removed fields).
    pub warnings: Vec<String>,
    /// New capabilities worth exposing (new routes, plugin methods, fields).
    pub new_features: Vec<String>,
    pub export_shape_diff: ShapeDiff,
}

fn version_key(v: &str) -> Vec<u64> {
    v.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// Compare the live surface and export with what the server needs and the baseline.
pub fn check(baseline: &Baseline, surface: &Value, export: Option<&Value>) -> Report {
    let mut r = Report {
        baseline_app_version: baseline.app_version.clone(),
        app_version: surface.get("appVersion").and_then(Value::as_str).map(str::to_string),
        ..Default::default()
    };

    // Plugin API
    match surface.get("pluginApi").filter(|v| v.is_object()) {
        None => r.errors.push("window.projectionlabPluginAPI is not available (are plugins enabled?)".into()),
        Some(api) => {
            let version = api.get("version").and_then(Value::as_i64);
            if version != Some(baseline.plugin_api_version) {
                r.errors.push(format!(
                    "Plugin API version is {:?}; the server was built for {}",
                    version, baseline.plugin_api_version
                ));
            }
            let methods: BTreeSet<&str> = api
                .get("methods")
                .and_then(Value::as_array)
                .map(|m| m.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            for m in REQUIRED_PLUGIN_METHODS {
                if !methods.contains(m) {
                    r.errors.push(format!("Plugin API method `{}` is missing", m));
                }
            }
            for m in &methods {
                if !baseline.plugin_api_methods.iter().any(|b| b == m) {
                    r.new_features.push(format!("New Plugin API method `{}`", m));
                }
            }
        }
    }

    // Stores
    for (id, _) in REQUIRED_STORE_MEMBERS {
        let store = surface.pointer(&format!("/stores/{}", id));
        match store {
            Some(s) if s.get("present") == Some(&json!(true)) => {
                let missing: Vec<&str> = s
                    .get("missing")
                    .and_then(Value::as_array)
                    .map(|m| m.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                if !missing.is_empty() {
                    r.errors.push(format!("App store `{}` is missing {}", id, missing.join(", ")));
                }
            }
            _ => r.errors.push(format!("App store `{}` not found", id)),
        }
    }

    // Simulation result shape (only checkable once a simulation has run)
    if let Some(fields) = surface.get("simulationYearFields").and_then(Value::as_array) {
        let fields: BTreeSet<&str> = fields.iter().filter_map(Value::as_str).collect();
        let missing: Vec<&&str> = REQUIRED_SIMULATION_YEAR_FIELDS.iter().filter(|f| !fields.contains(**f)).collect();
        if !missing.is_empty() {
            r.errors.push(format!("Simulation years are missing fields: {:?}", missing));
        }
    }

    // Routes
    let routes: BTreeSet<&str> = surface
        .get("routes")
        .and_then(Value::as_array)
        .map(|m| m.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if !routes.is_empty() {
        for req in REQUIRED_ROUTES {
            if !routes.contains(req) {
                r.errors.push(format!("App route `{}` no longer exists", req));
            }
        }
        let known: BTreeSet<&str> = baseline.routes.iter().map(String::as_str).collect();
        for route in routes.difference(&known) {
            r.new_features.push(format!("New app route `{}`", route));
        }
        for route in known.difference(&routes) {
            r.warnings.push(format!("App route `{}` was removed", route));
        }
    }

    // Version registry
    if let Some(registry) = surface.get("versionRegistry").and_then(Value::as_object) {
        let mut versions: Vec<&String> = registry.keys().collect();
        versions.sort_by_key(|v| version_key(v));
        r.latest_registry_version = versions.last().map(|v| v.to_string());
        for v in versions.iter().filter(|v| version_key(v) > version_key(&baseline.app_version)) {
            let entry = &registry[v.as_str()];
            let breaking = entry.get("hasBreakingChanges").and_then(Value::as_bool).unwrap_or(false);
            r.warnings.push(format!(
                "ProjectionLab {} is newer than the baseline {}{} (schemas: {})",
                v,
                baseline.app_version,
                if breaking { " and marked as having BREAKING changes" } else { "" },
                entry.get("schemas").cloned().unwrap_or(Value::Null)
            ));
        }
    }
    if let Some(v) = &r.app_version {
        if version_key(v) > version_key(&baseline.app_version) {
            r.warnings.push(format!(
                "The app reports version {} (baseline {}); run drift-check and review the changes",
                v, baseline.app_version
            ));
        }
    }

    // Export
    if let Some(export) = export {
        if let Err(e) = parse_export(export) {
            r.errors.push(format!("{:#}", e));
        }
        let envelope = export.get("schema").and_then(Value::as_i64);
        if envelope != Some(SUPPORTED_ENVELOPE_SCHEMA) {
            r.errors.push(format!(
                "Export envelope schema is {:?}; supported is {}",
                envelope, SUPPORTED_ENVELOPE_SCHEMA
            ));
        }
        let docs = export.get("documents").and_then(Value::as_object);
        for (doc, values) in document_schemas(export) {
            let supported = baseline.schemas.get(&doc).copied();
            for s in values {
                if supported.is_some_and(|sup| s > sup) {
                    r.errors.push(format!(
                        "`{}` document schema {} is newer than supported {}",
                        doc,
                        s,
                        supported.unwrap_or_default()
                    ));
                }
            }
        }
        if let Some(docs) = docs {
            for k in docs.keys() {
                if !["currentFinances", "plans", "settings", "accountHistory"].contains(&k.as_str()) {
                    r.new_features.push(format!("New export document `{}`", k));
                }
            }
        }
        r.export_shape_diff = diff_shapes(&baseline.export_shape, &export_shape(export));
        if !r.export_shape_diff.removed.is_empty() || !r.export_shape_diff.type_changed.is_empty() {
            r.warnings.push(
                "Export fields were removed or changed type (may also reflect changes in your own data); see export_shape_diff".into(),
            );
        }
    }

    r.status = if !r.errors.is_empty() {
        "broken"
    } else if !r.warnings.is_empty() || !r.new_features.is_empty() || !r.export_shape_diff.is_empty() {
        "drift"
    } else {
        "ok"
    };
    r
}

/// Schema versions found on each document in an export.
fn document_schemas(export: &Value) -> BTreeMap<String, BTreeSet<OrdF64>> {
    let mut out: BTreeMap<String, BTreeSet<OrdF64>> = BTreeMap::new();
    let mut add = |doc: &str, v: Option<f64>| {
        if let Some(v) = v {
            out.entry(doc.to_string()).or_default().insert(OrdF64(v));
        }
    };
    let docs = match export.get("documents") {
        Some(d) => d,
        None => return out,
    };
    add("currentFinances", docs.pointer("/currentFinances/schema").and_then(Value::as_f64));
    add("settings", docs.pointer("/settings/schema").and_then(Value::as_f64));
    for plan in docs.get("plans").and_then(Value::as_array).into_iter().flatten() {
        add("plan", plan.get("schema").and_then(Value::as_f64));
    }
    for history in docs.get("accountHistory").and_then(Value::as_object).into_iter().flat_map(|m| m.values()) {
        for year in history.as_object().into_iter().flat_map(|m| m.values()) {
            add("accountHistory", year.get("schema").and_then(Value::as_f64));
        }
    }
    out
}

/// f64 wrapper with a total order (schema versions are small finite numbers).
#[derive(Debug, Clone, Copy, PartialEq)]
struct OrdF64(f64);
impl Eq for OrdF64 {}
impl PartialOrd for OrdF64 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for OrdF64 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}
impl std::fmt::Display for OrdF64 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl PartialOrd<f64> for OrdF64 {
    fn partial_cmp(&self, other: &f64) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}
impl PartialEq<f64> for OrdF64 {
    fn eq(&self, other: &f64) -> bool {
        self.0 == *other
    }
}

/// Capture the live surface through the server's browser session and check it.
pub async fn run(sync: &crate::sync::SyncManager) -> anyhow::Result<Report> {
    let surface = sync.run_js_async(&app_surface_script()).await?;
    let baseline = Baseline::bundled();
    Ok(match sync.export_unparsed().await {
        Ok(export) => check(&baseline, &surface, Some(&export)),
        Err(e) => {
            let mut report = check(&baseline, &surface, None);
            report.errors.push(format!("exportData failed: {:#}", e));
            report.status = "broken";
            report
        }
    })
}

/// Build a new baseline from the live surface and export.
pub fn baseline_from(surface: &Value, export: &Value) -> anyhow::Result<Baseline> {
    let app_version = surface
        .get("appVersion")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("app version not available"))?
        .to_string();
    let schemas = surface
        .pointer(&format!("/versionRegistry/{}/schemas", app_version.replace('/', "~1")))
        .and_then(Value::as_object)
        .map(|m| m.iter().filter_map(|(k, v)| v.as_f64().map(|v| (k.clone(), v))).collect())
        .ok_or_else(|| anyhow::anyhow!("version registry has no entry for {}", app_version))?;
    let strings = |ptr: &str| -> Vec<String> {
        surface
            .pointer(ptr)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default()
    };
    Ok(Baseline {
        app_version,
        schemas,
        envelope_schema: export.get("schema").and_then(Value::as_i64).unwrap_or_default(),
        plugin_api_version: surface.pointer("/pluginApi/version").and_then(Value::as_i64).unwrap_or_default(),
        plugin_api_methods: strings("/pluginApi/methods"),
        routes: strings("/routes"),
        export_shape: export_shape(export),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline() -> Baseline {
        Baseline {
            app_version: "5.0.0".into(),
            schemas: [("plan".to_string(), 5.0), ("currentFinances".to_string(), 5.0)].into(),
            envelope_schema: 1,
            plugin_api_version: 2,
            plugin_api_methods: REQUIRED_PLUGIN_METHODS.iter().map(|s| s.to_string()).collect(),
            routes: vec!["/plan/overview".into(), "/settings/plugins".into()],
            export_shape: BTreeMap::new(),
        }
    }

    fn surface() -> Value {
        let stores: serde_json::Map<String, Value> = REQUIRED_STORE_MEMBERS
            .iter()
            .map(|(id, _)| (id.to_string(), json!({"present": true, "missing": []})))
            .collect();
        json!({
            "appVersion": "5.0.0",
            "pluginApi": {"version": 2, "methods": REQUIRED_PLUGIN_METHODS},
            "stores": stores,
            "routes": ["/plan/overview", "/settings/plugins"],
            "simulationYearFields": REQUIRED_SIMULATION_YEAR_FIELDS,
            "versionRegistry": {"5.0.0": {"hasBreakingChanges": true, "schemas": {"plan": 5}}}
        })
    }

    #[test]
    fn matching_surface_is_ok() {
        let r = check(&baseline(), &surface(), None);
        assert_eq!(r.status, "ok", "{:?}", r);
    }

    #[test]
    fn missing_plugin_method_is_broken() {
        let mut s = surface();
        s["pluginApi"]["methods"] = json!(["exportData"]);
        let r = check(&baseline(), &s, None);
        assert_eq!(r.status, "broken");
        assert!(r.errors.iter().any(|e| e.contains("updateAccountValues")));
    }

    #[test]
    fn missing_store_member_is_broken() {
        let mut s = surface();
        s["stores"]["plan"] = json!({"present": true, "missing": ["simulationRun"]});
        let r = check(&baseline(), &s, None);
        assert!(r.errors.iter().any(|e| e.contains("simulationRun")));
    }

    #[test]
    fn new_routes_and_methods_are_features() {
        let mut s = surface();
        s["routes"] = json!(["/plan/overview", "/settings/plugins", "/optimize/new-thing"]);
        s["pluginApi"]["methods"].as_array_mut().unwrap().push(json!("getPlans"));
        let r = check(&baseline(), &s, None);
        assert_eq!(r.status, "drift");
        assert!(r.new_features.iter().any(|f| f.contains("/optimize/new-thing")));
        assert!(r.new_features.iter().any(|f| f.contains("getPlans")));
    }

    #[test]
    fn newer_breaking_version_warns() {
        let mut s = surface();
        s["versionRegistry"]["5.1.0"] = json!({"hasBreakingChanges": true, "schemas": {"plan": 5.1}});
        let r = check(&baseline(), &s, None);
        assert_eq!(r.latest_registry_version.as_deref(), Some("5.1.0"));
        assert!(r.warnings.iter().any(|w| w.contains("5.1.0") && w.contains("BREAKING")));
    }

    #[test]
    fn newer_document_schema_is_broken() {
        let export = json!({
            "schema": 1, "exportedAt": 0,
            "documents": {"currentFinances": {"schema": 5, "accounts": {}}, "plans": [{"id": "p", "schema": 6}], "settings": {}}
        });
        let r = check(&baseline(), &surface(), Some(&export));
        assert!(r.errors.iter().any(|e| e.contains("`plan` document schema 6")), "{:?}", r.errors);
    }

    #[test]
    fn export_shape_normalizes_ids_dates_and_arrays() {
        let export = json!({
            "documents": {
                "currentFinances": {"accounts": {"acc_1775521218006": {"balance": 1}, "26f741dd-a2cb-4cd8-be41-590487e4bcc4": {"balance": 2, "costBasis": 1}}},
                "accountHistory": {"debt_1775521532485": {"2026": {"balance": {"2026-01-01": 5}}}},
                "plans": [{"milestones": [{"id": "x"}]}]
            }
        });
        let shape = export_shape(&export);
        assert!(shape.contains_key("documents.currentFinances.accounts.{id}.costBasis"));
        assert!(shape.contains_key("documents.accountHistory.{id}.{id}.balance.{id}"));
        assert!(shape.contains_key("documents.plans.[].milestones.[].id"));
        assert!(!shape.keys().any(|k| k.contains("acc_") || k.contains("2026")));
    }

    #[test]
    fn shape_diff_collapses_new_subtrees() {
        let base: BTreeMap<String, String> = [("a".into(), "object".into())].into();
        let cur: BTreeMap<String, String> = [
            ("a".into(), "object".into()),
            ("a.b".into(), "object".into()),
            ("a.b.c".into(), "number".into()),
        ]
        .into();
        let d = diff_shapes(&base, &cur);
        assert_eq!(d.added, vec!["a.b".to_string()]);
    }

    #[test]
    fn bundled_baseline_parses() {
        let b = Baseline::bundled();
        assert!(!b.app_version.is_empty());
        assert!(!b.export_shape.is_empty());
        for m in REQUIRED_PLUGIN_METHODS {
            assert!(b.plugin_api_methods.iter().any(|x| x == m), "baseline lacks {}", m);
        }
    }
}
