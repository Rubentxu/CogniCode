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
# Motor de selección (opt-in, por defecto sin cambios):
#   TEST_FAST_ENGINE=cr08     (por defecto) selector CR-08 + runner de suites.
#   TEST_FAST_ENGINE=harness  plan/run del test harness con el suelo del oráculo.
#                             Si CR-08 pide un scope que la lane `change` no
#                             ejecuta (hoy `explorer`, que solo corre en
#                             integración), cae al motor CR-08 en vez de
#                             ejecutar de menos. La paridad está medida en
#                             `harness parity --corpus` y comprobada por
#                             scripts/ci/test_fast_engine_parity.py.
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
ENGINE="${TEST_FAST_ENGINE:-cr08}"

case "$ENGINE" in
  cr08|harness) ;;
  *) echo "ERROR: TEST_FAST_ENGINE desconocido: $ENGINE (cr08|harness)" >&2; exit 2 ;;
esac

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
# BASE_REF es la base que corresponde a los paths detectados, y se pasa al motor
# harness para que su ChangeSet describa el mismo cambio. Sin esto, un árbol
# limpio daba al harness un cambio vacío (base HEAD) mientras el wrapper miraba
# HEAD~1..HEAD: dos motores midiendo cosas distintas es exactamente lo que la
# paridad de este wrapper existe para evitar.
detect_paths() {
  # Imprime DOS líneas: la base que corresponde al cambio y los paths. La base
  # se devuelve así, y no en una variable, porque esta función se invoca en una
  # sustitución de comando: una asignación dentro del subshell no llega al
  # shell padre, y el harness acababa planificando con base HEAD un cambio que
  # el wrapper había leído como HEAD~1..HEAD.
  local base_ref="HEAD"
  local paths
  paths="$(git diff --name-only HEAD 2>/dev/null | tr '\n' ' ')"
  if [ -z "$(printf '%s' "$paths" | tr -d '[:space:]')" ]; then
    paths="$(git diff --cached --name-only 2>/dev/null | tr '\n' ' ')"
  fi
  if [ -z "$(printf '%s' "$paths" | tr -d '[:space:]')" ]; then
    paths="$(git diff --name-only HEAD~1 HEAD 2>/dev/null | tr '\n' ' ')"
    if [ -n "$(printf '%s' "$paths" | tr -d '[:space:]')" ]; then
      base_ref="HEAD~1"
    fi
  fi
  # La base no se publica aquí: esta función devuelve DOS líneas (base y paths)
  # y su salida se parsea línea a línea. Un printf de más desplazaba el parseo y
  # dejaba los paths vacíos. Se publica en el camino del motor harness, que es
  # donde alguien necesita leerla.
  if [ -n "$EXTRA_PATHS" ]; then
    paths="${paths} ${EXTRA_PATHS}"
  fi
  printf '%s\n%s' "$base_ref" "$paths"
}

DETECTED="$(detect_paths)"
BASE_REF="$(printf '%s' "$DETECTED" | sed -n '1p')"
PATHS="$(printf '%s' "$DETECTED" | sed -n '2p')"
[ -n "$BASE_REF" ] || BASE_REF="HEAD"

# ---------- motor harness (opt-in) ----------
# La delegación va antes del selector porque decide si puede sustituirlo. Reglas:
#  * `--base HEAD` es la base correcta para la semántica de este wrapper: el
#    ChangeSet del harness incluye working tree + staged vs HEAD y los untracked,
#    que es exactamente lo que `detect_paths` mira.
#  * `--oracle cr08` aplica el suelo del selector: el harness nunca exige menos
#    que CR-08 en este camino.
#  * Si el plan declara un scope NOT_IN_CATALOG, la lane `change` no puede
#    ejecutar lo que CR-08 pide: se cae al motor CR-08 y se dice por qué. Un
#    wrapper que ejecuta de menos en silencio es peor que uno lento.
if [ "$ENGINE" = "harness" ]; then
  HARNESS="${REPO_ROOT}/test-harness/bin/harness.py"
  if [ ! -f "$HARNESS" ]; then
    echo "ERROR: motor harness pedido y no existe $HARNESS" >&2
    exit 2
  fi
  if [ "$MODE" = "full" ]; then
    exec bash "${REPO_ROOT}/scripts/test-full.sh" --reason "test-fast --full"
  fi
  HARNESS_PLAN="$(python3 "$HARNESS" --repo "$REPO_ROOT" --format json plan \
      --lane change --base "$BASE_REF" --oracle cr08 2>/dev/null)"
  # La base se publica antes de decidir: es parte de la decisión. Un consumidor (o
  # un contrato) que quiera comprobar la paridad contra el plan del harness tiene
  # que planificar el MISMO cambio, y adivinarlo desde el estado del árbol hace que
  # la comprobación pase o falle según haya o no cambios sin commitear.
  printf 'harness base: %s\n' "$BASE_REF"
  if [ -n "$EXTRA_PATHS" ]; then
    # `--paths` es una entrada explícita del usuario y el harness lee su
    # ChangeSet de git: delegar aquí sería ejecutar un cambio distinto del que
    # el usuario pidió. Se queda el motor CR-08, que sí consume esos paths.
    echo "motor harness: --paths es una entrada explícita y el harness lee git; se usa el motor CR-08"
    ENGINE="cr08"
  elif [ -z "$HARNESS_PLAN" ]; then
    echo "motor harness: el plan no se pudo construir; se usa el motor CR-08" >&2
    ENGINE="cr08"
  elif printf '%s' "$HARNESS_PLAN" | grep -q '"NOT_IN_CATALOG"'; then
    echo "motor harness: CR-08 pide scopes que la lane change no ejecuta; se usa el motor CR-08"
    ENGINE="cr08"
  else
    if [ "$DRY_RUN" = "true" ]; then
      echo "--- plan (motor harness) ---"
      printf '%s\n' "$HARNESS_PLAN"
      exit 0
    fi
    python3 "$HARNESS" --repo "$REPO_ROOT" run --lane change --base "$BASE_REF" --oracle cr08
    harness_rc=$?
    case "$harness_rc" in
      0) exit 0 ;;
      2) echo "ERROR: el harness no pudo ejecutar el plan" >&2; exit 2 ;;
      *) # 1 (obligaciones incumplidas) y 3 (indeterminado) no son verdes.
         exit 1 ;;
    esac
  fi
fi

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
printf '  base: %s\n' "$BASE_REF"
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