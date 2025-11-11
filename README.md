# ProjectionLab MCP Server

A Model Context Protocol (MCP) server that enables LLM integration with [ProjectionLab](https://projectionlab.com/), a personal finance and retirement planning tool.

This server uses browser automation (Firefox + GeckoDriver) to interact with ProjectionLab's Plugin API, allowing AI assistants to read and update your financial data.

## Features

- **Browser Automation**: Uses thirtyfour (Selenium for Rust) with Firefox to execute JavaScript API calls
- **Secure Authentication**: Manual login via visible browser window, API key automatically retrieved
- **Full API Coverage**: All 7 ProjectionLab Plugin API methods exposed as MCP tools
- **HTTP Streamable Transport**: Runs as an HTTP server compatible with MCP clients
- **Nix Integration**: Fully configured flake for easy development and deployment

## Architecture

```
MCP Client (Claude Desktop/CLI)
  ↓ HTTP/SSE
MCP Server (this project)
  ↓ Browser Automation (thirtyfour)
Firefox (visible, headed mode)
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

### 3. Start the MCP Server

```bash
# Using Nix
nix run

# Or with cargo
cargo run --release
```

The server will:
- Start on `http://127.0.0.1:8000/mcp`
- Wait for an MCP client to connect
- On first connection, launch Firefox and wait for you to log in
- Automatically navigate to the plugins settings page
- Extract and cache your API key
- Be ready to handle tool requests

### 4. Configure Your MCP Client

Add this server to your MCP client configuration:

#### Claude Desktop

Edit `~/.config/Claude/claude_desktop_config.json` (or equivalent for your platform):

```json
{
  "mcpServers": {
    "projectionlab": {
      "url": "http://127.0.0.1:8000/mcp"
    }
  }
}
```

## Available Tools

Once connected, the following MCP tools are available:

### `update_account`

Updates an account in Current Finances with new data.

**Parameters:**
- `account_id` (string, required): The ID of the account to update
- `data` (object, required): New data for the account (e.g., `{"balance": 1000}`)
- `force` (boolean, optional): Allow assignment of new properties

**Example:**
```json
{
  "account_id": "12345",
  "data": {"balance": 50000},
  "force": false
}
```

### `export_data`

Exports all financial data from ProjectionLab.

**Parameters:** None

**Returns:** Complete JSON export of all your financial data

### `restore_current_finances`

Replaces the Current Finances state with new data.

**Parameters:**
- `new_state` (object, required): The new Current Finances state

**Warning:** This overwrites your current finances. Ensure data is well-formed.

### `restore_plans`

Replaces all Plans with a new set.

**Parameters:**
- `new_plans` (object, required): The new plans data

**Warning:** This overwrites all your plans. Ensure data is well-formed.

### `restore_progress`

Replaces the Progress state with new data.

**Parameters:**
- `new_progress` (object, required): The new progress data

**Warning:** This overwrites your progress tracking.

### `restore_settings`

Replaces Settings state with new data.

**Parameters:**
- `new_settings` (object, required): The new settings data

**Warning:** This overwrites your settings.

### `validate_api_key`

Validates that the cached API key is still valid.

**Parameters:** None

**Returns:** Validation result from ProjectionLab

## Usage Examples

### With Claude Desktop

Once configured, you can ask Claude to interact with your ProjectionLab data:

```
User: Export my current financial data from ProjectionLab

Claude: [uses export_data tool]

User: Update my checking account balance to $5,000

Claude: [uses update_account tool with account_id and new balance]
```

### Direct API Testing

You can test the server directly via HTTP:

```bash
curl -X POST http://127.0.0.1:8000/mcp/tools/list
```

## Development

### Project Structure

```
projectionlab-mcp/
├── src/
│   ├── main.rs         # HTTP server setup and entry point
│   ├── server.rs       # MCP ServerHandler implementation + tool definitions
│   └── browser.rs      # Browser automation layer (thirtyfour)
├── Cargo.toml          # Rust dependencies
├── flake.nix           # Nix development environment
├── rust-toolchain.toml # Rust toolchain specification
└── README.md           # This file
```

### Running in Development Mode

```bash
# Enter Nix environment (if using Nix)
nix develop

# Run with debug logging
RUST_LOG=debug cargo run

# Run tests (if any)
cargo test
```

### Adding New Tools

To add new MCP tools:

1. Define the tool method in `src/server.rs` with `#[tool]` macro
2. Use `self.get_browser()` to access the browser session
3. Call `browser.call_plugin_api("methodName", args)` with appropriate args
4. Handle errors and return `CallToolResult`

The `#[tool_router]` and `#[tool_handler]` macros automatically register your tools.

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
