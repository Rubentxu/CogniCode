#!/usr/bin/env bash
# scripts/test-full.sh
#
# Batería completa para releases. Ejecuta:
#   1. cargo fmt --check
#   2. cargo clippy --workspace --all-targets -- -D warnings
#   3. cargo test --workspace
#   4. cargo test --workspace --doc
#   5. scripts/ci/run-all-contracts.sh
#   6. check_known_failures sobre cognicode-core/lib
#
# El paso 5 se anadio el 2026-10-18; antes, este script era un subconjunto
# del gate de merge y un verde aqui no significaba "mergable". Ver el
# comentario junto al paso.
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

# MEDIDO 2026-10-18. Este paso no estaba aqui, y su ausencia hacia que un
# `test-full` VERDE no significara "esto se puede mergear". El gate de
# merge corre `scripts/ci/run-all-contracts.sh`; `test-full.sh` no. Medido en
# db1f0c32: la bateria Rust rc=0 en los cinco pasos, y las superficies
# publicas seguian declarando v0.101.22 mientras el workspace estaba en
# 0.101.23 — README (release actual, binarios, tag, pin de mise, pin de
# install), CHANGELOG y SECURITY.md. El battery Rust no puede ver ninguna de
# esas siete afirmaciones, porque ninguna vive en codigo compilado.
#
# El modo de fallo que produce la diferencia es peor que un rojo: el
# operador ve verde, hace push, y descubre el rojo en el merge gate, donde el
# gate real ya fallo y el trabajo esta a medias. Un gate que es un subconjunto
# del gate que de verdad decide no es un gate previo; es una estimacion.
#
# La suite es un superconjunto, no una sustitucion: anade cobertura, no quita
# ninguna. El mismo script, el mismo codigo, un paso mas.
run_step "contracts"             bash -c "bash scripts/ci/run-all-contracts.sh"

if [ -f "$BASELINE_CHECK" ] && [ -f "$BASELINE" ]; then
  run_step "known_failures (core/lib)" \
    python3 "$BASELINE_CHECK" --package cognicode-core --target lib
fi

printf '\n=== test-full rc=%d ===\n' "$overall_rc"
exit "$overall_rc"