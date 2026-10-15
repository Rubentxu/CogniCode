#!/usr/bin/env bash
# Test: the MCP tool catalog (product/tools.json) must not list any
# CogniCode CLI binary as if it were an MCP tool.
#
# Background: the audit that produced BLOQUE 1 H1 found that the
# public MCP tool catalog (product/tools.json) listed 'cogh' as a
# stable MCP tool. cogh is actually the CogniCode version manager
# CLI binary (crates/cognicode-cli/src/bin/cogh.rs) — a standalone
# binary with no MCP handler in call_tool_handler. Two tests on
# origin/main (and the H1 branch) correctly flagged the ghost:
#
#   - interface::mcp::capabilities::tests::test_capabilities_matrix_for_stable_tools
#     'PRF-ANA-01: tool estable cogh no tiene capabilities declaradas'
#   - interface::mcp::mcp_roundtrip_tests::tests::tool_surface_parity::test_all_listed_tools_are_dispatchable
#     'Listed tools missing from dispatch allowlist: [cogh]'
#
# The fix in commit a68c179 removed the entry from the catalog. The
# runtime tests are the oracle; they are slow (>10s for the full
# core lib suite) and require a buildable tree. This static test
# is the contract pin: a CI-level check that runs in <100ms and
# catches the same kind of mistake at the source, without needing
# to compile the workspace.
#
# The contract:
#   1. Read every "name": "..." in product/tools.json
#   2. Cross-check against the known CLI binaries (see KNOWN_CLI_BINARIES)
#   3. Fail with a clear error if any catalog entry matches a known CLI
#
# To extend: add a new CLI binary to KNOWN_CLI_BINARIES below and the
# same regression coverage applies to it automatically.
#
# Refs: BLOQUE 1 H1 stabilization (PR #346), commit a68c179.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
CATALOG="$REPO_ROOT/product/tools.json"

if [ ! -f "$CATALOG" ]; then
    echo "FAIL: $CATALOG not found"
    exit 1
fi

# Known CLI binaries that are NOT MCP tools. The catalog must not
# list any of these.
#
# Extend by appending a quoted name; the loop below picks them up.
KNOWN_CLI_BINARIES=(
    "cogh"
)

# Extract every catalog name. Use python so we get exact JSON-parsed
# names (no false positives from substrings or escapes).
names_json=$(python3 -c "
import json
with open('$CATALOG') as f:
    catalog = json.load(f)
print(' '.join(t['name'] for t in catalog['tools'] if 'name' in t))
" 2>/dev/null) || {
    echo "FAIL: cannot parse $CATALOG as JSON"
    exit 1
}

if [ -z "$names_json" ]; then
    echo "FAIL: $CATALOG parsed but produced no tool names"
    exit 1
fi

failures=0
for cli in "${KNOWN_CLI_BINARIES[@]}"; do
    # Use a case-style match: an exact whole-word match (space-delimited).
    # We check for the CLI name appearing as a standalone token.
    case " $names_json " in
        *" $cli "*)
            echo "FAIL: catalog lists CLI binary '$cli' as an MCP tool."
            echo "      '$cli' is a standalone binary (see crates/cognicode-cli/src/bin/$cli.rs)"
            echo "      and has no MCP handler in call_tool_handler. Remove the entry"
            echo "      from product/tools.json, or add an MCP handler + capabilities"
            echo "      + dispatch arm if it genuinely is an MCP tool."
            failures=$((failures + 1))
            ;;
    esac
done

# Sanity contract: the catalog must list AT LEAST one tool. A silent
# regression that empties the catalog should also fail this test.
name_count=$(echo "$names_json" | wc -w)
if [ "$name_count" -eq 0 ]; then
    echo "FAIL: catalog is empty (0 tools); this is itself a regression."
    failures=$((failures + 1))
fi

if [ "$failures" -gt 0 ]; then
    echo ""
    echo "Test result: FAIL ($failures contract(s) violated, $name_count catalog entries)"
    exit 1
fi

echo "Test result: PASS ($name_count catalog entries; none match known CLI binaries)"
exit 0
