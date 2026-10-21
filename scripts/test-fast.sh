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
#   bash scripts/test-fast.sh --filter h42_    # acota a las pruebas que casen
#
# `--filter` es el segundo eje de acotado: el selector CR-08 elige QUE suite
# correr por los paths tocados, y el filtro elige QUE PRUEBAS de dentro. Un
# evolutivo de una unidad con nombre propio no necesita 4384 pruebas para
# saber si su contrato sigue valiendo.
#
# `--filter` NO es evidencia de gate, y no se puede confundir con ella:
#   - desactiva el baseline (compara el conjunto completo de fallos, que un
#     subconjunto filtrado nunca reproduce) y lo dice;
#   - falla si el filtro no selecciona NINGUNA prueba, porque libtest sale
#     con rc=0 cuando no encuentra nada. Sin ese fallo, un typo en el
#     nombre se lee como un verde con todas las garantias.
#   El gate es `bash scripts/test-full.sh`, sin filtro y sin atajos.
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
FILTER=""

while [ $# -gt 0 ]; do
  case "$1" in
    --paths)   EXTRA_PATHS="$2"; shift 2 ;;
    --full)    MODE="full"; shift ;;
    --dry-run) DRY_RUN="true"; shift ;;
    --no-baseline) USE_BASELINE="false"; shift ;;
    --filter)  FILTER="$2"; shift 2 ;;
    -h|--help)
      sed -n '2,/^set -uo/s/^# \{0,1\}//p' "$0" | head -25
      exit 0
      ;;
    *) echo "ERROR: flag desconocida: $1" >&2; exit 2 ;;
  esac
done

# ---------- acotado por nombre de prueba ----------
# Sin esto, `--filter h42_z` (typo) y `--filter h42_` (correcto) dan el
# mismo rc=0. Uno ejecuto pruebas; el otro no ejecuto ninguna. La
# diferencia es el riesgo entero de un filtro mal escrito, y por eso se
# cuenta antes de decir que la unidad esta verde.
#
# Es FUNCION y no un bloque al final porque el camino de fallback sale
# antes de llegar al final. Un guard que solo vigila un camino no
# vigila: MEDIDO, un filtro typo en fallback salia rc=0.
filter_verdict() {
  if [ -z "$FILTER" ] || [ "$SELECTED" -gt 0 ]; then
    return 0
  fi
  printf 'ERROR: el filtro `%s` no selecciono ninguna prueba.\n' "$FILTER" >&2
  printf '  libtest sale con rc=0 cuando no encuentra nada: un filtro mal\n' >&2
  printf '  escrito se lee como un verde sin haber ejecutado un solo test.\n' >&2
  return 1
}

# `cargo_filter` es el unico sitio donde `--filter` se convierte en
# argumentos de libtest, para que las cinco suites no puedan divergir.
cargo_filter() {
  [ -n "$FILTER" ] && printf -- " -- %s" "$FILTER"
}

# `SELECTED` acumula cuantas pruebas eligio el filtro entre todas las
# suites. Es el guard anti-vacuidad: libtest sale con rc=0 cuando el
# filtro no casa con nada, y ese 0 es indistinguible de un todo verde
# mientras no se cuente lo que se ejecuto.
SELECTED=0

# Corre un comando de cargo, deja ver su salida y suma sus pruebas
# `passed` a SELECTED. Devuelve el rc del comando, sin adornos.
run_counting() {
  local out rc
  out="$(mktemp)"
  "$@" >"$out" 2>&1; rc=$?
  cat "$out"
  SELECTED=$(( SELECTED + $(awk 'match($0,/[0-9]+ passed/){s+=substr($0,RSTART,RLENGTH)-0} END{print s+0}' "$out") ))
  rm -f "$out"
  return $rc
}

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

if [ -n "$FILTER" ]; then
  # El baseline compara el conjunto COMPLETO de fallos observados contra
  # `known_failures.yaml`. Un run filtrado produce un subconjunto, asi que
  # compararlo no es una medicion: es ruido con apariencia de control.
  USE_BASELINE="false"
  printf 'filtro: `%s` - run acotado, NO es evidencia de gate.\n' "$FILTER"
  printf '        para el gate: bash scripts/test-full.sh\n'
fi

# ---------- invocar selector ----------
if [ ! -x "$SELECTOR" ]; then
  echo "ERROR: selector no encontrado o no ejecutable: $SELECTOR" >&2
  exit 2
fi

SELECTION="$(bash "$SELECTOR" --paths "$PATHS" 2>/dev/null)"
STRATEGY="$(printf '%s' "$SELECTION" | sed -n 's/.*"strategy":"\([^"]*\)".*/\1/p')"
REASON="$(printf '%s' "$SELECTION" | sed -n 's/.*"reason":"\([^"]*\)".*/\1/p')"
# Strip JSON string quotes that the sed captures verbatim — the inner
# content of ["core","mcp"] is "core","mcp" (with literal quotes), which
# must be normalized before the case dispatch. The tr handles both single
# and multiple elements.
SUITES="$(printf '%s' "$SELECTION" | sed -n 's/.*"suites":\[\([^]]*\)\].*/\1/p' | tr -d '"')"

printf 'selector → strategy=%s suites=[%s]\n' "$STRATEGY" "$SUITES"
if [ -n "$REASON" ]; then
  printf '  motivo: %s\n' "$REASON"
fi
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

# ---------- veredicto del noop ----------
# MEDIDO 2026-10-19. Este mensaje decia "(solo metadata)" y para
# `sandbox/**` eso es FALSO: un manifiesto de escenario es un artefacto
# ejecutable con su propio lane (`certification.pipeline.kts` ->
# `sandbox-ci-smoke`), no documentacion. El selector ya emitia el motivo
# correcto y este runner lo descartaba, sustituyendolo por una
# generalizacion.
#
# Un noop que no nombra a quien cubre la superficie se lee como "aqui no
# hay nada que probar", y de ahi sale el falso verde. La regla 11 del
# selector ademas justificaba el noop con "vive en ci.yml local", y ese
# fichero no existe (tampoco `.github/workflows/`): una regla que cita
# a un dueno inexistente protege a nadie.
#
# Es FUNCION, no bloque suelto, por la misma razon que `filter_verdict`:
# como bloque, comprobarlo obligaria a lanzarlo por el script entero, y
# `--paths` se mezcla con el diff del arbol de quien lo corre — con el
# arbol sucio la rama noop ni se alcanza y la comprobacion mide el
# entorno, no el codigo. Una funcion se ejecuta sola.
noop_verdict() {
  printf 'noop: ningun camino de test tiene suite para esto; lo cubre otro lane.\n'
  case "$REASON" in
    *certification.pipeline.kts*)
      printf '  lane del sandbox: pipelinek run certification.pipeline.kts\n'
      ;;
  esac
  return 0
}

# Strategy noop: ninguna suite de cargo cubre esta superficie
if [ "$STRATEGY" = "noop" ]; then
  noop_verdict
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
  if [ -n "$FILTER" ]; then
    run_counting cargo test --workspace --lib --quiet -- "$FILTER"
  else
    run_counting cargo test --workspace --lib --quiet
  fi
  cargo_clippy_rc=$?
  cargo clippy --workspace --all-targets --quiet -- -D warnings >/dev/null 2>&1
  clippy_rc=$?
  if [ "$cargo_clippy_rc" -ne 0 ] || [ "$clippy_rc" -ne 0 ]; then
    exit 1
  fi
  filter_verdict
  exit $?
fi

# ---------- mapear suites a comandos ----------
# MEDIDO 2026-10-19: solo `explorer` recibia `$(cargo_filter)`. `core`
# corria sus ~2370 tests SIN acotar y tampoco pasaba por `run_counting`,
# asi que SELECTED se quedaba en 0 y `--filter` sobre core terminaba en
# dos fallos a la vez: no acotaba nada, y ademas reportaba "el filtro no
# selecciono ninguna prueba" cuando si habia pruebas. Un filtro que solo
# puede mentir es peor que un filtro ausente: el que lo usa creyendo que
# acota el feedback loop esta corriendo la suite entera. Las cinco suites
# pasan por `run_counting` para que el acotado y el recuento sean el mismo
# hecho, no dos que puedan divergir.
run_suite() {
  local suite="$1"
  case "$suite" in
    core)     run_counting cargo test -p cognicode-core --lib $(cargo_filter) ;;
    explorer) run_counting cargo test -p cognicode-explorer --lib $(cargo_filter) ;;
    mcp)      run_counting cargo test -p cognicode-mcp --lib $(cargo_filter) ;;
    cli)      run_counting cargo test -p cognicode-cli --lib $(cargo_filter) ;;
    ladybug)  run_counting cargo test -p cognicode-ladybug --lib $(cargo_filter) ;;
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

# ---------- veredicto del acotado ----------
filter_verdict || overall_rc=1

# ---------- clippy ----------
# MEDIDO 2026-10-19: `fast` solo corria clippy en el camino `fallback`.
# En `changed` —el camino del trabajo diario— el ciclo se cerraba verde
# con tres lints en el arbol (`clone_on_copy`, `needless_borrow` x2) y el
# gate los encontraba al final, dos minutos despues. `fast` existe para
# acortar el feedback loop, no para recortarlo: lo que el gate exige hay
# que verlo antes. El diagnostics salen a pantalla a proposito — el
# camino `fallback` los descarta con `>/dev/null`, y un error de clippy
# que no se ve es un error que se re-descubre cada vez.
printf '\n=== clippy -D warnings ===\n'
if ! cargo clippy --workspace --all-targets --quiet -- -D warnings; then
  printf 'ERROR: clippy -D warnings. El gate lo exige tambien.\n' >&2
  overall_rc=1
fi

# ---------- baseline check (core) ----------
# Cuando la línea base no tiene entradas comparables, re-correr
# `cargo test -p cognicode-core --lib` aquí solo verifica lo evidente:
# cualquier fallo sería una nueva regresión, y un éxito confirmaría la
# batería que ya corrió arriba dentro de la suite `core`. Saltarlo
# acorta el lazo de feedback sin perder cobertura: los fallos del
# `cargo test` siguen contando desde la corrida del suite.
if [ "$USE_BASELINE" = "true" ] && [ -f "$BASELINE_CHECK" ] && [ -f "$BASELINE" ]; then
  if printf ' %s ' "$seen" | grep -q ' core '; then
    if ! BASELINE_ENTRIES=$(grep -cE '^[[:space:]]*-[[:space:]]+test:' "$BASELINE" 2>/dev/null); then
      BASELINE_ENTRIES=0
    fi
    if [ "${BASELINE_ENTRIES:-0}" -eq 0 ]; then
      printf '\n=== baseline: check_known_failures (core/lib) — skipped (baseline is empty: %s) ===\n' "$BASELINE"
    else
      printf '\n=== baseline: check_known_failures (core/lib, %s entries) ===\n' "$BASELINE_ENTRIES"
      if ! python3 "$BASELINE_CHECK" --package cognicode-core --target lib; then
        overall_rc=1
      fi
    fi
  fi
fi

exit "$overall_rc"