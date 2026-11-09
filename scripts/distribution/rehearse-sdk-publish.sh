#!/usr/bin/env bash
# Rehearse publishing the four SDK library crates without uploading anything.
#
# One `cargo publish --dry-run` invocation names all four crates. Cargo packages
# them in dependency order and verifies each one against the packaged copies of
# the crates before it, so a crate whose dependency is not yet on the registry
# still verifies. Nothing is uploaded and no credential is read.
#
# Usage: bash scripts/distribution/rehearse-sdk-publish.sh
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
target="${CARGO_TARGET_DIR:-$(mktemp -d)/target}"
export CARGO_TARGET_DIR="$target"

cd "$root"
echo "rehearsal target dir: $CARGO_TARGET_DIR" >&2

cargo publish --dry-run --locked \
  -p cognicode-graph-algos \
  -p cognicode-macros \
  -p cognicode-core \
  -p cognicode-graph-wasm
