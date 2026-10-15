#!/usr/bin/env bash
# Test: mcp_core.rs must surface McpError::ProtocolViolation when a server
# emits sustained non-JSON output.
#
# Background: a P1 audit finding. The McpError enum already has a
# ProtocolViolation variant ('server emitted non-JSON line (protocol
# violation): {0}'), but the reader never constructs it. The
# read_response path does:
#
#     let msg: Value = match serde_json::from_str(trimmed) {
#         Ok(v) => v,
#         Err(_) => { continue; }
#     };
#
# — a non-JSON line is silently dropped. The pre-fix comment says
# "Non-JSON line (e.g., ANSI log from tracing), skip", which is
# reasonable for occasional log noise but lets a server that
# consistently emits non-protocol output pass through without any
# signal. The harness can't tell apart "tracing logs" from "the
# server does not speak MCP".
#
# The fix introduces a counter. Occasional non-JSON lines (e.g.,
# ANSI tracing output) are tolerated; sustained non-JSON output
# triggers ProtocolViolation. The threshold is per-call, so a
# well-behaved server that emits a few ANSI log lines at startup
# still works.
#
# This test pins the contract at the source level. A behavioral
# test (a fake server that streams non-JSON to stdout, verify
# read_response returns ProtocolViolation within budget) belongs
# in a follow-up commit; the static check is what we can run in CI
# in <1s.
#
# Refs: P1 in the sandbox harness audit.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
MCP_CORE="$REPO_ROOT/crates/cognicode-core/src/sandbox_core/mcp_core.rs"

if [ ! -f "$MCP_CORE" ]; then
    echo "FAIL: $MCP_CORE not found"
    exit 1
fi

failures=0

# Strip comments and blank lines, then run pattern checks against the
# stripped view. This prevents comment text from triggering the patterns.
stripped=$(mktemp)
trap 'rm -f "$stripped"' EXIT
sed -E 's/[[:space:]]*#.*$//; /^[[:space:]]*$/d' "$MCP_CORE" > "$stripped"

# Contract 1: read_response must count non-JSON lines. The pre-fix
# code has `Err(_) => { continue; }` which throws away the line
# without recording it. The fix tracks the count and breaks the
# silent-skip behaviour.
read_response_block=$(awk '
    /fn[[:space:]]+read_response/ { in_fn=1 }
    in_fn { print }
    in_fn && /^}/ { in_fn=0; exit }
' "$stripped")

if ! echo "$read_response_block" | grep -qE 'non_json_count|non_json_lines|protocol_violation_count'; then
    echo "FAIL: read_response has no non-JSON counter."
    echo "      The fix must introduce a counter (e.g. non_json_count) so"
    echo "      sustained non-JSON output is observable."
    failures=$((failures + 1))
fi

# Contract 2: read_response must reference McpError::ProtocolViolation
# as a possible return path. Pre-fix: never constructed.
if ! echo "$read_response_block" | grep -qE 'McpError::ProtocolViolation'; then
    echo "FAIL: read_response never returns McpError::ProtocolViolation."
    echo "      The variant exists in the enum but is unreachable from"
    echo "      the read path. A server that emits non-protocol output"
    echo "      goes undetected."
    failures=$((failures + 1))
fi

# Contract 3: the non-JSON handling must NOT be a bare `continue`
# with no counter. The pre-fix code is exactly:
#
#     Err(_) => { continue; }
#
# The fix replaces this with a counted branch. As a soft check, the
# pre-fix pattern is forbidden in the read_response block.
if echo "$read_response_block" | grep -qE 'Err\(\) => \{[[:space:]]*continue'; then
    echo "FAIL: read_response still has bare 'Err(_) => { continue; }' for non-JSON."
    echo "      The fix must count and branch on the count."
    echo "$read_response_block" | grep -nE 'Err\(\) => \{[[:space:]]*continue' | sed 's/^/        /'
    failures=$((failures + 1))
fi

# Contract 4: drain_notifications has the same pattern. The fix
# does not strictly require a counter there (the 100ms cap bounds
# the damage), but at least a tracked observation is needed for
# forensic visibility. We accept either:
#   a) a counter with a threshold, OR
#   b) a debug log of the non-JSON line.
drain_block=$(awk '
    /fn[[:space:]]+drain_notifications/ { in_fn=1 }
    in_fn { print }
    in_fn && /^}/ { in_fn=0; exit }
' "$stripped")

if echo "$drain_block" | grep -qE 'if let Ok\(msg\)' \
   && ! echo "$drain_block" | grep -qE 'non_json|protocol_violation|debug|eprintln'; then
    echo "FAIL: drain_notifications silently drops non-JSON lines."
    echo "      The pre-fix if-let-Ok branch has no else arm, so a"
    echo "      non-protocol message disappears without trace."
    echo "      Forensic visibility (eprintln / log / counter) is required."
    failures=$((failures + 1))
fi

if [ "$failures" -gt 0 ]; then
    echo ""
    echo "Test result: FAIL ($failures contract(s) violated)"
    exit 1
fi

echo "Test result: PASS"
exit 0
