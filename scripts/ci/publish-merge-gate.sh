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
    if [ -z "$repo" ] || [ "$repo" = "$origin" ]; then
        echo "ERROR: cannot work out owner/name from the origin remote ($origin)." >&2
        echo "       Pass it explicitly: publish-merge-gate.sh <sha> <owner/name>" >&2
        exit 2
    fi
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

# Gate attribution fix (2026-10-16). On machines where `~/.cargo/config.toml`
# sets `build.target-dir` to a shared location, cargo writes every invocation's
# build output to the same directory regardless of which checkout invoked it.
# Six checkouts of this repo do that here, and cargo deciding freshness by
# mtime means a binary from a sibling checkout can be executed as if it were
# this tree's — measured twice on 2026-10-15. The merge gate needs verdicts
# that are attributable to the SHA being checked; if another checkout is
# compiling, the verdict can be polluted mid-run. The detection-only fix in
# `scripts/ci/check-build-isolation.sh` refuses the run when a sibling
# process is visible; the *isolation* is `CARGO_TARGET_DIR` per run, which
# propagates through pipelinek into every sh() subprocess and the cargo
# invocations inside them (probe: INSIDE_PROBE shows the variable and
# `cargo metadata` reports the same target dir). Set it once here unless the
# caller has already done so.
if [ -z "${CARGO_TARGET_DIR:-}" ]; then
    check_id="$(git rev-parse --show-toplevel | md5sum | cut -c1-12)"
    export CARGO_TARGET_DIR="/tmp/cognicode-merge-gate-${check_id}/target"
    mkdir -p "$CARGO_TARGET_DIR"
    echo "    target dir: $CARGO_TARGET_DIR (per-checkout isolation)"
fi

set +e
# MEDIDO 2026-10-15. Without `--rerun` the runtime serves stages from the
# SQLite journal. Measured on a real run: 30 stages — clippy, four release
# builds and all — replayed in 79 ms, and the journal marked every step
# `RERUN`. The only steps that actually executed were the ones that failed.
#
# A check that republishes a replayed verdict is not evidence about the
# tree it names. The bridge already costs a full gate run every time it is
# invoked; caching on top of that is the one saving that turns it into a
# rubber stamp. `--rerun` pays the full price and means the published
# status corresponds to the SHA in its own description.
pipelinek run --rerun --db "$PIPELINE_DB" merge-gate.pipeline.kts
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
