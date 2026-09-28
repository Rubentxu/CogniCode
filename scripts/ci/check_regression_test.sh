#!/usr/bin/env bash
# scripts/ci/check_regression_test.sh — T6 (regression test in every fix(*))
#
# LOCAL-ONLY enforcement. Triggered by `act` running
# `.github/workflows/regression-check.yml` with workflow_dispatch.
# NOT executed on GitHub-hosted runners — the workflow has no
# `pull_request` / `push` / `schedule` triggers.
#
# Rule (per docs/TEST-PLAN.md §5):
#   Every pull request that contains `fix(*)` commits MUST also add,
#   modify, or re-enable at least one test in the same PR.
#
# The test can be unit (L1), integration (L2), sandbox scenario (L3), or
# browser-E2E (L4) — the level chosen depends on the bug's surface, but
# a test file MUST exist in the diff.
#
# Exit code:
#   0  PASS (no fix(*) commits, OR fix(*) commits with test files)
#   1  FAIL (fix(*) commits without any test file in the diff)
#   2  ERROR (the base ref could not be resolved, or resolved to HEAD, so the
#      diff this gate was asked to inspect was never read)
#
# `regression-check.yml` sets CI_T6_BASE from its `base_branch` input. When
# set, it is authoritative: an unresolvable value is an ERROR, never a
# silent fallback to a different range.
#
# Why exit 2 exists and matters: this gate reads the rule it enforces out of
# git history. If the range cannot be read, "no fix(*) commits" and "no
# commits at all" become indistinguishable, and the gate reports PASS on a
# diff it never saw. That is worse than no gate, because a false PASS is
# indistinguishable from a real one. A depth-1 clone with no `origin/main`
# and no resolvable `HEAD~1` reproduces it; pinned by
# scripts/ci/test_t6_gate_contract.py.

set -euo pipefail

# Resolve base branch (feature branch workflow): prefer origin/main, fall
# back to local main, then HEAD~1 to detect at least one commit back.
#
# The unresolvable case is an ERROR, not a fallback. The `|| true` that used
# to sit on the `git log` below is what let an unread range read as a clean
# bill: git printed "ambiguous argument" to stderr, the pipeline produced
# nothing, and the gate counted zero fix(*) commits as "nothing to enforce".
DIFF_BASE=""
DIFF_BASE_EXPLICIT=0
if [ -n "${CI_T6_BASE:-}" ]; then
  # Operator-supplied base wins, and must resolve: substituting a different
  # range would report a confident verdict about a range nobody asked for.
  DIFF_BASE="$CI_T6_BASE"
  DIFF_BASE_EXPLICIT=1
elif git rev-parse --verify --quiet origin/main >/dev/null 2>&1; then
  DIFF_BASE="origin/main"
elif git rev-parse --verify --quiet main >/dev/null 2>&1; then
  DIFF_BASE="main"
elif git rev-parse --verify --quiet "HEAD~1^{commit}" >/dev/null 2>&1; then
  DIFF_BASE="HEAD~1"
else
  echo "::error::T6: no base ref is resolvable (tried CI_T6_BASE, origin/main, main, HEAD~1)." >&2
  echo "::error::T6: the range to inspect is unknown, so no verdict can be reported." >&2
  echo "::error::T6: fetch the base (git fetch origin main) or pass CI_T6_BASE." >&2
  exit 2
fi

if ! git rev-parse --verify --quiet "${DIFF_BASE}^{commit}" >/dev/null 2>&1; then
  echo "::error::T6: base ref '$DIFF_BASE' does not resolve to a commit." >&2
  if [ "$DIFF_BASE_EXPLICIT" -eq 1 ]; then
    echo "::error::T6: it came from CI_T6_BASE, so no fallback is substituted." >&2
  fi
  exit 2
fi

# A base that resolves to the same commit as HEAD describes a range with no
# commits in it. The diff is then structurally empty, so the gate would reach
# "nothing to enforce" and print PASS — but an empty range proves nothing
# about the fix-without-test rule. Report the broken setup instead.
if [ "$(git rev-parse "${DIFF_BASE}^{commit}")" = "$(git rev-parse HEAD)" ]; then
  echo "::error::T6: base ref '$DIFF_BASE' resolves to HEAD, so the range is empty." >&2
  echo "::error::T6: an empty range cannot demonstrate the fix-without-test rule." >&2
  exit 2
fi

echo "==> T6 regression test check"
echo "    diff base: $DIFF_BASE"
echo "    HEAD:      $(git rev-parse --short HEAD)"

# Get fix(*) commits in the non-merge diff (commit subject starts with `fix`)
# Conventional commits: `fix(scope): subject` or `fix: subject`.
mapfile -t FIX_COMMIT_SUBJECTS < <(
  git log "$DIFF_BASE..HEAD" --no-merges --pretty=format:"%s" \
    | grep -E "^fix(\([^)]+\))?:" || true
)

if [ "${#FIX_COMMIT_SUBJECTS[@]}" -eq 0 ]; then
  echo "==> T6 PASS: no fix(*) commits in the diff. Nothing to enforce."
  exit 0
fi

echo "==> Found fix(*) commits: ${#FIX_COMMIT_SUBJECTS[@]}"
for s in "${FIX_COMMIT_SUBJECTS[@]}"; do
  echo "    - $s"
done

# Get all files changed in the PR-style diff (added or modified)
DIFF_FILES=$(git diff "$DIFF_BASE...HEAD" --name-only --diff-filter=AM 2>/dev/null \
  || git diff "$DIFF_BASE..HEAD" --name-only --diff-filter=AM 2>/dev/null \
  || echo "")

if [ -z "$DIFF_FILES" ]; then
  echo
  echo "==> T6 FAIL: fix(*) commits found but the diff is empty."
  echo "    An empty commit cannot supply a regression test — every fix(*)"
  echo "    must add, modify, or re-enable at least one test in the same PR"
  echo "    (per docs/TEST-PLAN.md §5)."
  exit 1
fi

# Test file patterns (L1 unit, L2 integration, L3 sandbox, L4 browser-e2e, openspec)
TEST_PATTERNS=(
  # L1 Rust unit tests
  '^crates/[^/]+/tests/.+'
  '^crates/[^/]+/src/.*test[s]?/.*'  # internal test modules
  # L2 Rust integration tests (next to main code)
  '^crates/[^/]+/tests/.+\.rs$'
  # L1/L2 vitest (TypeScript)
  '^apps/explorer-ui/.*\.(test|spec)\.ts$'
  '^apps/explorer-ui/.*\.(test|spec)\.tsx$'
  # L3 sandbox scenarios
  '^sandbox/manifests/.+\.yaml$'
  '^sandbox/manifests/.+\.yml$'
  # L4 browser E2E
  '^apps/explorer-ui/e2e/.+\.spec\.ts$'
  # Specs (also count as test artifacts when REQ-driven)
  '^openspec/.+\.md$'
  # Acceptance / harness
  '^crates/cognicode-rule-test-harness/.+'
  '^crates/cognicode-core/tests/.+'
)

# Patterns that do NOT count as tests (security review, doc, chore)
NON_TEST_PATH_HINTS=(
  '^docs/'
  'ADR.*\.md$'
  'ROADMAP\.md$'
  'CONTEXT\.md$'
  'CHANGELOG\.md$'
)

# A file counts as a test if it matches any TEST_PATTERN AND does NOT fall
# under docs/ROADMAP/CHANGELOG (those are documentation, not tests).
TEST_FILES_CHANGED=()
while IFS= read -r file; do
  [ -z "$file" ] && continue
  is_test=0
  for pat in "${TEST_PATTERNS[@]}"; do
    if echo "$file" | grep -qE "$pat"; then
      is_test=1
      break
    fi
  done
  if [ "$is_test" -eq 1 ]; then
    for hint in "${NON_TEST_PATH_HINTS[@]}"; do
      if echo "$file" | grep -qE "$hint"; then
        is_test=0
        break
      fi
    done
  fi
  if [ "$is_test" -eq 1 ]; then
    TEST_FILES_CHANGED+=("$file")
  fi
done <<< "$DIFF_FILES"

if [ "${#TEST_FILES_CHANGED[@]}" -eq 0 ]; then
  echo
  echo "==> T6 FAIL: fix(*) commits found but no test files changed in the diff."
  echo
  echo "fix(*) commits:"
  for s in "${FIX_COMMIT_SUBJECTS[@]}"; do
    echo "  - $s"
  done
  echo
  echo "All files changed (no tests among them):"
  while IFS= read -r f; do
    echo "  - $f"
  done <<< "$DIFF_FILES"
  echo
  echo "Per docs/TEST-PLAN.md §5, every fix(*) commit must add, modify, or"
  echo "re-enable at least one test in the same PR."
  exit 1
fi

echo "==> T6 PASS: ${#TEST_FILES_CHANGED[@]} test file(s) changed alongside fix(*)."
for f in "${TEST_FILES_CHANGED[@]}"; do
  echo "    - $f"
done
exit 0
