#!/bin/bash
# CogniCode MCP — Docker entrypoint
# Launches cognicode-mcp-server (and, with --multi, the Explorer API).

set -e

# The server binds a non-loopback address inside the container, and it
# refuses to serve one without a credential: fail here with the reason
# instead of a container that exits on startup.
if [ -z "${COGNICODE_MCP_AUTH_TOKEN:-}" ]; then
    echo "ERROR: COGNICODE_MCP_AUTH_TOKEN is not set." >&2
    echo "The server listens on 0.0.0.0:9847 and refuses to serve without a token." >&2
    echo "Run with: -e COGNICODE_MCP_AUTH_TOKEN=<secret>" >&2
    exit 64
fi

# If --multi flag, start Explorer API (multi-project mode)
if [ "$1" = "--multi" ]; then
    echo "Starting Explorer API (multi-project) on :8010"
    /usr/local/bin/explorer-api --listen 0.0.0.0:8010 --cwd /workspaces &
fi

PROJECT_DIR="${COGNICODE_PROJECT:-/workspace}"
echo "Starting CogniCode MCP HTTP/SSE Server on :9847 for $PROJECT_DIR"
exec /usr/local/bin/cognicode-mcp-server --cwd "$PROJECT_DIR" --listen 0.0.0.0:9847
