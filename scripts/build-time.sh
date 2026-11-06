#!/usr/bin/env bash
# ============================================================================
# build-time.sh — the instrument of the build times.
# ----------------------------------------------------------------------------
# What it measures, on the same tree and in the same isolated target directory:
#
#   cold       the whole workspace compiled from nothing
#   no-change  nothing changed (the floor: what cargo costs when it does nothing)
#   touch-test a change in an integration test of the core
#   touch-core a change in core/src/lib.rs (the whole dependent subtree)
#
# These are the four moments a developer pays for. They are the same four that
# TASK-69/TASK-71 published as the baseline (V09), so a number measured here
# can be compared with a number published there.
#
# Why this is NOT a test: it compiles the workspace four times. A test that
# compiles the workspace is a task disguised as a test, and this repository
# refuses to pay that on every suite run (AGENTS.md: "un test que frena la
# entrega no se queda dentro de la suite"). It is asked for on purpose, it is
# not part of the gate, and it prints its numbers instead of asserting them.
#
# Why an ISOLATED target directory: the shared one is shared. A measurement
# that reuses the artifacts of the developer's own build measures that
# developer's cache instead of the toolchain, and it invalidates the cache of
# the project that is running. This directory is created for the measurement
# and removed afterwards (unless --keep).
#
# It must not run in parallel with any other cargo work: the numbers are wall
# times of a build, and a build that competes for the machine measures the
# competition.
#
# Usage:
#   scripts/build-time.sh [--keep] [--target-root DIR] [--cache empty|ambient]
#
#   --cache empty    the compilation cache starts EMPTY (a real cold build:
#                    the toolchain alone, with no cache). The default,
#                    `ambient`, uses the machine's cache: the numbers then
#                    include what the cache saves, and the hit rate that is
#                    printed is the machine's.
#
# Exit code: 0 when every measurement ran, 1 when one of them failed.
# ============================================================================
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${REPO_ROOT}"

# The four measurements, declared. The contract reads these names: a
# measurement that silently loses a scenario is a measurement that looks
# better than it is.
MEASUREMENTS=(cold no-change touch-test touch-core)

KEEP=0
CACHE_MODE="ambient"
TARGET_ROOT="${CARGO_TARGET_DIR:-${REPO_ROOT}/target}"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --keep) KEEP=1; shift ;;
        --cache) CACHE_MODE="${2:?--cache needs empty or ambient}"; shift 2 ;;
        --target-root) TARGET_ROOT="${2:?--target-root needs a directory}"; shift 2 ;;
        -h|--help) sed -n '2,44p' "${BASH_SOURCE[0]}"; exit 0 ;;
        *) echo "uso: $0 [--keep] [--target-root DIR] [--cache empty|ambient]" >&2; exit 2 ;;
    esac
done
if [[ "${CACHE_MODE}" != "empty" && "${CACHE_MODE}" != "ambient" ]]; then
    echo "ERROR: --cache necesita 'empty' o 'ambient'" >&2
    exit 2
fi

TOUCH_TEST="$(ls crates/cognicode-core/tests/*.rs 2>/dev/null | sort | head -1 || true)"
TOUCH_CORE="crates/cognicode-core/src/lib.rs"
if [[ -z "${TOUCH_TEST}" ]]; then
    echo "ERROR: no hay ningun test de integracion en crates/cognicode-core/tests/" >&2
    exit 2
fi
if [[ ! -f "${TOUCH_CORE}" ]]; then
    echo "ERROR: no existe ${TOUCH_CORE}" >&2
    exit 2
fi

WORK="${TARGET_ROOT}/build-time-$$"
mkdir -p "${WORK}"
cleanup() {
    if [[ "${KEEP}" -eq 1 ]]; then
        echo "--- el directorio de la medicion se conserva: ${WORK}"
    else
        rm -rf "${WORK}"
    fi
}
trap cleanup EXIT

# The mtimes of the touched files, so the measurement leaves the tree as it
# found it: touching a file without changing it is what the scenarios are
# about, and a developer should not pay for the recompilation afterwards.
CORE_MTIME="$(stat -c %y "${TOUCH_CORE}")"
TEST_MTIME="$(stat -c %y "${TOUCH_TEST}")"
restore_mtimes() {
    touch -d "${CORE_MTIME}" "${TOUCH_CORE}" 2>/dev/null || true
    touch -d "${TEST_MTIME}" "${TOUCH_TEST}" 2>/dev/null || true
}
trap 'restore_mtimes; cleanup' EXIT

now_ms() { echo $(( $(date +%s%N) / 1000000 )); }

declare -A RESULT
run_measurement() {
    local name="$1"
    shift
    echo "--- ${name} ---"
    local start end rc
    start="$(now_ms)"
    set +e
    "$@" > "${WORK}/${name}.log" 2>&1
    rc=$?
    set -e
    end="$(now_ms)"
    if [[ "${rc}" -ne 0 ]]; then
        echo "FAIL (rc=${rc}): ${name}; log en ${WORK}/${name}.log" >&2
        tail -20 "${WORK}/${name}.log" >&2 || true
        RESULT[${name}]="FAILED"
        return 1
    fi
    RESULT[${name}]="$(( end - start ))"
    echo "    ${name}: ${RESULT[${name}]} ms"
}

echo "=== build-time ==="
echo "repo:   ${REPO_ROOT}"
echo "commit: $(git rev-parse --short HEAD)"
echo "target: ${WORK} (aislado)"
echo "cache:  ${CACHE_MODE}$([[ "${CACHE_MODE}" == "empty" ]] && echo " (SCCACHE_DIR aislado, empieza vacia)" || echo " (la de la maquina)")"
echo "cargo:  $(cargo --version)"
echo "rustc:  $(rustc --version)"
if command -v sccache >/dev/null 2>&1; then
    echo "sccache: $(sccache --version 2>&1 | head -1)"
fi
if command -v mold >/dev/null 2>&1; then
    echo "mold:   $(mold --version 2>&1 | head -1)"
fi
echo

export CARGO_TARGET_DIR="${WORK}"
if [[ "${CACHE_MODE}" == "empty" ]]; then
    # A cache of its own, empty: this measurement is the toolchain alone.
    export SCCACHE_DIR="${WORK}/sccache"
    mkdir -p "${SCCACHE_DIR}"
fi

FAILED=0
run_measurement cold cargo test --workspace --no-run || FAILED=1

# The cold build leaves ~900 objects waiting to be written to the cache, and
# that write-back competes with the next measurements: measured, it turned a
# 0,6 s "no change" into a 3,0 s one. Stopping the server flushes it, so the
# scenarios that follow measure the toolchain and not the write-back.
if command -v sccache >/dev/null 2>&1; then
    sccache --stop-server >/dev/null 2>&1 || true
    echo "    (cache purgada antes de seguir)"
fi

run_measurement no-change cargo test --workspace --no-run || FAILED=1
touch "${TOUCH_TEST}"
run_measurement touch-test cargo test --workspace --no-run || FAILED=1
touch "${TOUCH_CORE}"
run_measurement touch-core cargo test --workspace --no-run || FAILED=1

echo
echo "=== resultado (ms de pared) ==="
printf '%-12s %s\n' "escenario" "tiempo"
for name in "${MEASUREMENTS[@]}"; do
    printf '%-12s %s\n' "${name}" "${RESULT[${name}]:-no medido}"
done

if command -v sccache >/dev/null 2>&1; then
    echo
    echo "=== sccache: la tasa de aciertos es visible aqui ==="
    sccache --show-stats 2>&1 | grep -iE "cache hits|cache misses|Compile requests executed" | head -6
fi

if [[ "${FAILED}" -ne 0 ]]; then
    echo
    echo "RESULTADO: una o mas mediciones fallaron (ver arriba)." >&2
    exit 1
fi
echo
echo "RESULTADO: las cuatro mediciones se corrieron."
