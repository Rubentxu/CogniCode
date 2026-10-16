#!/usr/bin/env bash
# cargo-iso.sh — run cargo with an explicit CARGO_TARGET_DIR that survives the
# `~/.cargo/config.toml` global override.
#
# Why this exists
# ---------------
# `~/.cargo/config.toml` on this machine sets
# `build.target-dir = /var/home/rubentxu/cargo-targets`, shared by every
# checkout of this repo plus every other Rust project the user has open.
# Cargo decides freshness by mtime, and **a mtime does not say which checkout
# a binary came from**. The merge gate measured two runs in 2026-10-15 that
# failed for this reason and not for the code under test:
#
#   * `core_gate_coverage_contract.rs:900` named `the_rustdoc_gate_runs_in_*
#     the_merge_authority`, a test that does not exist in this tree (861
#     lines on disk; the binary that panicked had a 900-line version from
#     another checkout).
#   * `a014_capabilities_json_*` compared `source_commit (d5082848…) must
#     match git rev-parse HEAD (d73e7b08…)`. `d73e7b08` is the HEAD of
#     `worktree/test-harness`, not of this tree.
#
# The detection-only fix (`scripts/ci/check-build-isolation.sh`) refuses
# the gate when another checkout is mid-build. That is a guard rail, not an
# isolation. A run can pass the preflight and still write into the shared
# target dir; if a sibling starts compiling between preflight and the last
# cargo call, contamination is still possible.
#
# The only complete fix is to give this run its own target dir.
# `CARGO_TARGET_DIR=/tmp/...` makes that happen; the rest of the system
# (`cargo metadata`, `scripts/ci/target-dir.sh`, `check-build-isolation.sh`)
# already honours the variable. This script is a small wrapper so a
# pipeline stage can opt into isolation without rewriting the surrounding
# shell.
#
# Modes
# -----
#   * `--gate`  : auto-compute a per-checkout target dir if none is set.
#                  Use this from CI/gate scripts so verdicts are
#                  attributable regardless of who else is compiling.
#   * no flag   : pass through. Use this from dev workflows where shared
#                  cache is the point.
#
# Usage
# -----
#   scripts/ci/cargo-iso.sh --gate cargo build --release
#   scripts/ci/cargo-iso.sh cargo test -p cognicode-core
#   CARGO_TARGET_DIR=/tmp/x scripts/ci/cargo-iso.sh --gate cargo build
#
# Exit code is cargo's. The target dir is exported (not just set) so child
# processes that recurse through rustc inherit it.
set -euo pipefail

if [[ "${1:-}" == "--gate" ]]; then
    shift
    if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
        repo_root="$(git rev-parse --show-toplevel 2>/dev/null || printf '%s' "$PWD")"
        # Stable per-checkout identifier (md5 of the canonical path). Two
        # checkouts always hash differently; the same checkout always
        # hashes the same. Subsequent runs of the same checkout share
        # the cache.
        check_id="$(printf '%s' "$repo_root" | md5sum | cut -c1-12)"
        export CARGO_TARGET_DIR="/tmp/cognicode-gate-${check_id}/target"
        mkdir -p "$CARGO_TARGET_DIR"
        echo "cargo-iso[--gate]: CARGO_TARGET_DIR=$CARGO_TARGET_DIR" >&2
    fi
elif [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
    :  # no flag, no override; pass through, shared cache stays shared
fi

exec cargo "$@"