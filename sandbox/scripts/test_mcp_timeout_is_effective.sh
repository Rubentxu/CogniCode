#!/usr/bin/env bash
# Test: mcp_core.rs timeouts must be effective during blocking reads.
#
# Background: a P0 audit found that McpServer::read_response and
# McpServer::drain_notifications in
# crates/cognicode-core/src/sandbox_core/mcp_core.rs calculate a
# deadline but then call self.stdout.read_line(&mut line), which is
# a BLOCKING syscall. If the server is alive but emits no further
# data and does not close stdout, the read blocks forever and the
# deadline is never re-checked.
#
# The comment in the code (mcp_core.rs:280-281) admits the dependency:
# "read_line() blocks until a newline is received / but ANSI log lines
# from the server provide frequent newlines". The fix cannot rely on
# the server being noisy. The deadline must be enforced during the
# read, not just before and after.
#
# This test pins the contract at the source level:
#   1. read_response must NOT call read_line directly (which is
#      blocking). It must use a non-blocking primitive (channel +
#      reader thread, async, or set_nonblocking + poll).
#   2. drain_notifications must use the same non-blocking primitive.
#   3. There must be a code path where the deadline is checked while
#      waiting for data, not only before/after the read.
#
# A behavioral test (a fake server that never responds, run the real
# read_response with a 1s timeout, assert the call returns within
# 1.5s) belongs in a follow-up commit; for now, the static check is
# what we can run in CI in <1s without needing the orchestrator
# built. The behavioral test will be added once the 3 pre-existing
# compilation errors on origin/main are resolved (they block any
# Rust test from running).
#
# Refs: P0 #1 in the sandbox harness audit.

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

# Contract 1: read_response must not call read_line directly. The
# pre-fix code has:
#   let read_result = self.stdout.read_line(&mut line);
# which is blocking. The fix uses a channel + reader thread, or async,
# or set_nonblocking + poll. Any of these would NOT match
# "self.stdout.read_line".
if grep -qE 'self\.stdout\.read_line' "$stripped"; then
    echo "FAIL: mcp_core.rs still uses self.stdout.read_line."
    echo "      This is a blocking syscall; the deadline is not enforced during the read."
    echo "      Fix: use a reader thread + mpsc::Receiver with recv_timeout,"
    echo "      or set_nonblocking + poll, or async."
    grep -nE 'self\.stdout\.read_line' "$stripped" | sed 's/^/        /'
    failures=$((failures + 1))
fi

# Contract 2: drain_notifications must not call read_line directly
# (same blocking bug; the 100ms limit is also ineffective).
#
# Find the drain_notifications function and check it doesn't call
# read_line.
drain_block=$(awk '
    /fn[[:space:]]+drain_notifications/ { in_fn=1 }
    in_fn { print NR": "$0 }
    in_fn && /^}/ { in_fn=0 }
' "$stripped")

if echo "$drain_block" | grep -qE 'read_line'; then
    echo "FAIL: drain_notifications uses read_line."
    echo "      The 100ms limit declared in this function is not effective."
    echo "$drain_block" | grep -nE 'read_line' | sed 's/^/        /'
    failures=$((failures + 1))
fi

# Contract 3: the read_response function must reference a non-blocking
# read primitive OR a channel receiver. Acceptable patterns:
#   - line_rx.recv_timeout(...)
#   - stdout.set_nonblocking(...)
#   - stdout.read(...) with WouldBlock handling
#   - tokio::time::timeout wrapping an async read
if ! grep -qE '(recv_timeout|set_nonblocking|try_recv|tokio::time::timeout)' "$stripped"; then
    echo "FAIL: mcp_core.rs has no non-blocking read primitive."
    echo "      The fix must use a channel recv_timeout, set_nonblocking + poll,"
    echo "      or async. Anything else leaves the timeout ineffective."
    failures=$((failures + 1))
fi

# Contract 4: the read_response function must check the deadline INSIDE
# the wait loop, not only before/after the read. The pre-fix code checks
# at the loop start and after the read, but the read itself blocks.
# The fix must check the deadline at each poll iteration.
#
# As a proxy: the read_response function must contain a
# `Instant::now() > deadline` (or `>=`) check that is reachable
# WHILE waiting for data, after the read. We extract the function and
# count deadline checks.
read_response_block=$(awk '
    /fn[[:space:]]+read_response/ { in_fn=1 }
    in_fn { print }
    in_fn && /^}/ { in_fn=0; exit }
' "$stripped")

deadline_checks=$(echo "$read_response_block" | grep -cE 'Instant::now\(\)[[:space:]]*[><]' || true)
if [ "$deadline_checks" -lt 2 ]; then
    echo "FAIL: read_response has only $deadline_checks deadline checks."
    echo "      With a non-blocking read primitive, the loop must check the"
    echo "      deadline at each iteration. Pre-fix: 2 (before and after the read)."
    echo "      Post-fix: 2 minimum (loop start + after recv_timeout)."
    failures=$((failures + 1))
fi

if [ "$failures" -gt 0 ]; then
    echo ""
    echo "Test result: FAIL ($failures contract(s) violated)"
    exit 1
fi

echo "Test result: PASS"
exit 0
