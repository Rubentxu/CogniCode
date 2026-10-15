#!/usr/bin/env bash
# Test: run_campaign.sh must propagate worker failures to its exit code.
#
# Background: a P0 audit found three places where a red campaign could
# return exit 0:
#
#   1. Single-repeat path hardcoded `exit 0` after _run_campaign.
#   2. Multi-repeat path accumulated MULTI_FAILURES but never used it
#      as the script's exit code.
#   3. _mark_campaign_terminal complete 0 hardcoded 0 as the worker
#      exit code in the manifest, lying about what happened.
#
# This test pins the contract at the source level. The patterns are
# minimal and semantic. A behavioral complement (run the campaign
# with a real failing worker, check exit code) belongs in a follow-up
# commit; for now the static check runs in <1s without needing a
# built orchestrator binary.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUN_CAMPAIGN="$SCRIPT_DIR/run_campaign.sh"

if [ ! -f "$RUN_CAMPAIGN" ]; then
    echo "FAIL: $RUN_CAMPAIGN not found"
    exit 1
fi

failures=0

# Strip comments and blank lines, then run pattern checks against the
# stripped view. This prevents comment text from triggering the patterns.
stripped=$(mktemp)
trap 'rm -f "$stripped"' EXIT
sed -E 's/[[:space:]]*#.*$//; /^[[:space:]]*$/d' "$RUN_CAMPAIGN" > "$stripped"

# ── Contract 1: the script must capture _run_campaign's exit code. ──
# Without this, the caller cannot branch on worker success vs failure.
if ! grep -qE '(CAMPAIGN_RC|REPEAT_RC|FAILURES)=\$\?' "$stripped"; then
    echo "FAIL: script does not capture _run_campaign's exit code."
    echo "      A red campaign cannot be distinguished from a green one."
    failures=$((failures + 1))
fi

# ── Contract 2: there must be a non-zero exit reachable when the
# captured exit code is non-zero. We accept any of:
#   - `exit "$CAMPAIGN_RC"` (variable in exit)
#   - `if [ ... ]; then exit 1; fi` (guarded non-zero exit)
#   - `exit $FAILURES` (FAILURES in exit)
# The pre-fix code had `exit 0` with no FAILURES check.
#
# The simplest semantic check: the script must contain a literal
# `exit` (not just `exit 0`) somewhere reachable from _run_campaign.
# We grep for `exit 1`, `exit "$..."`, or `exit $FAILURES` as proxies.
if ! grep -qE 'exit[[:space:]]+([^0[:space:]]|"\$\$?[^"]+")' "$stripped"; then
    echo "FAIL: script has no non-zero exit path."
    echo "      A red campaign would exit 0."
    failures=$((failures + 1))
fi

# ── Contract 3: the script must call _mark_campaign_terminal incomplete
# in at least one code path. The pre-fix code only had `complete`,
# which always said "the campaign finished successfully" regardless
# of what actually happened.
if ! grep -qE '_mark_campaign_terminal[[:space:]]+incomplete' "$stripped"; then
    echo "FAIL: script never calls _mark_campaign_terminal incomplete."
    echo "      The campaign_manifest.json can never report a red campaign."
    failures=$((failures + 1))
fi

# ── Contract 4: the single-repeat path must NOT have a bare `exit 0`
# immediately after _run_campaign without a CAMPAIGN_RC capture in
# between. We extract the single-repeat block and check.
single_repeat_block=$(awk '
    /REPEAT[[:space:]]*-eq[[:space:]]*1/ { in_block=1 }
    in_block { print }
    in_block && /^fi$/ { in_block=0 }
' "$stripped")

if echo "$single_repeat_block" | grep -qE 'exit 0[[:space:]]*$'; then
    echo "FAIL: single-repeat path has a bare 'exit 0'."
    echo "      Block:"
    echo "$single_repeat_block" | sed 's/^/        /'
    failures=$((failures + 1))
fi

# ── Contract 5: the multi-repeat path must accumulate per-iteration
# RC into MULTI_FAILURES and use it to gate the script's exit.
if ! grep -qE 'MULTI_FAILURES=\$\(\(MULTI_FAILURES' "$stripped"; then
    echo "FAIL: multi-repeat path does not accumulate MULTI_FAILURES."
    failures=$((failures + 1))
fi

if ! grep -qE 'if[[:space:]]+.*FAILURES.*then[[:space:]]*$' "$stripped"; then
    echo "FAIL: script has no FAILURES-gated exit path."
    failures=$((failures + 1))
fi

if [ "$failures" -gt 0 ]; then
    echo ""
    echo "Test result: FAIL ($failures contract(s) violated)"
    exit 1
fi

echo "Test result: PASS"
exit 0
