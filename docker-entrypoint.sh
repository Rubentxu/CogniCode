#!/bin/bash
# CogniCode MCP — Docker entrypoint
# Launches the cognicode-mcp-server. The Explorer API is not hosted here:
# Rubentxu/cognicode-explorer builds its own container.

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

PROJECT_DIR="${COGNICODE_PROJECT:-/workspace}"
echo "Starting CogniCode MCP HTTP/SSE Server on :9847 for $PROJECT_DIR"
exec /usr/local/bin/cognicode-mcp-server --cwd "$PROJECT_DIR" --listen 0.0.0.0:9847
