#!/usr/bin/env bash
# MCP JSON-RPC client helper
# Usage: mcp_call.sh <binary> <workspace> <method> <params_json>
# Sends initialize + the requested method via stdio to cognicode-mcp
# Returns the JSON response from the requested method
set -euo pipefail

BINARY="$1"
WORKSPACE="$2"
METHOD="$3"
PARAMS="${4:-{}}"

# Create a temp file with the JSON-RPC messages
# MCP requires: initialize → initialized notification → then the actual request
REQUEST_FILE=$(mktemp)
trap "rm -f $REQUEST_FILE" EXIT

# Build the request sequence
cat > "$REQUEST_FILE" << REQUESTS
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"cognicode-benchmark","version":"1.0.0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"$METHOD","params":$PARAMS}
REQUESTS

# Send to MCP server via stdin, capture all output
# The server outputs one JSON per line on stdout
RESPONSE=$("$BINARY" --cwd "$WORKSPACE" < "$REQUEST_FILE" 2>/dev/null || true)

# Extract the response for our request (id:2)
# Each line is a JSON-RPC message; find the one with id:2
echo "$RESPONSE" | while IFS= read -r line; do
    if echo "$line" | jq -e '.id == 2' 2>/dev/null | grep -q true; then
        echo "$line"
        break
    fi
done
