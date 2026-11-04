#!/usr/bin/env bash
# Cargo runner for the mutation step: runs a test binary with a ceiling on
# its address space, so a mutant that loops while it allocates dies with an
# allocation failure (a caught mutant) instead of taking the machine down.
# Measured 2026-11-04: `dominators.rs` `head += 1` -> `*=` grew a Vec until
# the kernel OOM-killed test binaries at 71, 25 and 19 GB resident.
# Only the test binary is capped; rustc and the linker are not.
set -euo pipefail
ulimit -v "${MUTANT_TEST_VMEM_KB:-6291456}"
exec "$@"
