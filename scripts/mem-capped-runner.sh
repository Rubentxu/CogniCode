#!/usr/bin/env bash
# Cargo runner for the mutation step: runs a test binary with a ceiling on
# its address space, so a mutant that loops while it allocates dies with an
# allocation failure (a caught mutant) instead of taking the machine down.
# Measured 2026-11-04: `dominators.rs` `head += 1` -> `*=` grew a Vec until
# the kernel OOM-killed test binaries at 71, 25 and 19 GB resident.
# Only the test binary is capped; rustc and the linker are not.
#
# WHY 2 GiB AND NOT 6: the ceiling is paid by every mutant at the same time.
# The gate measures the mutants ONE AT A TIME — the reason is attribution, and
# it is written in `scripts/test-gate.sh` — so the ceiling is paid by a single
# test binary. It was 2 GiB for a different reason first: the gate used to run
# eight mutants in parallel, and eight test binaries growing to 6 GiB each ask
# for 48 GiB of address space at once — more than the machine has free
# (measured 2026-11-05: 94 GiB in total, about 39 GiB free). The
# oversubscription is not paid by the mutant that overshoots: it is paid by its
# neighbours, and a mutant the tests kill in seconds gets reported as a
# TIMEOUT. That is a red gate that says nothing about the code.
#
# Measured 2026-11-05 on the whole scope, 574 mutants, eight jobs: with a 6 GiB
# ceiling, 8 timeouts; with a 2 GiB ceiling, none, and every mutant caught.
# The ceiling is therefore a property of the parallelism, not of the crate, and
# it stays a knob (`MUTANT_TEST_VMEM_KB`) for a crate whose own tests need
# more. What it may not do is grow back without re-measuring the whole scope:
# `scripts/ci/test_gate_contracts.py` keeps the two numbers from
# drifting apart, and it is also what fixes the parallelism at one job. On a
# machine with less free memory than this one, the ceiling is what comes down,
# not the attribution: the measurement stays serial.
set -euo pipefail
ulimit -v "${MUTANT_TEST_VMEM_KB:-2097152}"
exec "$@"
