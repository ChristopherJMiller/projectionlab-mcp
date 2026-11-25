# projectionlab-mcp

MCP server (Rust, stdio) that drives a logged-in ProjectionLab web session through Firefox/GeckoDriver and exposes its Plugin API and app internals as tools. Goal: be the most faithful, complete reflection of ProjectionLab — keep up with its changes and expose its features.

## Build & test

- `nix develop -c cargo build` (if `nix develop` hangs, `nix-shell -p gcc pkg-config openssl --run 'cargo build'` works with the system cargo)
- `cargo test` — unit/model tests; also checks `debug-data/full_export.json` (gitignored) parses and round-trips when present
- Live binaries (need a valid browser session; profile at `~/.local/share/projectionlab-mcp/firefox-profile`, only one Firefox may use it at a time — stop a running MCP server first):
  - `cargo run --bin drift-check` — compare the live app with `compat/projectionlab.json`
  - `cargo run --bin export-data` — dump a fresh export to `debug-data/`
  - `cargo run --bin test-tools` — integration test of the tool handlers
  - `cargo run --bin call-tool -- raw <method> '[args]'` — ad-hoc Plugin API calls

## Invariants (learned the hard way)

- **Models are thin.** Only identity fields are typed; everything else is a flattened pass-through map. Don't add required fields — ProjectionLab changes its schema often and a strict model turns any change into a total outage.
- **Writes edit raw export JSON** (`tools/doc.rs`), never typed structs, so unknown fields survive.
- **Plugin API arguments must be built in the page's realm** (`window.JSON.parse` in `browser.rs`). Objects created in WebDriver's sandbox are rejected ("Account values must be an object", "Failed to admit plan document").
- **Every write waits for ProjectionLab to persist it** (`flush_pending_saves` → `appDataSaving.savePendingChanges`). The app commits to memory immediately and saves later; closing the browser early loses writes.
- **Verify structural writes.** ProjectionLab normalizes on commit and can silently drop things (e.g. a second `debt` expense for the same `debtId`). Creates/deletes re-read the export and fail loudly.
- `restorePlans` replaces *all* plans (omitted plans are deleted). `restoreCurrentFinances` *merges* accounts (it cannot delete); removal = archive (`archivedOn`).
- Balance changes go through `updateAccountValues`, which records history.
- Simulation tools select plans via the plan store (`selectPlan`) and restore the previous selection; routes no longer carry plan ids.
- The server instructions (`INSTRUCTIONS` in `server.rs`) and the README tool table must list exactly the registered tools — tests enforce this.

## Live-testing safety

The live account holds real financial data and its history is permanent.
- Do plan writes only inside temporary plans named `__mcp_test_*` and delete them (test-tools does this).
- Never change real Current Finances values to test, even temporarily: every value change records a history point. Use no-op writes (same value) or `updateAccountValues` with `values: {}`.
- Never commit anything from `debug-data/`, `example.json`, or values from the user's data. `compat/projectionlab.json` must contain structure only (field names, routes, versions) — review its diff.

## Keeping up with ProjectionLab

Signals that ProjectionLab changed:
1. The GitHub Action `ProjectionLab version watch` opens an issue when `https://app.projectionlab.com/version-registry.json` lists a version newer than `compat/projectionlab.json` (`hasBreakingChanges` and per-document schema versions are included).
2. At startup the server runs the compatibility check and logs a warning/error; the `compatibility_check` tool returns the full report.
3. Tools fail; parse errors name the exact JSON path.

Workflow:
1. `cargo run --bin drift-check` — triage `errors` (broken), `warnings`, `new_features` (new routes / Plugin API methods / export documents), and `export_shape_diff`.
2. `cargo run --bin export-data` + `cargo test` — parse and round-trip.
3. Investigate changes in the live app (see below), update models/tools/`schema_help`, and consider exposing new features.
4. `cargo run --bin test-tools`.
5. `cargo run --bin drift-check -- --update-baseline`, review the diff (structure only), commit.

### Investigating the app

- The Plugin API is `window.projectionlabPluginAPI`; `fn.toString()` shows each method's (minified) source.
- Pinia stores: `document.querySelector('#app').__vue_app__.config.globalProperties.$pinia._s` (a Map by store id); routes: `.$router.getRoutes()`.
- To read internals, download the JS bundles: the entry is `assets/index-*.js` (from `document.scripts`) plus `link[rel=modulepreload]` chunks. The resource-timing buffer caps at 250 entries, so don't rely on `performance.getEntriesByType`. The Plugin API store (`hn("plugin-api", …)`) lived in `notification-*.js` as of 5.0.0; search for error strings such as `Plugin Operation Failed`.
- `run_js_in_browser` works for quick checks from an MCP client.

## Coverage

Exposed: Plugin API v2 (all methods), Current Finances (accounts/debts/real assets, archive), household, plans, milestones, all plan event sections, plan accounts, account + net worth history, simulation results (summary, year snapshot/range), Monte Carlo results when present.

Not yet exposed (app routes as of 5.0.0): `/optimize/*` (Roth conversions, drawdown, tax strategy, gain harvesting, flexible spending, comparison), `/tax-analytics`, `/cash-flow`, `/estate`, `/reports`, plan exploration / baseline selection, `/net-worth/history/import`. Monte Carlo can't be started programmatically yet (`requestMonteCarloRerun` alone doesn't run it).
