#!/usr/bin/env bash
# Rust micro-fixture smoke test
set -euo pipefail
cd "$(dirname "$0")"

echo "=== Rust Micro-Fixture Tests ==="
cargo test --lib 2>&1
cargo build 2>&1
echo "=== OK: All checks passed ==="
