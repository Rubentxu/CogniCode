#!/usr/bin/env bash
# scripts/test-full.sh
#
# Batería completa para releases. Ejecuta:
#   1. cargo fmt --check
#   2. cargo clippy --workspace --all-targets -- -D warnings
#   3. cargo test --workspace
#   4. cargo test --workspace --doc
#   5. check_known_failures sobre cognicode-core/lib
#
# Política contractual: ver AGENTS.md § "Estrategia de tests".
# Modo de uso: antes de tag / antes de PR a main / antes de bump SemVer.
set -uo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "${REPO_ROOT}" ]; then
  echo "ERROR: no se ejecuta dentro de un repositorio git" >&2
  exit 2
fi
cd "${REPO_ROOT}"

REASON="${1:-manual}"
BASELINE_CHECK="${REPO_ROOT}/scripts/check_known_failures.py"
BASELINE="${REPO_ROOT}/scripts/known_failures.yaml"

printf '=== test-full (%s) @ %s ===\n' "$REASON" "$(git rev-parse --short HEAD)"
overall_rc=0

run_step() {
  local label="$1"; shift
  printf '\n--- %s ---\n' "$label"
  if "$@"; then
    printf 'OK: %s\n' "$label"
  else
    rc=$?
    printf 'FAIL (rc=%d): %s\n' "$rc" "$label"
    overall_rc=1
  fi
}

run_step "fmt --check"           bash -c "cargo fmt --all --check"
run_step "clippy -D warnings"    bash -c "cargo clippy --workspace --all-targets -- -D warnings"
run_step "test --workspace"      bash -c "cargo test --workspace"
run_step "test --workspace --doc" bash -c "cargo test --workspace --doc"

if [ -f "$BASELINE_CHECK" ] && [ -f "$BASELINE" ]; then
  run_step "known_failures (core/lib)" \
    python3 "$BASELINE_CHECK" --package cognicode-core --target lib
fi

printf '\n=== test-full rc=%d ===\n' "$overall_rc"
exit "$overall_rc"