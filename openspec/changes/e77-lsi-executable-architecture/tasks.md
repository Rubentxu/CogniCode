# Tasks — e77 LSI Executable Architecture

> Cycle: A-lite | Milestone: M10 — Executable Architecture (e77 first slice) | Phase: tasks | Date: 2026-09-17

## Decomposition

e77 is decomposed into 7 work units (WU0..WU6), each independently
mergeable. Each WU is a unit of review and corresponds to a
verifiable slice of the proposal.

| WU | Title | Files | Tests | Status |
|----|-------|-------|-------|--------|
| WU0 | Architecture ownership map | `docs/analysis/e77-architecture-ownership-map.md` | n/a (doc) | DONE |
| WU1 | ArchitectureConstraint model | `crates/cognicode-core/src/domain/architecture/{mod,constraint}.rs` | implicit (compiles + used by WU2..6) | DONE |
| WU2 | Candidate → approved admission | `crates/cognicode-core/src/application/architecture/{mod,admission}.rs` | 4 unit tests in `admission.rs` | DONE |
| WU3 | Architecture evaluator | `crates/cognicode-core/src/application/architecture/evaluator.rs` (model + match logic) | 6 unit tests in `evaluator.rs` | DONE |
| WU4 | Drift finding construction | `crates/cognicode-core/src/application/architecture/evaluator.rs` (`build_finding`) | covered by WU3 tests | DONE |
| WU5 | CogniCode self-host architectural mutations | `crates/cognicode-core/tests/architecture_self_host_e2e.rs` | 2 + 1 ignored | DONE |
| WU6 | UAT + adversarial matrix | `crates/cognicode-core/tests/architecture_drift_e2e.rs` | 10 tests | DONE |

**Total: 32 passing tests, 1 ignored, 0 failing.**

## WU0 — Architecture ownership map

**Goal**: codify the architecture of `cognicode-core` as a single
source of truth document. No code changes.

**Done when**: `docs/analysis/e77-architecture-ownership-map.md` is
present, lists the three concrete rules, and links to the future
implementation files.

## WU1 — ArchitectureConstraint model

**Goal**: introduce the typed constraint vocabulary.

**Done when**:

* `domain::architecture::constraint` exists with
  `ArchitectureConstraint`, `ConstraintCandidate`,
  `ArchitectureConstraintKind`, the three rule structs, the
  `Admitter` type, and the error type.
* `domain::architecture::use_parser` exposes
  `parse_use_lines` with at least 5 unit tests.
* `cargo check -p cognicode-core --lib` is clean.

## WU2 — Candidate → approved admission

**Goal**: turn a candidate into an admitted constraint or reject
it.

**Done when**:

* `application::architecture::admission::ArchitectureAdmissionService`
  has `admit`, `admitted`, `from_admitted`, `new`, and a clock
  port.
* Non-promoted admitters are rejected.
* Already-admitted ids are rejected (idempotency).
* Empty rule bodies are rejected.
* 4 unit tests pass.

## WU3 — Architecture evaluator

**Goal**: produce findings for admitted constraints.

**Done when**:

* `application::architecture::evaluator::ArchitectureEvaluator`
  evaluates each of the three rule kinds.
* The evaluator is stateless and deterministic (same input →
  same report).
* Source with no `use` yields zero findings (positive control).
* 6 unit tests pass.

## WU4 — Drift finding construction

**Goal**: emit drift findings as regular `Finding` instances.

**Done when**:

* Findings carry `architecture.*` namespaced kinds.
* Detector execution reference has `DetectorAuthority::Gated`.
* Synthetic evidence ids are deterministic (FNV-1a of
  `file:line`).
* The causal chain explains `Source → Flow → Sink`.
* Covered by WU3 tests (no additional tests; the WU is a
  refactor that the existing tests exercise).

## WU5 — CogniCode self-host architectural mutations

**Goal**: prove the evaluator is load-bearing by running it on
real `cognicode-core` source.

**Done when**:

* `tests/architecture_self_host_e2e.rs` walks the source tree,
  admits the three canonical constraints, runs the evaluator,
  and asserts no findings clear the gate.
* A second test (`self_host_evaluator_finds_real_drift_in_synthetic_fixture`)
  proves the evaluator finds real drifts in a fixture, so the
  evaluator is load-bearing (not trivial).
* A third test (`self_host_admitted_constraints_are_promoted`)
  guards the admission flow.
* The ignored test (`self_host_evaluator_finds_zero_drift_on_clean_source`)
  reproduces the 4 real drifts documented in DEBT-SDDK-004.

## WU6 — UAT + adversarial matrix

**Goal**: codify the adversarial matrix as executable tests.

**Done when**:

* `tests/architecture_drift_e2e.rs` has 10 tests, one per row
  of the matrix declared in the proposal.
* Row 1 (`adr_text_alone_produces_zero_findings`) is the
  load-bearing case.
* All 10 tests pass.

## Verification plan

After all WUs land:

```bash
# Unit tests for the new module
cargo test -p cognicode-core --lib architecture::

# E2E adversarial
cargo test -p cognicode-core --test architecture_drift_e2e

# Self-host (including the ignored test that reproduces real drifts)
cargo test -p cognicode-core --test architecture_self_host_e2e
cargo test -p cognicode-core --test architecture_self_host_e2e -- --ignored
```

## Out-of-scope tasks (for follow-on cycles)

These are **not** WUs of e77. They are recorded so the next
planner sees them.

* **Persist admitted constraints to Postgres** — requires an
  adapter in `infrastructure::architecture`. Not justified by
  the current consumer count (1).
* **Generalised Packs** (e78) — DEFERRED per consumer-count
  checkpoint (see `state.yaml`).
* **Cross-crate checks** (e.g. `cognicode-cli` respecting the
  same rules) — requires a collector. Not justified yet.
* **Wire drift findings into the EvidenceBundle pipeline** —
  requires WU4 to land first; the synthetic evidence ids are
  already shaped to be replaceable with real ones.
* **Revocation flow** — out of scope. The admission set is
  monotonic.
