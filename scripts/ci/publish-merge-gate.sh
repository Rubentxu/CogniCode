#!/usr/bin/env bash
# publish-merge-gate.sh — run the merge authority and report its verdict to GitHub.
#
# WHY THIS EXISTS
# ---------------
# `main` requires a status check named `merge-gate`, with `strict: true`, so
# that a merge waits for it and a branch that falls behind has to re-run it.
# That check has always come from `.github/workflows/pr-ci.yml`. The cutover is
# deleting that file, and on the day it is deleted the pull request that deletes
# it cannot produce its own required check — the workflow is gone, so nothing
# posts the context, and the pull request is blocked forever.
#
# PipelineK 0.46.0 publishes no commit status. Measured: it is a local JVM
# runner (`pipeline <validate|run>`, with sqlite, `scm-git` and local
# credentials jars) and the binary contains no reference to GitHub, commit
# statuses or a webhook. So the bridge has to be here, in the repository, and
# it has to be small enough to read in one sitting.
#
# WHAT IT DOES NOT DO
# -------------------
# It does not re-implement the gate. It runs the gate and reports what the gate
# said. There is one implementation of what a merge has to pass, and it is
# `merge-gate.pipeline.kts`; this script cannot report a pass without that file
# having returned 0, because the exit code it publishes is the exit code it got.
#
# WHAT THIS COSTS, STATED PLAINLY
# -------------------------------
# A status posted by a script is an assertion by whoever ran the script. GitHub
# can verify that a check named `merge-gate` reported success; it cannot verify
# that a pipeline ran to produce it. Under Actions the check was only posted by
# a workflow triggered on the pull request, so a contributor could not satisfy
# it by asserting. Here, someone with a token can.
#
# That is the honest price of removing the orchestrator, and it is not hidden:
#   * the published description names the PipelineK database the run wrote to,
#     so the verdict is checkable after the fact and not just assertable;
#   * the run database is kept in the repository, not in a temporary directory;
#   * the operator who publishes is the operator who runs the gate, which is
#     the same person who would have pushed the failing commit.
#
# It is a narrower guarantee than "only a workflow triggered on this pull
# request can report this". If that is not an acceptable trade, the alternative
# is to keep a minimal workflow, which is a decision for the maintainer and not
# one this script makes.
#
# Usage:
#   bash scripts/ci/publish-merge-gate.sh [<sha>] [<repo>]
#
# Defaults to HEAD and the repository's origin remote. Exits with the lane's
# exit code, so a caller using `set -e` still stops on a red gate.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

# The context name `main` requires. It is spelled here, once, and
# `test_publish_merge_gate_status.py` holds it against the branch protection
# rule — because a bridge that publishes under the wrong name is a bridge that
# publishes nothing, and it would fail silently in the only place that matters.
MERGE_GATE_CONTEXT="merge-gate"

# Kept in the repository so the verdict is auditable after the fact, which is
# the only thing that makes a posted status worth anything.
PIPELINE_DB=".pipelinek/merge-gate.db"

sha="${1:-HEAD}"
repo="${2:-}"

if [ -z "$repo" ]; then
    origin="$(git remote get-url origin 2>/dev/null || true)"
    # `git@github.com:owner/name.git` and `https://github.com/owner/name.git`
    # are the two spellings that appear in practice; either is enough to get
    # `owner/name`, which is all the statuses API needs.
    repo="$(printf '%s' "$origin" | sed -E 's#^git@[^:]+:##; s#^https?://[^/]+/##; s#\.git$##')"
fi
if [ -z "$repo" ] || [ "$repo" = "$origin" ]; then
    echo "ERROR: cannot work out owner/name from the origin remote ($origin)." >&2
    echo "       Pass it explicitly: publish-merge-gate.sh <sha> <owner/name>" >&2
    exit 2
fi

if ! command -v pipelinek >/dev/null 2>&1; then
    echo "ERROR: the \`pipelinek\` binary is not on PATH." >&2
    echo "       This script does not know how to decide the verdict; it only" >&2
    echo "       reports the one the merge authority gives." >&2
    exit 2
fi
if ! command -v gh >/dev/null 2>&1; then
    echo "ERROR: the \`gh\` CLI is not on PATH, so there is no way to report." >&2
    exit 2
fi

resolved="$(git rev-parse "$sha")"
echo "==> running the merge authority for $resolved"
echo "    database: $PIPELINE_DB"
mkdir -p "$(dirname "$PIPELINE_DB")"

set +e
pipelinek run --db "$PIPELINE_DB" merge-gate.pipeline.kts
lane_rc=$?
set -e

if [ "$lane_rc" -eq 0 ]; then
    state="success"
    description="merge-gate.pipeline.kts passed on $resolved (pipelinek, db: $PIPELINE_DB)"
else
    state="failure"
    description="merge-gate.pipeline.kts failed with exit $lane_rc on $resolved (pipelinek, db: $PIPELINE_DB)"
fi

echo "==> publishing $MERGE_GATE_CONTEXT=$state to $repo@$resolved"
gh api --method POST "repos/$repo/statuses/$resolved" \
    -f "state=$state" \
    -f "context=$MERGE_GATE_CONTEXT" \
    -f "description=$description" \
    -f "target_url=https://github.com/$repo/commit/$resolved" \
    --silent

echo "==> $MERGE_GATE_CONTEXT=$state published; the gate's own exit code was $lane_rc"
exit "$lane_rc"
