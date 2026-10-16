#!/usr/bin/env bash
# pipelinek-iso.sh — run pipelinek in an isolated CARGO_TARGET_DIR.
#
# Same rationale as `cargo-iso.sh`: cargo resolves its target dir from the
# shared `~/.cargo/config.toml` override, and that override is shared by
# every checkout of every Rust project on this machine. PipelineK spawns
# sh() subprocesses that inherit env vars from the JVM; the only thing we
# have to do here is set the env var before invoking pipelinek and the
# subprocess tree inherits it.
#
# Usage:
#   scripts/ci/pipelinek-iso.sh run --rerun merge-gate.pipeline.kts
#   scripts/ci/pipelinek-iso.sh run release-candidate.pipeline.kts
#   CARGO_TARGET_DIR=/tmp/x scripts/ci/pipelinek-iso.sh run release.pipeline.kts
#
# Per-checkout isolation: stable across runs (md5 of repo root) so the
# cache survives subsequent invocations of the same checkout. Different
# checkouts hash differently so they cannot read each other's binaries.
set -euo pipefail

if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
    repo_root="$(git rev-parse --show-toplevel 2>/dev/null || printf '%s' "$PWD")"
    check_id="$(printf '%s' "$repo_root" | md5sum | cut -c1-12)"
    export CARGO_TARGET_DIR="/tmp/cognicode-pipelinek-${check_id}/target"
    mkdir -p "$CARGO_TARGET_DIR"
    echo "pipelinek-iso: CARGO_TARGET_DIR=$CARGO_TARGET_DIR" >&2
fi

exec pipelinek "$@"