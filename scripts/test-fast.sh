#!/usr/bin/env bash
# scripts/test-fast.sh
#
# Suite selectiva para evolutivos. Detecta paths modificados (working
# tree + index vs HEAD), los pasa al selector CR-08 y corre SOLO las
# suites afectadas. Aplica baseline `scripts/known_failures.yaml` para
# distinguir regresiones reales del ruido conocido.
#
# Uso:
#   bash scripts/test-fast.sh                  # auto: git diff vs HEAD
#   bash scripts/test-fast.sh --paths "a.rs b.rs"
#   bash scripts/test-fast.sh --full           # fuerzo batería completa
#   bash scripts/test-fast.sh --dry-run        # solo imprime el plan
#   bash scripts/test-fast.sh --no-baseline    # omite check_known_failures
#
# Política contractual: ver AGENTS.md § "Estrategia de tests".
set -uo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "${REPO_ROOT}" ]; then
  echo "ERROR: no se ejecuta dentro de un repositorio git" >&2
  exit 2
fi
cd "${REPO_ROOT}"

SELECTOR="${REPO_ROOT}/scripts/ci/select-suites.sh"
BASELINE="${REPO_ROOT}/scripts/known_failures.yaml"
BASELINE_CHECK="${REPO_ROOT}/scripts/check_known_failures.py"

# ---------- args ----------
MODE="auto"
DRY_RUN="false"
USE_BASELINE="true"
EXTRA_PATHS=""

while [ $# -gt 0 ]; do
  case "$1" in
    --paths)   EXTRA_PATHS="$2"; shift 2 ;;
    --full)    MODE="full"; shift ;;
    --dry-run) DRY_RUN="true"; shift ;;
    --no-baseline) USE_BASELINE="false"; shift ;;
    -h|--help)
      sed -n '2,/^set -uo/s/^# \{0,1\}//p' "$0" | head -25
      exit 0
      ;;
    *) echo "ERROR: flag desconocida: $1" >&2; exit 2 ;;
  esac
done

# ---------- detectar paths ----------
detect_paths() {
  # working + staged vs HEAD; si no hay HEAD (repo recién clonado), usar
  # todo el árbol contra main como aproximación.
  local paths
  paths="$(git diff --name-only HEAD 2>/dev/null | tr '\n' ' ')"
  if [ -z "$(printf '%s' "$paths" | tr -d '[:space:]')" ]; then
    paths="$(git diff --cached --name-only 2>/dev/null | tr '\n' ' ')"
  fi
  if [ -z "$(printf '%s' "$paths" | tr -d '[:space:]')" ]; then
    # working tree dirty vs HEAD sin nada: caer a HEAD~1..HEAD
    paths="$(git diff --name-only HEAD~1 HEAD 2>/dev/null | tr '\n' ' ')"
  fi
  if [ -n "$EXTRA_PATHS" ]; then
    paths="${paths} ${EXTRA_PATHS}"
  fi
  printf '%s' "$paths"
}

PATHS="$(detect_paths)"

# ---------- invocar selector ----------
if [ ! -x "$SELECTOR" ]; then
  echo "ERROR: selector no encontrado o no ejecutable: $SELECTOR" >&2
  exit 2
fi

SELECTION="$(bash "$SELECTOR" --paths "$PATHS" 2>/dev/null)"
STRATEGY="$(printf '%s' "$SELECTION" | sed -n 's/.*"strategy":"\([^"]*\)".*/\1/p')"
SUITES="$(printf '%s' "$SELECTION" | sed -n 's/.*"suites":\[\(.*\)\].*/\1/p')"

printf 'selector → strategy=%s suites=[%s]\n' "$STRATEGY" "$SUITES"
printf '  paths: %s\n' "${PATHS:-<empty>}"

if [ "$DRY_RUN" = "true" ]; then
  echo "--- plan ---"
  echo "$SELECTION"
  exit 0
fi

# Modo --full: delegar a test-full.sh
if [ "$MODE" = "full" ]; then
  exec bash "${REPO_ROOT}/scripts/test-full.sh" --reason "test-fast --full"
fi

# Strategy noop: nada que correr
if [ "$STRATEGY" = "noop" ]; then
  echo "noop: cambios no impactan suites (solo metadata)."
  exit 0
fi

# Fallback o strategy vacía → ejecutar subset conservador
if [ "$STRATEGY" != "changed" ] || [ -z "$SUITES" ]; then
  echo "fallback: corriendo subset conservador (workspace --lib + clippy)."
  cargo test --workspace --lib --quiet
  cargo_clippy_rc=$?
  cargo clippy --workspace --all-targets --quiet -- -D warnings >/dev/null 2>&1
  clippy_rc=$?
  [ "$cargo_clippy_rc" -eq 0 ] && [ "$clippy_rc" -eq 0 ] && exit 0 || exit 1
fi

# ---------- mapear suites a comandos ----------
run_suite() {
  local suite="$1"
  case "$suite" in
    core)     cargo test -p cognicode-core --lib ;;
    explorer) cargo test -p cognicode-explorer --lib ;;
    mcp)      cargo test -p cognicode-mcp --lib ;;
    cli)      cargo test -p cognicode-cli --lib ;;
    ladybug)  cargo test -p cognicode-ladybug --lib ;;
    merge)
      # En fast NO lanzamos la batería completa: los integration tests
      # de release-flow son lentos (3+ min) y ya están cubiertos por
      # `scripts/test-full.sh`. Aquí solo validamos que las libs del
      # workspace compilan y pasan sus tests unitarios.
      cargo test --workspace --lib --quiet
      ;;
    *) echo "WARN: suite desconocida: $suite" >&2; return 0 ;;
  esac
}

overall_rc=0
seen=""
for s in $(printf '%s' "$SUITES" | tr ',' ' '); do
  case " $seen " in *" $s "*) continue ;; esac
  seen="$seen $s"
  printf '\n=== suite: %s ===\n' "$s"
  if ! run_suite "$s"; then
    overall_rc=1
  fi
done

# ---------- baseline check (core) ----------
if [ "$USE_BASELINE" = "true" ] && [ -f "$BASELINE_CHECK" ] && [ -f "$BASELINE" ]; then
  if printf ' %s ' "$seen" | grep -q ' core '; then
    printf '\n=== baseline: check_known_failures (core/lib) ===\n'
    if ! python3 "$BASELINE_CHECK" --package cognicode-core --target lib; then
      overall_rc=1
    fi
  fi
fi

exit "$overall_rc"