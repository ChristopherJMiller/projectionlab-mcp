#!/usr/bin/env bash
# Wrapper script for MCP stdio transport.
# Builds first (output suppressed), then runs the binary cleanly.
# stdout: MCP JSON-RPC protocol only
# stderr: tracing logs from the server
set -euo pipefail

cd "$(dirname "$0")"

# Build silently — all cargo/nix output goes to /dev/null
nix develop -c cargo build --quiet 2>/dev/null

# Resolve the binary path, then exec it directly under nix develop.
# The `--` prevents nix from interpreting the binary's arguments.
# Nix's own stderr warnings are harmless (they go to stderr, not stdout).
exec nix develop --quiet -c ./target/debug/projectionlab-mcp
