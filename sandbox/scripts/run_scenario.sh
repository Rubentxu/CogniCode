#!/usr/bin/env bash
set -euo pipefail
SCENARIO_SCRIPT="$1"
PROJECT_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BINARY="$PROJECT_ROOT/target/release/cognicode-mcp"
RESULTS_DIR="$(cd "$(dirname "$0")/../results" && pwd)"

scenario_name=$(basename "$SCENARIO_SCRIPT" .sh)
timestamp=$(date +%Y%m%dT%H%M%S)

echo "Running scenario: $scenario_name"

# Run the scenario script directly (it handles MCP server lifecycle)
REPO_DIR="$(cd "$(dirname "$0")/../repos" && pwd)" \
RESULTS_DIR="$RESULTS_DIR" \
BINARY="$BINARY" \
PROJECT_ROOT="$PROJECT_ROOT" \
SCENARIO_NAME="$scenario_name" \
bash "$SCENARIO_SCRIPT"

echo "Scenario $scenario_name completed"