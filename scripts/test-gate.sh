#!/usr/bin/env bash
# scripts/test-gate.sh
#
# The gate. Three things, and nothing else:
#
#   1. unit tests        — cargo nextest run --profile gate, over the radius
#   2. integration tests — cargo nextest run --profile gate, over the radius
#   3. mutants           — cargo mutants, over the crates the batch can break
#
# There is no CI lane behind this script. It was retired on 2026-11-04 with the
# rest of the gate apparatus, and what replaced it is the tests themselves plus
# a human who reads the diff. The three steps above are what "the gate is
# green" means.
#
# THE RADIUS: WHAT THIS RUN MEASURES, AND WHY
# --------------------------------------------
# The gate used to measure everything, always: the whole workspace for the two
# test steps, and `cognicode-graph-algos` for the mutation step. Measured on
# 2026-11-06 (the receipt of TASK-69 is in the JOURNAL), that costs 61 minutes
# cold and 55 minutes warm, and the mutation step is 92% of it — 98% warm.
# A gate whose cost does not depend on what changed is a gate that cannot be
# run often, and a gate that is not run often is a gate that certifies less.
#
# So the gate measures a RADIUS, and the radius is computed by ONE script:
# `scripts/gate_scope.py`. The gate does not decide what to measure; it asks,
# and it runs what the answer says. Two decisions about the same radius would
# be two sources of truth, and the second one is the one that rots.
#
# The answer has two halves, and they are different on purpose:
#
#   THE RADIUS OF THE TESTS is the inverse dependency closure of the crates
#   the batch touched, and cargo decides the graph. A crate can only break
#   because of a change in itself or in one of its dependencies, so the
#   closure is a fact and not a guess. It is not a filter by path: the
#   retired `scripts/test-fast.sh` filtered by path and its fallback ran
#   `cargo test --workspace --lib` only, so an integration test that had just
#   been written never ran — the suite was green because it had not looked.
#
#   THE RADIUS OF THE MUTANTS is the crates the batch touched that are
#   AFFORDABLE to mutate, plus the crate of the calendar when its turn comes.
#
# WHAT IS MUTABLE, AND WHAT IS NOT, AND WHY IT IS NOT A LIE
# -----------------------------------------------------------
# A scope that only says what it includes reads like a scope that includes
# everything else. The crates that are out of it are named here with the
# reason, and the reason is always one of two things: COST or WORK.
#
# COST — the mutation step copies the tree before it mutates anything, and the
# copy lands on a different path than the repository's own builds, so cargo
# cannot reuse the shared cache. Every mutant of a crate that depends on
# `cognicode-core` therefore rebuilds `cognicode-core` from scratch:
#
#   cognicode-core        217k lines — mutating it does not finish in a
#   cognicode-explorer     65k lines — lifetime, and a gate nobody ever sees
#                                     the end of is a gate that watches
#                                     nothing.
#   cognicode-runtime      1.1k lines — measured on 2026-11-04: 40 mutants
#   cognicode-mcp           874 lines — at ~140 s of rebuild and ~80 s of
#                                     test each, because both sit on top of
#                                     `cognicode-core`. That is hours, and
#                                     the step has to be one a person runs
#                                     before a merge. Its known hole (the
#                                     telemetry opt-in) is closed with a
#                                     test; its one equivalent mutant (the
#                                     Rayon stack size) is declared in
#                                     `.cargo/mutants.toml`.
#
# WORK — the measurement was made and the holes are real, so these crates
# are waiting for their tests:
#
#   cognicode-graph-wasm   102 mutants, 88 survived the measurement
#   cognicode-macros       155 mutants, 99 survived the measurement
#   cognicode-ladybug      305 mutants, not measured yet
#
# The counts and the reason are in the JOURNAL receipt, so the exclusion is
# visible rather than implied.
#
# The calendar does not change WHICH crate is ever measured: it changes WHEN.
# The mutation scope before this policy was `cognicode-graph-algos`, every
# batch; after it, it is `cognicode-graph-algos` when the batch touches it or
# when its turn comes. The set of crates that carry a mutation certificate
# does not shrink — the same crate, on a declared schedule.
#
# THE CALENDAR: THE AGE OF A HOLE IS BOUNDED
# -------------------------------------------
# The crate that carries the graph logic is measured on a turn, not on every
# batch. The turn is declared HERE, in three numbers, so that anybody can
# check when it last fired:
#
#   MUTANT_CALENDAR_PACKAGE   the crate that is measured in its turn
#   MUTANT_CALENDAR_EVERY_N   how many merges of `main` it waits between turns
#   MUTANT_CALENDAR_MARKER    the commit where the count starts
#
# The unit of the calendar is the merge commit on the main line: it is
# countable from git and it is auditable by anybody, with no state file that
# could disagree with the history. A batch of ten units puts about ten merges
# in `main`, so the period below is about five batches.
#
# The turn is a CROSSING, not a remainder: the batch fires when it crosses a
# multiple of the period, because a batch does not always have the same size
# and a rule of "fire when the count is a multiple" can step over the multiple
# and never fire.
#
# The marker is the commit where the last batch's gate was measured in full:
# the count starts there, and the hole of a mutation certificate therefore has
# an age bounded by the period, counted in merges.
#
# The mutants step builds in its own target directory. cargo-mutants works on a
# copy of the tree, so its builds land under a different path than the
# repository's own builds and would otherwise evict them from the shared
# cache. `MUTANTS_TARGET_DIR` can point somewhere else on a machine that has
# the space.
#
# There is no `cargo doc` anywhere in this script, and no test that runs one.
# The `m011_rustdoc_gate` target used to run a full `cargo doc --workspace`
# from inside the test suite, which cost ~150 s on EVERY `cargo test` — and
# on every one of the forty mutants, because a mutation step re-runs the
# suite forty times. ADR-TH-011 had already decided that "rustdoc se paga
# una vez"; the lane that implemented that decision is gone, so the target
# was paying that price on every run instead of once. The check survives as
# `just docs-check`, which is where a documentation audit belongs: on
# demand, not inside every test run.
#
# Usage:
#   bash scripts/test-gate.sh              # the whole gate
#   bash scripts/test-gate.sh --no-mutants # unit + integration only
#   bash scripts/test-gate.sh --mutants    # mutants only
#
# The base of the batch (the commit where it started) can be named with
# GATE_BASE. Without it, the base is where this branch diverged from `main`,
# and on `main` itself there is no divergence to measure against, so the gate
# runs WHOLE and says so.
#
# Policy: see AGENTS.md § "Estrategia de tests".
set -uo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "${REPO_ROOT}" ]; then
  echo "ERROR: no se ejecuta dentro de un repositorio git" >&2
  exit 2
fi
cd "${REPO_ROOT}"

# The mutation scope, declared here so that the numbers that decide what is
# measured are in one place and not in the answer of a command.
#
# The crates that are affordable to mutate: the ones whose mutant is a rebuild
# of a single crate and one test run. The rest are out for COST or WORK, and
# the reasons are in the header above.
MUTANT_AFFORDABLE_PACKAGES="cognicode-graph-algos"

# The calendar: the crate that carries the graph logic, measured in its turn.
MUTANT_CALENDAR_PACKAGE="cognicode-graph-algos"
MUTANT_CALENDAR_EVERY_N=50
MUTANT_CALENDAR_MARKER="fb526521db6e45142362d9eb19d53f2ba2a9c05a"

RUN_UNIT=true
RUN_INTEGRATION=true
RUN_MUTANTS=true

case "${1:-}" in
  --no-mutants) RUN_MUTANTS=false ;;
  --mutants)    RUN_UNIT=false; RUN_INTEGRATION=false ;;
  -h|--help)
    # Print the header comment: everything from the first comment line up to
    # the `set` line, minus the leading "# ".
    sed -n '3,/^set /p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
    ;;
  "") ;;
  *) echo "ERROR: opcion desconocida: $1" >&2; exit 2 ;;
esac

# Steps 1 and 2 run under cargo-nextest (one process per test, profile `gate`
# in .config/nextest.toml). A missing runner is infrastructure, not a
# regression, and it has to say so before the suite starts.
if { $RUN_UNIT || $RUN_INTEGRATION; } && ! cargo nextest --version >/dev/null 2>&1; then
  printf 'FAIL: cargo-nextest no esta instalado. Instálalo con: cargo install cargo-nextest --locked\n' >&2
  exit 2
fi

printf '=== test-gate @ %s ===\n' "$(git rev-parse --short HEAD)"
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

# THE RADIUS. The decision lives in scripts/gate_scope.py, so the gate has no
# branch of its own that can be wrong. The script always answers: if it cannot
# decide, the answer is the WHOLE gate, with the reason.
radius_field() {
  printf '%s\n' "$RADIUS" | sed -n "s/^$1=//p"
}

RADIUS_ARGS=(
  --repo-root "${REPO_ROOT}"
  --affordable "${MUTANT_AFFORDABLE_PACKAGES}"
  --calendar-crate "${MUTANT_CALENDAR_PACKAGE}"
  --calendar-every "${MUTANT_CALENDAR_EVERY_N}"
  --calendar-marker "${MUTANT_CALENDAR_MARKER}"
)
if [ -n "${GATE_BASE:-}" ]; then
  RADIUS_ARGS+=(--base "${GATE_BASE}")
fi
# stdout carries the answer, stderr carries the trouble: a traceback has to be
# visible in the log, and it must not be able to corrupt the answer.
RADIUS="$(python3 "${REPO_ROOT}/scripts/gate_scope.py" "${RADIUS_ARGS[@]}")"
RADIUS_RC=$?

R_STATUS="$(radius_field status)"
R_BASE="$(radius_field base)"
R_TOUCHED="$(radius_field touched)"
R_TEST="$(radius_field test-scope)"
R_TEST_FULL="$(radius_field test-scope-full)"
R_MUTANT="$(radius_field mutant-scope)"
R_CALENDAR="$(radius_field calendar)"
R_REASON="$(radius_field reason)"

# The selector could not even answer (it is missing, or python is missing, or
# it died). The gate then measures EVERYTHING: an empty radius would mean
# "nothing to check", and a gate that checks nothing and says green is worse
# than a slow gate.
if [ -z "${R_STATUS}" ]; then
  R_STATUS="undetermined"
  R_BASE=""
  R_TOUCHED=""
  R_TEST=""
  R_TEST_FULL="true"
  R_MUTANT="${MUTANT_AFFORDABLE_PACKAGES} ${MUTANT_CALENDAR_PACKAGE}"
  R_CALENDAR="unknown"
  R_REASON="el selector del radio no respondio (rc=${RADIUS_RC}): ${RADIUS}"
fi

# The scope of the two test steps: the whole workspace when the radius could
# not be narrowed, and the closure of the touched crates when it could.
if [ "${R_TEST_FULL}" = "true" ]; then
  TEST_PKG_ARGS=(--workspace)
else
  TEST_PKG_ARGS=()
  for pkg in ${R_TEST}; do
    TEST_PKG_ARGS+=(-p "${pkg}")
  done
fi

printf '\n=== radio ===\n'
if [ "${R_STATUS}" = "ok" ]; then
  printf 'base:        %s\n' "${R_BASE:0:12}"
  printf 'tocado:      %s\n' "${R_TOUCHED:-(ninguno)}"
  printf 'radio test:  %s\n' "${R_TEST}"
else
  printf 'radio:       NO DETERMINADO — %s\n' "${R_REASON}"
  printf 'radio test:  TODO EL WORKSPACE (el gate corre entero)\n'
fi
printf 'radio mut.:  %s\n' "${R_MUTANT:-(vacio)}"
printf 'calendario:  %s (%s fusiones desde el marcador %s, cada %s)\n' \
  "${R_CALENDAR}" "$(radius_field calendar-count)" "${MUTANT_CALENDAR_MARKER:0:12}" \
  "${MUTANT_CALENDAR_EVERY_N}"

# 1. Unit tests. `--lib` compiles and runs the `#[cfg(test)]` modules inside
#    the sources: one unit, in isolation, no process and no network.
if $RUN_UNIT; then
  run_step "unit tests" cargo nextest run --profile gate "${TEST_PKG_ARGS[@]}" --lib
fi

# The release-flow suites execute release-profile bytes on purpose: a debug
# build that happens to exist would let them pass without testing what they
# claim to test. When the binary is missing they say so in a panic that reads
# like a product failure. It is not: it is infrastructure. This check says it
# once, before the suite starts, with the command to run.
if $RUN_INTEGRATION; then
  RELEASE_ROOT="$(cargo metadata --format-version 1 --no-deps 2>/dev/null \
    | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p' | head -1)/release"
  MISSING=""
  for bin in cognicode cogh cognicode-release; do
    if [ ! -x "${RELEASE_ROOT}/${bin}" ]; then
      MISSING="${MISSING} ${bin}"
    fi
  done
  if [ -n "${MISSING}" ]; then
    printf 'AVISO: faltan binarios de release (%s).\n' "${MISSING}"
    printf '      Es infraestructura, no una regresión. Construye con:\n'
    printf '        cargo build --release --bin <nombre>\n\n'
  fi
fi

# 2. Integration tests. `--tests` compiles and runs every target under
#    `tests/`: several real pieces working together, with the edges mocked.
if $RUN_INTEGRATION; then
  run_step "integration tests" cargo nextest run --profile gate "${TEST_PKG_ARGS[@]}" --tests
fi

# 3. Mutants. A mutant is a deliberate defect injected into the source; a
#    mutant that survives is a hole in the suite that no amount of passing
#    tests would have revealed.
if $RUN_MUTANTS; then
  if [ -z "${R_MUTANT}" ]; then
    # An empty radius here does NOT mean "nothing to check": it means that this
    # batch touched no crate that is affordable to mutate, and that the crate
    # of the calendar is not in its turn. The gate says so, loudly, because a
    # skipped step that does not say it was skipped is a step that looks green.
    printf '\n--- mutants ---\n'
    printf 'SKIP (mutants): este lote NO midio mutacion.\n'
    printf '  motivo:      el lote toco %s, y ninguno es mutable por coste;\n' \
      "${R_TOUCHED:-(nada)}"
    printf '               el crate del calendario (%s) no esta en su turno (%s).\n' \
      "${MUTANT_CALENDAR_PACKAGE}" "${R_CALENDAR}"
    printf '  el hueco:    el calendario va en %s fusiones desde el marcador,\n' \
      "$(radius_field calendar-count)"
    printf '               y acota la edad de un hueco a %s fusiones (%s lotes de diez).\n' \
      "${MUTANT_CALENDAR_EVERY_N}" "$((MUTANT_CALENDAR_EVERY_N / 10))"
    printf '  lo que no se dice: que la mutacion se midio y no encontro nada.\n'
  elif ! command -v cargo-mutants >/dev/null 2>&1 && ! cargo mutants --version >/dev/null 2>&1; then
    printf 'FAIL: cargo-mutants no esta instalado. Instálalo con: cargo install cargo-mutants --locked\n' >&2
    overall_rc=1
  else
    # The mutants are measured ONE AT A TIME, and this is not a taste: it is
    # what the tool can certify. cargo-mutants works on a single copy of the
    # tree — one BuildDir, shared by every worker — and its workers mutate
    # that copy without a lock (lab.rs: apply -> build -> test -> revert).
    # With more than one worker, they write to the same file: one applies
    # its mutation, another overwrites it, a third reverts it while the
    # first is still compiling. The verdict of a mutant then stops being
    # attributable to that mutant.
    #
    # Measured 2026-11-05 on this scope: a run with eight workers reported
    # SEVEN survivors in `god_nodes.rs`, and all ten mutants of that file
    # are CAUGHT when they are measured on their own. The same run also
    # reported a number of unviable mutants that changed from one run to
    # the next (1, 2, 4) over the very same tree.
    #
    # A gate that reports a hole that is not there is a gate nobody trusts,
    # and a gate that reports a catch that came from somebody else's
    # mutation is a gate that watches nothing. The certificate here is per
    # mutant, so the measurement is serial.
    #
    # For a quick look at one file, `scripts/mutate-file.sh` says the same
    # thing in seconds; that is a LOOK, not a certificate, and it does not
    # go through this script on purpose.
    MUTANTS_JOBS=1
    # The mutants build gets its own target directory, derived from the one
    # cargo already uses on this machine rather than hardcoded, so the script
    # works on a machine whose cache lives somewhere else. cargo-mutants
    # builds a COPY of the tree, and without a directory of its own those
    # builds evict the repository's own artifacts from the shared cache.
    #
    # The directory is its own for a second reason, and it is not a nicety:
    # cargo-mutants restores the mutated source file with its ORIGINAL mtime,
    # so when the run ends the target directory still holds the binary of the
    # LAST mutant and cargo considers it fresh. A `cargo test` afterwards in
    # the same directory reuses that binary and reports phantom failures in
    # tests that have nothing to do with the mutation. Measured 2026-11-04:
    # `single_community_top_node` failed out of nowhere after a mutant run.
    # Its own directory means the suite never sees the mutant's binary.
    if [ -z "${MUTANTS_TARGET_DIR:-}" ]; then
      CARGO_TARGET_ROOT="$(cargo metadata --format-version 1 --no-deps 2>/dev/null \
        | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p' | head -1)"
      CARGO_TARGET_ROOT="${CARGO_TARGET_ROOT:-${HOME}/.cache/cognicode/cargo-targets}"
      MUTANTS_TARGET_DIR="${CARGO_TARGET_ROOT}/mutants"
    fi
    mkdir -p "${MUTANTS_TARGET_DIR}"
    # `--gitignore true` is not a nicety: cargo-mutants copies the tree
    # before it mutates anything, and the copy honours the ignore rules.
    # Without it every mutant copies the whole working tree — 6.6 GB, most
    # of it the sandbox fixture repositories — and the copy either fills
    # the temporary volume or takes longer than the mutation it is
    # supposed to measure.
    #
    # The copy also honours the OPERATOR'S global gitignore, where a bare
    # `bin/` sits next to the Go artefacts. That is why the repository
    # declares `!crates/cognicode-runtime/src/bin/`: without the
    # exemption the copy is missing the `explorer-api` and `explorer-mcp`
    # sources, and the copy fails to build with "can't find bin" — the
    # same failure the cli and explorer exemptions were written for.
    #
    # The test binaries run under `scripts/mem-capped-runner.sh`: a mutant
    # that loops while it allocates (a BFS cursor that never advances) must
    # die at the ceiling, not exhaust the machine's memory.
    #
    # The ceiling is paid by every job at the same time, so the two numbers
    # belong together: the runner's ceiling times this script's parallelism is
    # the memory the step asks for at once, and it has to fit in what the
    # machine has free. When it does not fit, the mutants that the tests kill
    # in seconds are reported as timeouts instead — a red gate that says
    # nothing about the code. `scripts/ci/test_mutant_memory_budget.py` keeps
    # the two numbers from drifting apart, and it is also the contract that
    # fixes the parallelism at ONE job: the measurement has to be
    # attributable to be a certificate.
    #
    # The ceiling and the budget it has to fit in live in ONE file, which
    # the focal route (`scripts/mutate-file.sh`) loads as well: two scripts
    # naming the same budget are two sources of truth, and the second one
    # rots. The value does not change — it is the 2 GiB the runner already
    # imposed — but it is now written where both routes read it.
    # Fail-closed: without the ceiling there is no measurement to certify.
    # The step is not run under a default that nobody declared: an hour of
    # mutants measured against an implicit ceiling is an hour whose verdict
    # says nothing about the ceiling the certificate names.
    if ! source "${REPO_ROOT}/scripts/mutants-budget.sh"; then
      printf 'FAIL: scripts/mutants-budget.sh no se puede leer: el techo del paso de mutantes no tiene dueño\n' >&2
      overall_rc=1
    elif [ -z "${MUTANTS_GATE_VMEM_KIB:-}" ]; then
      printf 'FAIL: scripts/mutants-budget.sh no declara el techo del paso de mutantes\n' >&2
      overall_rc=1
    else
      MUTANT_PKG_ARGS=()
      for pkg in ${R_MUTANT}; do
        MUTANT_PKG_ARGS+=(-p "${pkg}")
      done
      run_step "mutants" env CARGO_TARGET_DIR="${MUTANTS_TARGET_DIR}" \
        CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER="${PWD}/scripts/mem-capped-runner.sh" \
        MUTANT_TEST_VMEM_KB="${MUTANTS_GATE_VMEM_KIB}" \
        cargo mutants -j "${MUTANTS_JOBS}" \
        --gitignore true \
        "${MUTANT_PKG_ARGS[@]}" \
        -- -- --test-threads=4
    fi
  fi
fi

printf '\n=== test-gate rc=%d ===\n' "$overall_rc"
exit "$overall_rc"
