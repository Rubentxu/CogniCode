# Exploration report — PR-306 merge-gate failures

**Cycle:** `p-c1fac1fea05615c6/pr306-ci-unblock`
**Date:** 2026-09-28
**Observed evidence:** `gh run view 36456416891 --log-failed` (exit 0,
`sha256:8aa4c1e83659543ae1a324d174405704ecca6ee3fa9a9264dcb45d972aa2247e`)
**HEAD at diagnosis:** `6dd8530a` on `docs/cp2-a012-closure`, 21 commits ahead
of `origin/main`. PR #306 `DRAFT`, `mergeStateStatus=BLOCKED`.

## Finding 1 — published version drift (4 of 4 contract-test failures)

**Observed.** `fmt + clippy` job: `TOTAL: 43 passed, 4 failed`. Assertions:

- `assert manifest["version"] == "0.99.2"` (`test_product_manifest.py:32`)
- `assert current in claimed, f"SECURITY.md does not mention the actual current version {current}"` (`test_oss_foundation.py:218`), with `current = 0.100.0`

**Cause.** `4702e471` (MINOR bump to 0.100.0) modified exactly three files —
`Cargo.toml`, `Cargo.lock`, `CHANGELOG.md` — and **zero** files under
`product/`. Verified with `git show --name-only 4702e471`. The three
published documents derive `version` from `[workspace.package] version`
(`generate_product_manifest.py:70`, `generate_support_matrix.py:117`), so
they went on announcing 0.99.2 while the binaries they describe report
0.100.0.

**Contributing cause.** `test_product_manifest.py` asserted against a
hardcoded literal. The drift was not detected at the bump; it surfaced as
four red gates later, where the correct response (regenerate the artefact)
is the one a literal makes look wrong.

## Finding 2 — lib-test isolation defect (1 failure in `cognicode-core --lib`)

**Observed.** `test pineado` job: `2230 passed; 1 failed; 12 ignored`, panic
at `crates/cognicode-core/src/application/services/file_operations.rs:3171`
with message `Should return ok result` — the second of two
`assert!(result.is_ok())` calls in
`test_retrieve_and_verify_deterministic`.

**Not reproducible locally.** Full `cargo test -p cognicode-core --lib` gave
`2231 passed; 0 failed` before any change. Three runs of the seven
`retrieve_and_verify` tests and six runs of the rustc-not-found test alone
were all green. The subject test is innocent.

**Cause.** `test_retrieve_and_verify_rustc_not_found`, in the same module,
removed `rustc` from the process-wide `PATH` and restored it afterwards.
`#[serial]` was why nobody connected the two: it orders the three
`#[serial]` tests *in that module* against each other, but a lib test binary
runs all ~2200 tests concurrently, so the window in which `PATH` lacked
`rustc` was visible to every non-`#[serial]` sibling. A sibling reaching
the toolchain check received `rustc not found` and failed for a reason
unrelated to its own subject.

**Underlying defect.** The absent-toolchain precondition was only
reachable by mutating process-global state, because `retrieve_and_verify`
gated on a hardcoded `which::which("rustc")` in the application layer
(`file_operations.rs:1883`). That call duplicated knowledge belonging to
the adapter and bypassed the `CodeVerifier` port the service already
injects (`FileOperationsService::new` takes `Arc<dyn CodeVerifier>`).

**In-repo precedent for the fix.** `GitRenameEvidenceAdapter`
(`infrastructure/git/rename_evidence.rs:30-38`) documents exactly this
constraint in its own field comment: injecting the program name so the
"git absent from PATH" case is reproducible *"WITHOUT mutating the
process-wide environment (Rust 2024 makes `std::env::set_var` unsafe and
global mutation would race sibling tests)."*

## Classification

The two findings share no cause. Reading a red `merge-gate` as one problem
invites a fix that addresses neither; they were closed as two atomic
commits.
