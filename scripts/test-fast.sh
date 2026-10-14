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
ROADMAP_RATCHET="${REPO_ROOT}/scripts/ci/test_roadmap_version_ratchet.py"

# ---------- roadmap coherence ratchet ----------
# MEASURED 2026-10-14 (JOURNAL N+106): this ratchet was FAILING for 112
# commits because neither gate invoked it. It costs 0.08 s, which is why it
# belongs here and not only in the full battery: drift is cheapest to catch on
# the loop that runs after every change.
#
# It runs at the TOP, before the strategy dispatch, because every branch below
# (noop, fallback, suite loop) exits on its own. A check placed at the bottom
# would be skipped exactly when the selector degrades, which is when a
# developer most needs the other contracts to still hold.
if [ -f "$ROADMAP_RATCHET" ]; then
  printf '\n=== roadmap version ratchet ===\n'
  roadmap_rc=0
  python3 "$ROADMAP_RATCHET" || roadmap_rc=1
else
  roadmap_rc=0
fi

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

# MEDIDO 2026-10-14 (CD-1). El rc del selector NO puede descartarse: antes
# `2>/dev/null` lo tiraba y la salida vacia caia al fallback en silencio. Un
# selector roto producia un resultado que parecia una seleccion legitima.
if SELECTION="$(bash "$SELECTOR" --paths "$PATHS")"; then
  :
else
  selector_rc=$?
  echo "ERROR: el selector fallo (rc=$selector_rc): $SELECTOR" >&2
  exit 2
fi

# MEDIDO 2026-10-14 (CD-1). Los dos `sed` de antes devolvian
# SUITES='"core"' con las comillas del JSON dentro, de modo que ningun token
# casaba con el `case` de `run_suite` y el wrapper no ejecutaba ni una sola
# suite. Se parsea con python3, que ya es dependencia del script (ratchet,
# baseline) y que distingue "JSON malformado" de "seleccion vacia".
SELECTION_PARSED="$(printf '%s' "$SELECTION" | python3 -c '
import json, sys

raw = sys.stdin.read().strip()
if not raw:
    print("SELECTION_EMPTY", file=sys.stderr)
    sys.exit(3)
try:
    doc = json.loads(raw)
except json.JSONDecodeError as e:
    print(f"SELECTION_MALFORMED: {e}", file=sys.stderr)
    sys.exit(3)

strategy = doc.get("strategy")
suites = doc.get("suites")
if not isinstance(strategy, str) or not isinstance(suites, list):
    print("SELECTION_MALFORMED: faltan strategy o suites", file=sys.stderr)
    sys.exit(3)
if not all(isinstance(s, str) for s in suites):
    print("SELECTION_MALFORMED: suites no es una lista de cadenas", file=sys.stderr)
    sys.exit(3)

print("STRATEGY=" + strategy)
for s in suites:
    print("SUITE=" + s)
')"
parse_rc=$?
if [ "$parse_rc" -ne 0 ]; then
  echo "ERROR: salida del selector no interpretable (rc=$parse_rc)" >&2
  echo "  selector: $SELECTOR" >&2
  echo "  salida: ${SELECTION:-<vacia>}" >&2
  exit 2
fi

# `strategy` y `suites` se separan en el shell porque el bucle de abajo las
# necesita como variables planas; python3 se limita a leer el JSON.
STRATEGY="$(printf '%s\n' "$SELECTION_PARSED" | sed -n 's/^STRATEGY=//p')"
SUITE_LIST="$(printf '%s\n' "$SELECTION_PARSED" | sed -n 's/^SUITE=//p')"

printf 'selector → strategy=%s suites=[%s]\n' "$STRATEGY" "$(printf '%s' "$SUITE_LIST" | tr '\n' ',' | sed 's/,$//')"
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
  exit "$roadmap_rc"
fi

# MEDIDO 2026-10-14 (CD-1). strategy `changed` sin suites no es "nada que
# correr": es una seleccion que no se pudo resolver. Antes salia 0 sin ejecutar
# nada, que es el peor resultado posible. Cae al subset conservador, que si
# ejecuta tests reales, y se avisa de por que.
if [ "$STRATEGY" != "changed" ] || [ -z "$SUITE_LIST" ]; then
  echo "fallback: corriendo subset conservador (workspace --lib + clippy)."
  cargo test --workspace --lib --quiet
  cargo_test_rc=$?
  cargo clippy --workspace --all-targets --quiet -- -D warnings >/dev/null 2>&1
  clippy_rc=$?
  # The roadmap ratchet result is ANDed in, never ORed: a green subset must
  # not be able to hide a red contract.
  [ "$cargo_test_rc" -eq 0 ] && [ "$clippy_rc" -eq 0 ] && [ "$roadmap_rc" -eq 0 ] && exit 0 || exit 1
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
    # MEDIDO 2026-10-14 (CD-1). `return 0` hacia que un typo en el nombre de la
    # suite se convirtiera en PASS sin ejecutar nada. Una suite que el wrapper
    # no sabe correr es un fallo del contrato, no un caso vacio.
    *) echo "ERROR: suite desconocida: '$suite'" >&2; return 1 ;;
  esac
}

overall_rc=$roadmap_rc
seen=""
executed=0
while IFS= read -r s; do
  [ -z "$s" ] && continue
  case " $seen " in *" $s "*) continue ;; esac
  seen="$seen $s"
  printf '\n=== suite: %s ===\n' "$s"
  executed=$((executed + 1))
  if ! run_suite "$s"; then
    overall_rc=1
  fi
done <<< "$SUITE_LIST"

# MEDIDO 2026-10-14 (CD-1). Guarda de red: si el bucle no llego a ejecutar
# ninguna suite, el resultado no es un PASS, sea cual sea el camino por el que
# se llegase aqui. Con `changed` y una lista no vacia esto solo puede dispararse
# por un fallo de shell, y en ese caso tampoco es atribuible a los tests.
if [ "$executed" -eq 0 ]; then
  echo "ERROR: no se ejecuto ninguna suite (seleccion vacia o bucle sin iterar)" >&2
  exit 2
fi

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