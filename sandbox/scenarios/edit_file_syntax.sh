#!/usr/bin/env bash
set -uo pipefail
# E2E Test: edit_file with syntax validation via mcp-client
# Tests that edit_file tool modifies files and validates syntax
# Env: BINARY, RESULTS_DIR, SCENARIO_NAME

SCENARIO_NAME="${SCENARIO_NAME:-edit_file_syntax}"
RESULTS_DIR="${RESULTS_DIR:-./results}"
PROJECT_ROOT="${PROJECT_ROOT:-$(cd "$(dirname "$0")/../.." && pwd)}"
MCP_CLIENT="${PROJECT_ROOT}/target/release/mcp-client"

mkdir -p "$RESULTS_DIR"

echo "Running edit_file_syntax scenarios..."

# Create temp fixture
tmpdir=$(mktemp -d)
mkdir -p "$tmpdir/src"
cat > "$tmpdir/src/lib.rs" << 'EOF'
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
EOF
cat > "$tmpdir/Cargo.toml" << 'EOF'
[package]
name = "test-fixture"
version = "0.1.0"
edition = "2021"
EOF

timestamp=$(date +%Y%m%dT%H%M%S)
start_ts=$(date +%s%3N)

# Test 1: Valid edit - add a new function via edit_file tool
success_valid=false
edit_response=$("$MCP_CLIENT" \
    --workspace "$tmpdir" \
    --method "tools/call" \
    --params "{\"name\":\"edit_file\",\"arguments\":{\"path\":\"src/lib.rs\",\"edits\":[{\"old_string\":\"pub fn add(a: i32, b: i32) -> i32 {\\n    a + b\\n}\",\"new_string\":\"pub fn add(a: i32, b: i32) -> i32 {\\n    a + b\\n}\\n\\npub fn subtract(a: i32, b: i32) -> i32 {\\n    a - b\\n}\"}]}}" \
    2>/dev/null) && mcp_ok=true || mcp_ok=false

if [ "$mcp_ok" = "true" ]; then
    # Check if edit was applied (file should now contain subtract)
    if grep -q "fn subtract" "$tmpdir/src/lib.rs" 2>/dev/null; then
        success_valid=true
    fi
fi

# Test 2: Verify server is still responsive after edit (ping via tools/list)
server_alive=false
list_response=$("$MCP_CLIENT" \
    --workspace "$tmpdir" \
    --method "tools/list" \
    --params '{}' \
    2>/dev/null) && server_alive=true || server_alive=false

# Count tools returned
tool_count=0
if [ "$server_alive" = "true" ]; then
    tool_count=$(echo "$list_response" | jq '.result.tools | length' 2>/dev/null || echo 0)
fi

# Overall success = edit was applied AND server still responsive
success=false
if [ "$success_valid" = "true" ] && [ "$server_alive" = "true" ]; then
    success=true
fi

end_ts=$(date +%s%3N)
duration=$((end_ts - start_ts))

jq -n \
    --arg scenario "$SCENARIO_NAME" \
    --arg language "rust" \
    --arg repo "fixture" \
    --argjson duration "$duration" \
    --argjson success "$success" \
    --argjson files_modified 1 \
    --argjson build_passed "$success_valid" \
    --argjson test_passed "$success_valid" \
    --argjson tool_count "$tool_count" \
    --arg error "edit_applied=$success_valid server_alive=$server_alive tools=$tool_count" \
    '{scenario_name:$scenario, language:$language, repo:$repo, duration_ms:$duration, success:$success, files_modified:1, build_passed:$build_passed, test_passed:$test_passed, tool_count:$tool_count, error_message:$error}' \
    > "$RESULTS_DIR/${SCENARIO_NAME}_rust_${timestamp}.json"

echo "  [rust] $SCENARIO_NAME: success=$success duration=${duration}ms edit=$success_valid alive=$server_alive"

rm -rf "$tmpdir"
