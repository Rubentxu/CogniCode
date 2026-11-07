#!/usr/bin/env bash
# Una corrida focal de mutación, con el techo de memoria que el gate tiene.
#
# QUÉ ES Y POR QUÉ EXISTE
#
# Mirar un fichero mutado es una operación de a mano: se pide una ojeada
# a los mutantes de un fichero, se leen, y se decide. La forma de corrida
# que se documentaba para eso era `cargo mutants --file <ruta> -j 8`, y
# esa forma no llevaba nada de lo que hace segura al paso de mutantes del
# gate:
#
#   - sin techo de memoria: un mutante que se desborda crece hasta que la
#     máquina se queda sin memoria. Medido el 2026-11-05 a las 19:40, el
#     OOM-killer del kernel mató un binario de `cognicode-core` de 37 GB
#     de RSS y, en la misma pasada, se llevó `coredns`, el `java` de un
#     contenedor y dos procesos de otro: nada de eso es de este
#     repositorio, y ese es el punto;
#   - sin directorio de destino propio: el binario del último mutante se
#     queda en la caché compartida y la suite que corra después lo
#     reutiliza y reporta fallos fantasma en tests que no tienen nada que
#     ver (AGENTS.md, regla 8);
#   - sin presupuesto: el techo se paga por cada trabajo a la vez, así que
#     ocho trabajos con un techo de 2 GiB piden 16 GiB de espacio de
#     direcciones de golpe.
#
# Esta ruta es la misma forma con las tres cosas. No es una certificación
# —la certificación es el paso de mutantes del gate, en serie—: es una
# mirada, y ahora se hace sin riesgo para la máquina.
#
# USO
#
#   scripts/mutate-file.sh crates/cognicode-core/src/infrastructure/graph/strategy.rs
#   scripts/mutate-file.sh --jobs 4 crates/cognicode-core/src/lib.rs
#   scripts/mutate-file.sh --plan crates/cognicode-core/src/lib.rs
#
# `--plan` imprime lo que la corrida haría, sin mutar nada: es lo que
# lee el contrato `scripts/ci/test_gate_contracts.py`.
#
# EL TECHO Y EL PRESUPUESTO
#
# El techo por defecto es el de `scripts/mutants-budget.sh`
# (`MUTANTS_FOCAL_VMEM_KIB`), que está por encima del pico legítimo del
# binario de test del core (2,70 GiB, medidos) y corta al mutante que se
# desborda. Se puede subir con `MUTANT_TEST_VMEM_KB`; si el techo que se
# pide no cabe en el presupuesto, la ruta lo dice y no corre.
#
# Los trabajos se derivan del presupuesto: pedir más no los compra.
set -uo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "${REPO_ROOT}" ]; then
  echo "ERROR: no se ejecuta dentro de un repositorio git" >&2
  exit 2
fi
cd "${REPO_ROOT}" || exit 2

# El dueño único del presupuesto y de los techos. Sin dueño no hay techo, y
# sin techo esta ruta es la forma de corrida que se llevó la máquina por
# delante: se sale con el motivo, no con un default que nadie ha declarado.
# shellcheck source=scripts/mutants-budget.sh
if ! source "${REPO_ROOT}/scripts/mutants-budget.sh"; then
  echo "ERROR: scripts/mutants-budget.sh no se puede leer: el techo no tiene dueño" >&2
  exit 2
fi

usage() {
  cat >&2 <<'USAGE'
uso: scripts/mutate-file.sh <ruta> [--jobs N]
     scripts/mutate-file.sh --plan <ruta> [--jobs N]

  <ruta>    el fichero a mutar, con la forma crates/<crate>/src/...
  --jobs N  cuántos mutantes se miden a la vez (acotado por el presupuesto)
  --plan    imprime lo que la corrida haría, sin mutar nada
USAGE
}

PLAN=false
JOBS=1
TARGET=""

while [ $# -gt 0 ]; do
  case "$1" in
    --plan)
      PLAN=true
      shift
      ;;
    --jobs)
      JOBS="${2:-}"
      shift 2 || true
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    -*)
      printf 'opción desconocida: %s\n' "$1" >&2
      usage
      exit 2
      ;;
    *)
      TARGET="$1"
      shift
      ;;
  esac
done

if [ -z "${TARGET}" ]; then
  echo "ERROR: falta el fichero a mutar" >&2
  usage
  exit 2
fi

# `crates/<crate>/src/...` es la forma con la que se piensa en el
# workspace; cargo-mutants quiere el fichero relativo a la raíz del
# paquete, y el paquete por su nombre.
PACKAGE=""
FILE="${TARGET}"
case "${TARGET}" in
  crates/*/src/*)
    PACKAGE="$(printf '%s' "${TARGET}" | cut -d/ -f2)"
    FILE="$(printf '%s' "${TARGET}" | cut -d/ -f3-)"
    ;;
  *)
    printf 'ERROR: la ruta tiene que ser de la forma crates/<crate>/src/...\n' >&2
    printf '       (la que se ha dado: %s)\n' "${TARGET}" >&2
    exit 2
    ;;
esac

case "${JOBS}" in
  '' | *[!0-9]*)
    printf 'ERROR: --jobs tiene que ser un número\n' >&2
    exit 2
    ;;
esac

# El techo: el de la ruta focal, o el que pida quien la llama.
CEILING_KIB="${MUTANT_TEST_VMEM_KB:-${MUTANTS_FOCAL_VMEM_KIB:-}}"
case "${CEILING_KIB}" in
  '')
    printf 'ERROR: scripts/mutants-budget.sh no declara MUTANTS_FOCAL_VMEM_KIB: el techo no está escrito\n' >&2
    exit 2
    ;;
  *[!0-9]*)
    printf 'ERROR: el techo tiene que ser un número de KiB\n' >&2
    exit 2
    ;;
esac

case "${MUTANTS_BUDGET_KIB:-}" in
  '')
    printf 'ERROR: scripts/mutants-budget.sh no declara MUTANTS_BUDGET_KIB: el presupuesto no está escrito\n' >&2
    exit 2
    ;;
  *[!0-9]*)
    printf 'ERROR: el presupuesto tiene que ser un número de KiB\n' >&2
    exit 2
    ;;
esac

if [ "${CEILING_KIB}" -gt "${MUTANTS_BUDGET_KIB}" ]; then
  printf 'ERROR: el techo pedido (%s KiB) no cabe en el presupuesto (%s KiB)\n' \
    "${CEILING_KIB}" "${MUTANTS_BUDGET_KIB}" >&2
  exit 2
fi

# Los trabajos se derivan del presupuesto: el techo se paga por cada uno
# a la vez, así que pedir más de los que caben no los compra.
MAX_JOBS=$((MUTANTS_BUDGET_KIB / CEILING_KIB))
[ "${MAX_JOBS}" -lt 1 ] && MAX_JOBS=1
if [ "${JOBS}" -gt "${MAX_JOBS}" ]; then
  JOBS="${MAX_JOBS}"
fi

# El directorio de destino propio: el binario del último mutante no puede
# quedarse en la caché compartida (AGENTS.md, regla 8).
CARGO_TARGET_ROOT="$(cargo metadata --format-version 1 --no-deps 2>/dev/null \
  | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p' | head -1)"
CARGO_TARGET_ROOT="${CARGO_TARGET_ROOT:-${HOME}/.cache/cognicode/cargo-targets}"
TARGET_DIR="${MUTANTS_FOCAL_TARGET_DIR:-${CARGO_TARGET_ROOT}/mutants-focal}"

RUNNER="${REPO_ROOT}/scripts/mem-capped-runner.sh"

if $PLAN; then
  printf 'ceiling_kib=%s\n' "${CEILING_KIB}"
  printf 'jobs=%s\n' "${JOBS}"
  printf 'budget_kib=%s\n' "${MUTANTS_BUDGET_KIB}"
  printf 'target_dir=%s\n' "${TARGET_DIR}"
  printf 'runner=%s\n' "${RUNNER}"
  printf 'gitignore=true\n'
  printf 'package=%s\n' "${PACKAGE}"
  printf 'file=%s\n' "${FILE}"
  exit 0
fi

if [ ! -f "${TARGET}" ]; then
  printf 'ERROR: no existe el fichero %s\n' "${TARGET}" >&2
  exit 2
fi

mkdir -p "${TARGET_DIR}"

printf 'mutando %s (paquete %s) con techo de %s KiB y %s trabajo(s)\n' \
  "${FILE}" "${PACKAGE}" "${CEILING_KIB}" "${JOBS}"

exec env CARGO_TARGET_DIR="${TARGET_DIR}" \
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER="${RUNNER}" \
  MUTANT_TEST_VMEM_KB="${CEILING_KIB}" \
  cargo mutants -p "${PACKAGE}" --file "${FILE}" -j "${JOBS}" \
  --gitignore true \
  -- -- --test-threads=4
