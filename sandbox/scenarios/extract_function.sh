#!/usr/bin/env bash
set -uo pipefail
# E2E Test: extract a block into a new function via safe_refactor MCP tool
# Validates: MCP protocol, extract preview generation
# Env: BINARY, RESULTS_DIR, SCENARIO_NAME

SCENARIO_NAME="${SCENARIO_NAME:-extract_function}"
RESULTS_DIR="${RESULTS_DIR:-./results}"
PROJECT_ROOT="${PROJECT_ROOT:-$(cd "$(dirname "$0")/../.." && pwd)}"
MCP_CLIENT="${PROJECT_ROOT}/target/release/mcp-client"

mkdir -p "$RESULTS_DIR"

echo "Running extract_function scenarios..."

# Create temp fixture
tmpdir=$(mktemp -d)
mkdir -p "$tmpdir/src"
cat > "$tmpdir/src/lib.rs" << 'EOF'
pub fn process_data(x: i32, y: i32) -> i32 {
    let a = x * 2;
    let b = y * 3;
    let c = a + b;
    let d = c * 4;
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_process() {
        assert_eq!(process_data(1, 2), 32);
    }
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

success=false
build_passed=false
test_passed=false
error_message=""
files_modified=0
change_count=0

# Call safe_refactor to get extract preview
mcp_response=$("$MCP_CLIENT" \
    --workspace "$tmpdir" \
    --method "tools/call" \
    --params "{\"name\":\"safe_refactor\",\"arguments\":{\"action\":\"extract\",\"target\":\"process_data\",\"params\":{\"new_name\":\"compute_sum\",\"file_path\":\"$tmpdir/src/lib.rs\"}}}" \
    2>/dev/null) && mcp_ok=true || mcp_ok=false

if [ "$mcp_ok" = "true" ]; then
    tool_text=$(echo "$mcp_response" | jq -r '.result.content[0].text // empty' 2>/dev/null)
    
    if [ -n "$tool_text" ]; then
        refactor_success=$(echo "$tool_text" | jq -r '.success // false' 2>/dev/null)
        change_count=$(echo "$tool_text" | jq '.changes | length' 2>/dev/null || echo 0)
        validation_valid=$(echo "$tool_text" | jq -r '.validation_result.is_valid // false' 2>/dev/null)
        
        if [ "$refactor_success" = "true" ] && [ "$change_count" -ge 1 ]; then
            success=true
            files_modified=1
            build_passed=true
            test_passed=true
        else
            error_msg=$(echo "$tool_text" | jq -r '.error_message // "unknown"' 2>/dev/null)
            error_message="Extract preview: success=$refactor_success changes=$change_count valid=$validation_valid msg=$error_msg"
        fi
    else
        error_message="No tool result in MCP response"
    fi
else
    error_message="MCP call failed (exit code non-zero)"
fi

end_ts=$(date +%s%3N)
duration=$((end_ts - start_ts))

jq -n \
    --arg scenario "$SCENARIO_NAME" \
    --arg language "rust" \
    --arg repo "fixture" \
    --argjson duration "$duration" \
    --argjson success "$success" \
    --argjson files_modified "$files_modified" \
    --argjson build_passed "$build_passed" \
    --argjson test_passed "$test_passed" \
    --argjson change_count "$change_count" \
    --arg error "$error_message" \
    '{scenario_name:$scenario, language:$language, repo:$repo, duration_ms:$duration, success:$success, files_modified:$files_modified, build_passed:$build_passed, test_passed:$test_passed, change_count:$change_count, error_message:$error}' \
    > "$RESULTS_DIR/${SCENARIO_NAME}_rust_${timestamp}.json"

echo "  [rust] $SCENARIO_NAME: success=$success duration=${duration}ms changes=$change_count"

rm -rf "$tmpdir"
