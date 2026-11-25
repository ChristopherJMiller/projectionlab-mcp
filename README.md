# ProjectionLab MCP Server

A Model Context Protocol (MCP) server that enables LLM integration with [ProjectionLab](https://projectionlab.com/), a personal finance and retirement planning tool.

This server uses browser automation (Firefox + GeckoDriver) to interact with ProjectionLab's Plugin API, allowing AI assistants to read and update your financial data.

## Features

- **Browser Automation**: Uses thirtyfour (Selenium for Rust) with Firefox to execute JavaScript API calls
- **Secure Authentication**: Manual login via visible browser window, API key automatically retrieved
- **Full API Coverage**: Every ProjectionLab Plugin API v2 method, plus higher-level tools for accounts, plans, events, milestones, history, and simulation results
- **stdio Transport**: Runs as a standard stdio MCP server
- **Nix Integration**: Fully configured flake for easy development and deployment

## Architecture

```
MCP Client (Claude Desktop/CLI)
  ↓ stdio (JSON-RPC)
MCP Server (this project)
  ↓ Browser Automation (thirtyfour)
Firefox (headless; visible only for login)
  ↓ JavaScript Injection
ProjectionLab Web UI (window.projectionlabPluginAPI)
```

## Prerequisites

### NixOS / Nix

If you're using Nix (recommended), all dependencies are handled by the flake:

```bash
nix develop
```

### Manual Installation

If not using Nix, you'll need:

- Rust 1.91+ (via rustup)
- Firefox browser
- GeckoDriver (matching your Firefox version)
- OpenSSL development headers

## Installation

### Using Nix (Recommended)

```bash
# Enter development environment
nix develop

# Build the project
nix build

# Run directly
nix run
```

### Manual Build

```bash
# Build release binary
cargo build --release

# Run
./target/release/projectionlab-mcp
```

## Setup

### 1. Start GeckoDriver

The server expects GeckoDriver to be running on `http://localhost:4444`:

```bash
# In a separate terminal
geckodriver
```

Or if using Nix:

```bash
nix develop -c geckodriver
```

### 2. Enable ProjectionLab Plugins

Before using this server, you must enable plugins in your ProjectionLab account:

1. Log in to [ProjectionLab](https://app.projectionlab.com/)
2. Navigate to Account Settings → Plugins
3. Enable "Community Plugins" support
4. Note your API key (the server will retrieve this automatically)

### 3. Run the MCP Server

The server speaks MCP over **stdio**. `run-mcp.sh` builds quietly and execs the binary, so it can be used directly as an MCP command:

```json
{
  "mcpServers": {
    "projectionlab": {
      "command": "/path/to/projectionlab-mcp/run-mcp.sh"
    }
  }
}
```

On first connection the server launches Firefox with a persistent profile (`~/.local/share/projectionlab-mcp/firefox-profile`). If the saved session is still valid it stays headless; otherwise a visible window opens for you to log in. It then reads your Plugin API key from the plugins settings page and is ready for tool calls.

## ProjectionLab V5 data model

ProjectionLab's late-2026 "V5" release reshaped its data and Plugin API. This server targets V5:

- `exportData` returns `{schema: 1, exportedAt, documents: {currentFinances, plans, settings, accountHistory}}`.
- **Current Finances** keeps every balance-sheet item in one `accounts` map, distinguished by `kind`: `account` (savings, taxable, 401k, roth-ira, hsa, crypto, …), `debt` (mortgage, auto-loan, student-loans, credit-card, …), and `real-asset` (real-estate, car, …).
- Accounts are **archived** (`archivedOn`), not deleted. The plugin API cannot hard-delete them.
- **History is automatic.** Editing a current value records account and net worth history. The old `progress` document and `restoreProgress` are gone.
- Plans no longer have an `active` flag. `settings.plan.activeId` is the plan selected in the UI and `settings.plan.baselineId` is the comparison baseline.
- Plugin API v2 adds `getAccounts` and `updateAccountValues`.

Models are intentionally thin: only identity fields are typed and everything else is passed through untouched. All writes edit the raw export JSON, so fields this server doesn't know about survive round trips, and new upstream fields don't break parsing.

## Available Tools

Call `schema_help` with `topic: "topics"` for field-level docs (`v5_changes` summarizes what changed).

| Area | Tools |
|------|-------|
| Current Finances | `accounts_list`, `accounts_get`, `accounts_create`, `accounts_update`, `accounts_update_balance`, `accounts_archive`, `accounts_unarchive`, `household_get` |
| Real assets | `real_assets_list`, `real_assets_get`, `real_assets_create`, `real_assets_update` (kind-filtered views of the account tools) |
| Debts | `debts_list`, `debts_get`, `debts_create`, `debts_update` (kind-filtered views of the account tools) |
| Plans | `plans_list`, `plans_get`, `plans_create`, `plans_clone`, `plans_delete`, `plans_get_variables`, `plans_update_variables`, `plans_update_metadata`, `plans_get_milestones` |
| Milestones | `milestones_create`, `milestones_update`, `milestones_delete` (built-ins can be edited/hidden, not deleted) |
| Expenses | `expenses_list`, `expenses_create`, `expenses_update`, `expenses_delete` |
| Income | `income_list`, `income_create`, `income_update`, `income_delete` |
| Priorities | `priorities_list`, `priorities_create`, `priorities_update`, `priorities_delete` |
| Planned assets | `plan_assets_list`, `plan_assets_create`, `plan_assets_update`, `plan_assets_delete` |
| Plan accounts | `plan_accounts_list`, `plan_accounts_update` (writes shared fields through to Current Finances) |
| History | `net_worth_history`, `account_history` (ProjectionLab records history automatically when values change) |
| Simulation | `plans_get_simulation_results`, `plans_get_year_snapshot`, `plans_get_year_range`, `plans_get_montecarlo_results` |
| Integration | `sync_account_balances` (batch balance updates, e.g. from Monarch Money) |
| System | `schema_help`, `refresh_cache`, `cache_status`, `validate_api_key`, `run_js_in_browser` |
| Raw Plugin API | `get_accounts`, `update_account`, `update_account_values`, `export_data`, `restore_current_finances`, `restore_plans`, `restore_settings` |

Notes:

- Balance changes go through `updateAccountValues`, so ProjectionLab records them in history. Items are removed by archiving (`accounts_archive`); the plugin API cannot delete them.
- `restore_plans` replaces **all** plans; any plan missing from the array is deleted. Prefer the targeted tools.
- Simulation tools temporarily select the requested plan in the app, wait for a fresh projection, and restore the previously selected plan.
- Monte Carlo results only exist after ProjectionLab's Chance of Success page has run them in the automated browser, so they are usually unavailable.

## Development

```bash
nix develop            # toolchain, Firefox, geckodriver
cargo test             # unit + model tests (no network)
cargo run --bin export-data   # dump a fresh export to debug-data/ (gitignored) and check it parses
cargo run --bin call-tool -- plans          # quick Plugin API probes
cargo run --bin test-tools                  # live integration test (see below)
```

`cargo test` also checks the local `debug-data/full_export.json` when present: it must parse, and the typed model must preserve every value.

`test-tools` exercises the real tool handlers against your account. All create/update/delete steps happen inside a temporary `__mcp_test_<timestamp>` plan that is removed afterwards. Current Finances is only touched with a no-op value write, so no history is recorded.

When ProjectionLab changes again, run `export-data` first. Parse errors report the exact JSON path that failed, e.g. `documents.plans[0].expenses.events[3].owner`.

### Project Structure

```
src/
├── main.rs            # stdio MCP entry point
├── server.rs          # tool definitions (#[tool_router])
├── browser.rs         # Firefox/GeckoDriver session + Plugin API calls
├── sync.rs            # export cache + Plugin API write operations
├── resources.rs       # MCP resources
├── models/            # thin V5 models (current finances, plans, events, history, settings)
└── tools/             # tool handlers; tools/doc.rs holds raw-JSON edit helpers
```

## Troubleshooting

### "Failed to connect to GeckoDriver"

Ensure GeckoDriver is running on port 4444:

```bash
geckodriver --port 4444
```

### "Login timeout: User did not log in within 300 seconds"

The server waits 5 minutes for you to log in. If you need more time, the timeout can be adjusted in `src/browser.rs` (see `LOGIN_WAIT_TIMEOUT_SECS`).

### "Could not find API key on plugins settings page"

The API key extraction uses multiple strategies. If it fails:

1. Ensure you're on the correct page (`/settings/plugins`)
2. Ensure plugins are enabled in your ProjectionLab account
3. Check the page DOM structure hasn't changed (you may need to update selectors)

### Browser doesn't close properly

The browser session is managed by Rust's RAII (Drop trait), but if the server crashes, Firefox may stay open. Manually close it or kill the geckodriver process.

## Security Considerations

- **API Key Storage**: The API key is cached in memory only (not persisted to disk)
- **Visible Browser**: The browser runs in visible mode, allowing you to see all operations
- **Manual Login**: You manually log in each time, so credentials are never stored by the server
- **Local Only**: The server binds to `127.0.0.1` (localhost) by default

## License

[Add your license here]

## Contributing

Contributions welcome! Please open an issue or pull request.

## Acknowledgments

- [ProjectionLab](https://projectionlab.com/) for the excellent financial planning tool
- [MCP](https://modelcontextprotocol.io/) for the protocol specification
- [rmcp](https://github.com/isaidspaghetti/rmcp) for the Rust MCP implementation
- [thirtyfour](https://github.com/stevepryde/thirtyfour) for browser automation

## Links

- [ProjectionLab Plugin API Docs](https://app.projectionlab.com/docs/module-PluginAPI.html)
- [Model Context Protocol](https://modelcontextprotocol.io/)
- [rmcp Documentation](https://docs.rs/rmcp/)
