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

# `TOUCHED_RAN` cuenta los targets de integracion tocados que se han
# ejecutado. MEDIDO 2026-10-29: sin el, el camino de fallback no podia
# distinguir "no habia ningun target tocado" de "habia y no se ha
# corrido", y los dos se leen igual desde fuera: un verde. Un wrapper que
# no puede decir que no, aqui, no puede decir nada.
TOUCHED_RAN=0

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

# El camino de fallback vive MAS ABAJO, despues de `run_suite`. MEDIDO
# 2026-10-29: mientras vivia aqui no podia llamar a `run_suite` —la funcion
# todavia no estaba registrada en ese punto del script— y se saltaba la
# funcion entera. Ver el bloque de fallback mas abajo.

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
# Los targets de integracion que implican los paths tocados de un crate.
#
# MEDIDO 2026-10-22. `run_suite` mapeaba las cinco suites a
# `cargo test -p <crate> --lib`, de modo que tocar
# `crates/<crate>/tests/<fichero>.rs` seleccionaba la suite correcta y no
# ejecutaba el fichero tocado: el lazo de feedback daba verde sin haber
# corrido el test que se acababa de escribir. Correr los 99 binarios de
# integracion de `core` convertia `fast` en minutos, asi que no se corren
# todos: se corre el que el path toca, que es el unico que puede hablar del
# cambio.
#
# Un fichero directamente bajo `tests/` es un target; uno dentro de un
# subdirectorio es un modulo compartido, y un modulo no produce binario. Un
# fichero que ya no esta en disco no produce binario tampoco, y puede
# seguir en el diff: un test borrado no se puede correr, y pedirlo hace que
# cargo falle con un nombre de target que no existe.
touched_targets() {
  local crate="$1"
  local base="crates/${crate}/tests"
  local names="" path rel name
  for path in ${PATHS:-}; do
    case "$path" in
      "${base}"/*) ;;
      *) continue ;;
    esac
    rel="${path#"${base}"/}"
    case "$rel" in
      */*) continue ;;
      *.rs) ;;
      *) continue ;;
    esac
    [ -f "$path" ] || continue
    name="${rel%.rs}"
    case " ${names} " in *" ${name} "*) continue ;; esac
    names="${names}${name} "
  done
  printf '%s' "${names% }"
}

run_suite() {
  local suite="$1"
  # `touched-only`: solo los targets de integracion que los paths tocan, sin
  # la biblioteca del crate. Es el modo del camino de fallback, que ya corre
  # `cargo test --workspace --lib` y no debe repetirla una vez por suite.
  # MEDIDO 2026-10-29. El mapa suite->crate vive AQUI, en este `case`, y es
  # el unico sitio donde vive: el camino de fallback lo usa en vez de
  # derivar el crate del path, que seria una segunda fuente de verdad — y
  # que ademas daria un rojo falso sobre `crates/spike-ladybug/`, que no es
  # miembro del workspace y por tanto no tiene paquete que cargo resuelva.
  local mode="${2:-full}"
  local crate
  case "$suite" in
    core)     crate=cognicode-core ;;
    explorer) crate=cognicode-explorer ;;
    mcp)      crate=cognicode-mcp ;;
    cli)      crate=cognicode-cli ;;
    ladybug)  crate=cognicode-ladybug ;;
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

  # La suite dice QUE crate; los paths tocados dicen QUE target. Se corren
  # los dos: la biblioteca, que es el contrato de la unidad, y el binario
  # de integracion del fichero que se ha tocado, que es lo que puede
  # hablar del cambio. En modo `touched-only` la biblioteca no se corre: el
  # camino que usa ese modo ya la ha corrido, entera, con `--workspace`.
  local rc=0 target
  if [ "$mode" != "touched-only" ]; then
    run_counting cargo test -p "$crate" --lib $(cargo_filter) || rc=1
  fi
  for target in $(touched_targets "$crate"); do
    printf -- '--- %s: target de integracion tocado ---\n' "$target"
    TOUCHED_RAN=$(( TOUCHED_RAN + 1 ))
    run_counting cargo test -p "$crate" --test "$target" $(cargo_filter) || rc=1
  done
  return $rc
}

# ---------- camino de fallback ----------
# MEDIDO 2026-10-29, en el cierre de TASK-16. Este bloque estaba mas arriba,
# antes de `run_suite`, y por eso no podia llamar a `run_suite`: se saltaba
# la funcion entera y corria `cargo test --workspace --lib`, que son SOLO
# pruebas unitarias. Con un test de integracion nuevo y un path que el
# selector no conoce en el mismo diff —`product/tools.json` no lo conoce
# ninguna regla de CR-08, y esta versionado— el lazo de feedback daba
# verde sin haber corrido el test recien escrito. Es el mismo falso verde
# que M13 cierra en el camino `changed`, un camino mas alla.
#
# El fallback no puede correr la bateria de integracion: eso es `full`, y
# correr los 99 binarios de `core` convertira `fast` en minutos. Lo que si
# puede —y debe— es correr el target que los paths tocan, que es el unico
# que puede hablar del cambio.
if [ "$STRATEGY" != "changed" ] || [ -z "$SUITES" ]; then
  echo "fallback: el selector no pudo clasificar el cambio (motivo: ${REASON:-sin_motivo})."
  echo "  Subconjunto conservador (workspace --lib) + el target de integracion"
  echo "  que los paths tocan, si lo tocan."
  echo "  AVISO: NO se corre la bateria de integracion (b5_*, b6_*, prf_*)."
  echo "  Para esa bateria: bash scripts/test-full.sh"
  fallback_rc=0
  if [ -n "$FILTER" ]; then
    run_counting cargo test --workspace --lib --quiet -- "$FILTER" || fallback_rc=1
  else
    run_counting cargo test --workspace --lib --quiet || fallback_rc=1
  fi
  # Los targets tocados, y solo ellos. `merge` se salta: no es una suite
  # cargo, es el merge-gate completo, y su comando es `test-full.sh`.
  for s in $(printf '%s' "$SUITES" | tr ',' ' '); do
    case " $s " in
      *" merge "*) continue ;;
    esac
    printf '\n=== suite: %s (solo el target tocado) ===\n' "$s"
    run_suite "$s" touched-only || fallback_rc=1
  done
  cargo clippy --workspace --all-targets --quiet -- -D warnings >/dev/null 2>&1
  clippy_rc=$?
  if [ "$TOUCHED_RAN" -eq 0 ]; then
    echo "  AVISO: ningun target de integracion se ha corrido. Los paths tocados"
    echo "  no produjeron ningun target en las suites del selector, o el crate"
    echo "  tocado no lo mapea el selector. En ambos casos este subconjunto no"
    echo "  dice nada del cambio de integracion: el gate si lo dice"
    echo "  (bash scripts/test-full.sh)."
  else
    echo "  targets de integracion ejecutados: ${TOUCHED_RAN}"
  fi
  if [ "$fallback_rc" -ne 0 ] || [ "$clippy_rc" -ne 0 ]; then
    exit 1
  fi
  filter_verdict
  exit $?
fi

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
#
# La decisión de si la línea base está vacía la toma el propio checker:
# el formato del fichero es suyo, y contar entradas con un `grep` en shell
# sería una segunda fuente de verdad para la misma regla. Aquí solo se
# decide si el paso se aplica, no lo que dice.
if [ "$USE_BASELINE" = "true" ] && [ -f "$BASELINE_CHECK" ] && [ -f "$BASELINE" ]; then
  if printf ' %s ' "$seen" | grep -q ' core '; then
    printf '\n=== baseline: check_known_failures (core/lib) ===\n'
    if ! python3 "$BASELINE_CHECK" --package cognicode-core --target lib --skip-if-empty; then
      overall_rc=1
    fi
  fi
fi

exit "$overall_rc"