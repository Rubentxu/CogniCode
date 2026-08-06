#!/usr/bin/env bash
# prune_stale_branches.sh — remove stale git branches (dry-run by default).
#
# Policy:
#   - REMOTE branches merged into origin/main are candidates (whitelist excluded).
#   - Whitelist: main, master, HEAD, feat/e30-*, fix/e30-*, and any branch with
#     an OPEN pull request (queried via gh).
#   - LOCAL toxic pre-E29 branches are removed with --apply (explicit list).
# Usage:
#   bash sandbox/scripts/prune_stale_branches.sh            # dry-run
#   bash sandbox/scripts/prune_stale_branches.sh --apply    # delete
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

DRY_RUN=1
if [ "${1:-}" = "--apply" ]; then
    DRY_RUN=0
fi

open_pr_branches() {
    gh pr list --json headRefName --jq '.[].headRefName' 2>/dev/null || true
}

merged_remote() {
    git branch -r --merged origin/main | sed 's/^[* ]*//' | grep -v '^origin/main$' | grep -v '^origin/HEAD' || true
}

is_whitelisted() {
    local b="$1"
    case "$b" in
        main|master|HEAD) return 0 ;;
        origin/main|origin/HEAD) return 0 ;;
        feat/e30-*|fix/e30-*) return 0 ;;
    esac
    return 1
}

echo "=== Remote branches merged into origin/main (candidates) ==="
PRS="$(open_pr_branches)"
COUNT=0
for b in $(merged_remote); do
    short="${b#origin/}"
    if is_whitelisted "$short"; then
        echo "  SKIP (whitelist): $short"
        continue
    fi
    if echo "$PRS" | grep -qx "$short"; then
        echo "  SKIP (open PR): $short"
        continue
    fi
    echo "  PRUNE: $short"
    if [ "$DRY_RUN" = "0" ]; then
        git push origin --delete "$short" >/dev/null 2>&1 && echo "    deleted" || echo "    FAILED"
    fi
    COUNT=$((COUNT + 1))
done
echo "  ($COUNT candidates)"

echo
echo "=== Local toxic pre-E29 branches (explicit list) ==="
LOCAL_TOXIC="feat/e12g-risk-map feat/e12h-decision-trace feat/e14-narrative-runtime feat/e14-narrative-runtime-cycle-2 feat/e19-4-composed-narrative feat/e21-1-investigation-entity feat/e28-1-pr2-plan-algebra feat/e28-5-structural-analytics-cohort-2 feat/e29-6-ladybug-store-wiring feat/relation-candidates-v1 fix/composed-narrative-provider-applies-to impl/e13-investigation-scope-integration-test"
for b in $LOCAL_TOXIC; do
    if git show-ref --verify --quiet "refs/heads/$b" 2>/dev/null; then
        if echo "$PRS" | grep -qx "$b"; then
            echo "  SKIP (open PR): $b"
            continue
        fi
        echo "  PRUNE (local): $b"
        if [ "$DRY_RUN" = "0" ]; then
            git branch -D "$b" >/dev/null 2>&1 && echo "    deleted" || echo "    FAILED"
        fi
    fi
done

if [ "$DRY_RUN" = "1" ]; then
    echo
    echo "DRY-RUN — no branches deleted. Re-run with --apply to execute."
fi
