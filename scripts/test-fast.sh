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
# Strip JSON string quotes that the sed captures verbatim — the inner
# content of ["core","mcp"] is "core","mcp" (with literal quotes), which
# must be normalized before the case dispatch. The tr handles both single
# and multiple elements.
SUITES="$(printf '%s' "$SELECTION" | sed -n 's/.*"suites":\[\([^]]*\)\].*/\1/p' | tr -d '"')"

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
  # MEDIDO 2026-10-15. Este camino se salta `run_suite` entero, asi que con
  # el fallback del selector NUNCA se ejecutan las suites listadas — y la
  # lista del fallback incluye `merge`, que es el merge-gate completo. El
  # subconjunto conservador es lo que AGENTS.md prescribe para fallback
  # ("corre el subset conservador y avisa — no falla"), asi que el rc se
  # respeta; lo que no se respeta es que el aviso no decia en voz alta que
  # ningun test contractual de integracion se ha corrido. Este wrapper no es
  # el gate: el gate es `bash scripts/test-full.sh`.
  echo "  AVISO: este subconjunto NO incluye los tests de integracion"
  echo "  contractuales (b5_*, b6_*, prf_*). Para eso: bash scripts/test-full.sh"
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
      # `merge` es, segun el propio selector, "merge-gate completo (e2e +
      # pineados)" (scripts/ci/select-suites.sh). Su comando correcto es
      # `bash scripts/test-full.sh`, y no existe un comando cargo que sea eso:
      # son las stages del lane, no una suite. Este caso corria
      # `--workspace --lib` en su nombre y devolvia verde, que es reportar
      # exito sin haber ejecutado ni un test contractual de integracion
      # (b5_*, b6_*, prf_* — hay 65 en crates/cognicode-core/tests/).
      # Un gate que no puede decir que no, aqui, no puede decir nada.
      # Fallar cerrado y decir cual es el comando correcto.
      printf 'ERROR: la suite `merge` no se puede correr desde fast.\n' >&2
      printf '  correcto:  bash scripts/test-full.sh\n' >&2
      printf '  `merge` es el merge-gate completo (e2e + pineados) segun\n' >&2
      printf '  scripts/ci/select-suites.sh; fast no lo sustituye por --lib.\n' >&2
      return 1
      ;;
    *)
      # MEDIDO 2026-10-15. Esto era `WARN` + `return 0`: una suite que el
      # selector emite y el runner no conoce se saltaba EN VERDE. Un WARN
      # no cambia el rc, asi que el unico efecto era escribir en stderr,
      # y stderr no es un veredicto. Una divergencia entre CR-08 y este
      # wrapper es un fallo de cableado, no una novedad: o el wrapper no
      # la sabe correr, o el selector no deberia emitirla.
      printf 'ERROR: suite desconocida para %s: `%s`\n' "$0" "$suite" >&2
      printf '  La emitio select-suites.sh y este wrapper no la ejecuta.\n' >&2
      printf '  Anade el caso en run_suite() o quita la suite del selector.\n' >&2
      return 1
      ;;
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