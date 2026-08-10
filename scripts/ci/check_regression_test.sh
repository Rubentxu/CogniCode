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
#   2  ERROR (could not determine base branch, no diff, etc.)

set -euo pipefail

# Resolve base branch (feature branch workflow): prefer origin/main, fall
# back to local main, then HEAD~1 to detect at least one commit back.
DIFF_BASE=""
if git rev-parse origin/main >/dev/null 2>&1; then
  DIFF_BASE="origin/main"
elif git rev-parse main >/dev/null 2>&1; then
  DIFF_BASE="main"
else
  DIFF_BASE="HEAD~1"
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
