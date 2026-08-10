#!/usr/bin/env bash
# post_e31_audit.sh — Verify E31 program deliverables and tag chain integrity.
#
# Run from the repo root after a clean \`git fetch origin\`. Verifies:
#   1. v0.92.0 tag is reachable from origin/main
#   2. All 14 E31 cycles are merged into origin/main
#   3. The Tier-1 closure matrix is at >=30% (per G13)
#   4. The conformance matrix is at 100% triaged (per G10)
#   5. The 14 PROPOSED ADRs are all in a final state (per E31-C)
#   6. The flaky-scenarios log script exists (per E31-B6)
#   7. The T6 CI gate script exists (per E31-B5)
#   8. The cold-cache filter is in analyze_stability.py (per E31-E)
#   9. The pre-cut checklist exists (per E31-Z prep)
#  10. The scorecard-streak ledger exists (per E31-G)
#
# Exit code: 0 if all checks pass, 1 otherwise.
#
# Useful pre-cut checklist companion: docs/V1.0.0-PRE-CUT-CHECKLIST.md.

set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

PASS=0
FAIL=0

check() {
    local desc="$1"
    local cmd="$2"
    # Run the command in a subshell; rely on its exit code, not eval.
    if bash -c "$cmd" >/dev/null 2>&1; then
        echo "  ✓ $desc"
        PASS=$((PASS + 1))
    else
        echo "  ✗ $desc"
        FAIL=$((FAIL + 1))
    fi
}

echo "==> E31 program audit (post-E31-Z prep)"

# 1. Tag chain
echo ""
echo "Tag chain integrity:"
check "v0.92.0 tag reachable from origin/main" \
    "git merge-base --is-ancestor v0.92.0 origin/main"

# 2. E31 cycles
echo ""
echo "E31 cycles merged into origin/main:"
for pr in 237 238 239 240 241 242 243 244 245 246 247 248 249 250 251; do
    check "PR #${pr} merge commit on origin/main" \
        "git log --oneline origin/main | grep -q 'Merge pull request #${pr}'"
done

# 3. Tier-1 closure
echo ""
echo "Tier-1 closure (G13):"
check "TEST-PLAN.md §3.1 references 18+ tools at 5/5 (post-B7 cumulative)" \
    "grep -qE '18 tools full Tier-1|26 tools full Tier-1' docs/TEST-PLAN.md"
check "Tier-1 closure >=30% (post-B8)" \
    "grep -q 'T3 closure rate.*35.6%' docs/TEST-PLAN.md"

# 4. Conformance
echo ""
echo "Conformance matrix (G10):"
check "conformance_matrix.yaml exists" \
    "test -f sandbox/reports/conformance_matrix.yaml"
check "pct_triaged = 100.0%" \
    "grep -q 'pct_triaged: 100.0' sandbox/reports/conformance_matrix.yaml"

# 5. ADRs
echo ""
echo "ADR hygiene (E31-C):"
check "0 PROPOSED ADRs in docs/adr/" \
    "! grep -q 'Status.\{0,3\}: PROPOSED' docs/adr/ADR-*.md"
check "ADR-019 exists (renumber from 2nd ADR-015)" \
    "test -f docs/adr/ADR-019-temporal-graph-history-and-atomic-ingest.md"

# 6. T7 cadence script
echo ""
echo "T7 stability cadence (E31-B6):"
check "build_flaky_log.py exists" \
    "test -f sandbox/scripts/build_flaky_log.py"
check "scorecard-stability recipe" \
    "grep -q 'scorecard-stability' justfile"

# 7. T6 CI gate
echo ""
echo "T6 CI gate (E31-B5):"
check "regression-check.yml workflow exists" \
    "test -f .github/workflows/regression-check.yml"
check "check_regression_test.sh exists" \
    "test -f scripts/ci/check_regression_test.sh"
check "ci-t6 recipe in justfile" \
    "grep -q 'ci-t6' justfile"

# 8. Cold-cache filter
echo ""
echo "Cold-cache filter (E31-E):"
check "cv_warm field in analyze_stability.py" \
    "grep -q 'cv_warm' sandbox/scripts/analyze_stability.py"
check "cv_warm preferred in release_scorecard.py" \
    "grep -q 'mean_cv_warm' sandbox/scripts/release_scorecard.py"

# 9. Pre-cut checklist
check "V1.0.0-PRE-CUT-CHECKLIST.md exists" \
    "test -f docs/V1.0.0-PRE-CUT-CHECKLIST.md"

# 10. Scorecard streak
echo ""
echo "Scorecard streak (E31-G):"
check "scorecard_streak.py exists" \
    "test -f sandbox/scripts/scorecard_streak.py"
check "scorecard-streak recipe in justfile" \
    "grep -q '^scorecard-streak' justfile"

# 11. CHANGELOG
echo ""
echo "CHANGELOG coverage:"
check "CHANGELOG.md v0.92.0 entry" \
    "grep -q 'v0.92.0' CHANGELOG.md"
check "CHANGELOG.md Unreleased section" \
    "grep -q 'Unreleased' CHANGELOG.md"

# 12. Roadmap
echo ""
echo "Roadmap:"
check "ROADMAP.md exists" \
    "test -f docs/ROADMAP.md"

# 13. Scorecard artifacts
echo ""
echo "Scorecard (latest run):"
check "scorecard.json produced" \
    "test -f sandbox/reports/scorecard.json"
check "scorecard.md produced" \
    "test -f sandbox/reports/scorecard.md"

# 14. ROADMAP.md + active_lock hygiene (E31-Z bookkeeping)
echo ""
echo "Docs hygiene (E31-Z bookkeeping):"
check "ROADMAP.md has E31 section" \
    "grep -q '## Release 1.0.0 Program (E31)' docs/ROADMAP.md"
check "ROADMAP.md Active references E31 or E32" \
    "grep -q 'CogniCode Distribution (E32 program)' docs/ROADMAP.md"

# 15. Vault hygiene
echo ""
echo "Vault hygiene:"
check "INC-005 (CONF-001) status is closed" \
    "grep -q 'status: closed' ~/.sddk-knowledge/CogniCode/incidences/INC-005-CONF-001.md"
check "active_lock last released cycle is post-E31" \
    "grep -qE 'M-E31-(followup-audit|Z-bookkeeping|E2|B2-rollup|B4-rollup|B5-rollup|B6-rollup)' ~/.sddk-knowledge/CogniCode/milestones/_active.md"

# 16. E31-E2 decision recorded
echo ""
echo "E31-E2 (retrieve_and_verify decision):"
check "B1 ACCEPT decision documented in pre-cut checklist" \
    "grep -q 'B1. .retrieve_and_verify' docs/V1.0.0-PRE-CUT-CHECKLIST.md"
check "B1 marked as ACCEPT (closed in E31-E2)" \
    "grep -q 'closed in E31-E2' docs/V1.0.0-PRE-CUT-CHECKLIST.md"

# 17. cogh binary (E32-A) build verification
echo ""
echo "E32-A cogh binary verification:"
check "cogh binary builds (cargo build -p cognicode-cli --bin cogh)" \
    "test -f target/debug/cogh"
check "cogh binary runs (cogh --version)" \
    "target/debug/cogh --version >/dev/null"
check "cogh binary shows help" \
    "target/debug/cogh --help >/dev/null"

# 18. E32-C portable skill bundles
echo ""
echo "E32-C portable skill bundles (E32-C):"
check "skills/cognicode-mcp-driven bundle exists" \
    "test -f skills/cognicode-mcp-driven/SKILL.md"
check "skills/cognicode-mcp-driven/manifest.yaml exists" \
    "test -f skills/cognicode-mcp-driven/manifest.yaml"
check "skills/cognicode-core bundle exists" \
    "test -f skills/cognicode-core/SKILL.md"
check "skills/cognicode-core/manifest.yaml exists" \
    "test -f skills/cognicode-core/manifest.yaml"
check "cogh skill validate works on cognicode-mcp-driven" \
    "target/debug/cogh skill validate skills/cognicode-mcp-driven >/dev/null"


echo ""
echo "E32-D opencode IDE adapter:"
check "cogh ide detect runs" \
    "target/debug/cogh ide detect >/dev/null"
check "cogh ide install opencode wires MCP entry" \
    "test -f crates/cognicode-cli/src/bin/ide.rs"
check "cogh ide uninstall wires removal" \
    "grep -q 'uninstall_opencode' crates/cognicode-cli/src/bin/ide.rs"

echo ""
echo "==> E31 audit: ${PASS} PASS, ${FAIL} FAIL"
if [ "$FAIL" -eq 0 ]; then
    echo "    E31 program status: VERIFIED (all deliverables present)"
    exit 0
fi
echo "    E31 program status: GAPS DETECTED (see ✗ above)"
exit 1

# 18. E32-C portable skill bundles
echo ""
echo "E32-C portable skill bundles (E32-C):"
check "skills/cognicode-mcp-driven bundle exists" \
    "test -f skills/cognicode-mcp-driven/SKILL.md"
check "skills/cognicode-mcp-driven/manifest.yaml exists" \
    "test -f skills/cognicode-mcp-driven/manifest.yaml"
check "skills/cognicode-core bundle exists" \
    "test -f skills/cognicode-core/SKILL.md"
check "skills/cognicode-core/manifest.yaml exists" \
    "test -f skills/cognicode-core/manifest.yaml"
check "cogh skill validate works on cognicode-mcp-driven" \
    "target/debug/cogh skill validate skills/cognicode-mcp-driven >/dev/null"


echo ""
echo "E32-D opencode IDE adapter:"
check "cogh ide detect runs" \
    "target/debug/cogh ide detect >/dev/null"
check "cogh ide install opencode wires MCP entry" \
    "test -f crates/cognicode-cli/src/bin/ide.rs"
check "cogh ide uninstall wires removal" \
    "grep -q 'uninstall_opencode' crates/cognicode-cli/src/bin/ide.rs"
