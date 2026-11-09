#!/usr/bin/env bash
# Proves the published SDK crates install and build from crates.io in a clean
# consumer project. Run it only AFTER the operator has published the crates in
# the order documented in docs/distribution/SDK_RELEASE.md. It never publishes
# and never logs in: it only reads from the registry.
set -euo pipefail

if [[ "${1:-}" != "--confirm-published" ]]; then
  echo "refusing to run: pass --confirm-published once the four crates are on crates.io" >&2
  exit 2
fi

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
version="$(sed -n '/^\[workspace.package\]/,/^\[/{s/^version *= *"\(.*\)"/\1/p}' "$root/Cargo.toml" | head -n1)"
[[ -n "$version" ]] || { echo "cannot read the workspace version" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
export CARGO_TARGET_DIR="$work/target"

cd "$work"
cargo new --bin consumer --quiet
cd consumer
for crate in cognicode-graph-algos cognicode-macros cognicode-core cognicode-graph-wasm; do
  cargo add "$crate" --version "=$version" --quiet
done
cp "$root/crates/cognicode-graph-algos/examples/page_rank.rs" src/main.rs

# The lockfile must resolve every crate from the registry, not from a path.
cargo generate-lockfile
if grep -q '^source = "path' Cargo.lock; then
  echo "a dependency resolved from a path" >&2
  exit 1
fi
cargo run --quiet
echo "consumer verification passed for version $version"
