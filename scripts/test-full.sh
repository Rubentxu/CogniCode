#!/usr/bin/env bash
# scripts/test-full.sh
#
# Batería completa para releases. Ejecuta:
#   1. cargo fmt --check
#   2. cargo clippy --workspace --all-targets -- -D warnings
#   3. cargo test --workspace
#   4. cargo test --workspace --doc
#   5. check_known_failures sobre cognicode-core/lib
#   6. ratchet de coherencia del roadmap (N+106: antes no lo invocaba nadie)
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
ROADMAP_RATCHET="${REPO_ROOT}/scripts/ci/test_roadmap_version_ratchet.py"

# MEDIDO 2026-10-14 (N+114): `check_known_failures.py` nace con un flag
# `--tmpdir-root` que nadie usaba, y sin el el paso cae en su rama de fallo
# (rc=2) en cualquier maquina donde `TMPDIR` cruce un symlink — aqui
# `/home -> /var/home`. Se calcula una raiz utilizable en vez de supeditar
# el gate a acordarse de exportar una variable.
#
# El orden importa: primero el `TMPDIR` heredado si es valido (respeta la
# intencion del operador), luego `/tmp` y `/var/tmp`. Ningun path de esta
# maquina esta escrito aqui a proposito: son rutas del sistema, no rutas
# que dependan de conocer esta instalacion.
TMPDIR_ROOT="$(
  python3 - "$TMPDIR" <<'PY'
import os
import sys


def usable(path: str | None) -> str | None:
    if not path:
        return None
    absolute = os.path.abspath(path)
    if os.path.realpath(absolute) != absolute:
        return None
    if not (os.path.isdir(absolute) and os.access(absolute, os.W_OK)):
        return None
    return absolute


for candidate in (sys.argv[1] if len(sys.argv) > 1 else None, "/tmp", "/var/tmp"):
    resolved = usable(candidate)
    if resolved:
        print(resolved)
        break
PY
)"
if [ -z "$TMPDIR_ROOT" ]; then
  echo "WARNING: no se encontro una raiz TMPDIR sin symlink; el paso" \
       "known_failures fallara con rc=2" >&2
fi

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
  if [ -n "$TMPDIR_ROOT" ]; then
    run_step "known_failures (core/lib)" \
      python3 "$BASELINE_CHECK" --package cognicode-core --target lib \
      --tmpdir-root "$TMPDIR_ROOT"
  else
    # Sin raiz utilizable no hay redireccion que ofrecer: se invoca tal cual
    # y que el propio script decida. No se enmascara el fallo devolviendo 0.
    run_step "known_failures (core/lib)" \
      python3 "$BASELINE_CHECK" --package cognicode-core --target lib
  fi
fi

# MEASURED 2026-10-14 (JOURNAL N+106): this ratchet existed and was FAILING
# since 1c664066 — 112 commits earlier — precisely because nothing invoked
# it. A contract that no gate runs is a comment with a shebang. It is here so
# the next drift of docs/roadmap/CURRENT.md is caught by a command that
# already runs, not by a human noticing months later.
if [ -f "$ROADMAP_RATCHET" ]; then
  run_step "roadmap version ratchet" python3 "$ROADMAP_RATCHET"
fi

printf '\n=== test-full rc=%d ===\n' "$overall_rc"
exit "$overall_rc"