# Design — e77 LSI Executable Architecture

> Cycle: A-lite | Milestone: M10 — Executable Architecture (e77 first slice) | Phase: design | Date: 2026-09-17

## Purpose

This document is the **technical design** that closes the gap between
the [proposal](proposal.md) and the [tasks](tasks.md). It captures
the decisions that did not fit in either of those documents: the
layer placement, the parser choice, the authority mapping, the
idempotency semantics, and the explicit non-goals.

## Layer placement

The architecture module is split across two layers following the
existing hexagonal pattern:

| Component | Layer | Reason |
|-----------|-------|--------|
| `domain::architecture::constraint` (types) | `domain` | Pure data; no I/O. |
| `domain::architecture::use_parser` (parser) | `domain` | Pure string → struct transformation. |
| `application::architecture::admission` (service) | `application` | Orchestration (role check, idempotency, timestamp). |
| `application::architecture::evaluator` (evaluator) | `application` | Crosses multiple domain types; produces findings. |
| `application::architecture::registry` (facade) | `application` | Composition root for admission + evaluator. |

`domain::architecture` does **not** import `sqlx`, `tokio`, or any
adapter. `application::architecture` does **not** import
`infrastructure::*` either; the evaluator receives its source as a
plain data structure.

## Parser choice: hand-rolled, not `syn`

`cognicode-core` does **not** depend on `syn`. `syn` lives in
`cognicode-macros` and dragging it into the domain layer would
inflate compile times and add a heavy parser to a layer that is
meant to be lightweight.

We wrote a hand-rolled, comment-aware parser for `use ...;`
statements. It supports:

* `use crate::foo::bar;`
* `use crate::foo::{a, b as c};` (normalised to `foo`)
* `use self::foo;` / `use super::foo;` (prefixes stripped)
* Line comments (`//`) and block comments (`/* ... */`).

What it does **not** support:

* Macros that expand to `use` (e.g. `pub use foo::*;`).
* Generic `use` declarations with attribute macros.
* String-literal awareness (a `use` inside a doc-string would be
  reported as if real).

For the three rule families of e77 (layer, forbidden, namespace
boundary), the trade-off is acceptable. If we need richer syntax
later, the parser is replaceable without changing the evaluator or
the constraint model.

## Authority mapping

The constraint model carries its own authority vocabulary
(`AdmitterRole`), separate from the detector authority vocabulary
(`DetectorAuthority`). The two are linked at evaluation time:

```text
ConstraintCandidate  --[Admitter::HumanPromoter | CiPromoter]-->  ArchitectureConstraint
                                                                            |
                                                                            v
                                                              DetectorAuthority::Gated
                                                              (only for admitted constraints)
```

Concretely:

* `AdmitterRole::Other` → cannot admit → no constraint → no finding.
* `AdmitterRole::HumanPromoter` or `CiPromoter` → admits →
  constraint is created → evaluator emits findings with
  `DetectorAuthority::Gated` (so the existing `FindingGate` can
  decide whether they block).

The reason for keeping the two vocabularies separate is that the
*admitter* role governs who may **propose a rule**, while the
*detector* authority governs who may **block CI**. Conflating them
would force every rule admission to also be a CI gating decision,
which is the kind of leaky abstraction we want to avoid.

## Idempotency semantics

The admission service is idempotent **on id**:

* A second admission of the same `ArchitectureConstraintId` is
  rejected with `AdmissionError::AlreadyAdmitted`.
* The admitted set is preserved across `from_admitted`
  (re-hydration from disk) so a persistence adapter round-trip
  does not silently re-admit constraints.

This is a stronger property than "no duplicate ids": it is
*monotonic*. The admitted set can only grow by admission; it
cannot be replaced wholesale. Removal is **not** in scope for the
first slice — it would require a separate `revoke` flow that
documents the impact (which findings stop being emitted) and a
policy gate (e.g. only an `HumanPromoter` may revoke a constraint
they admitted).

## Findings are normal findings

Drift findings are emitted as **regular `Finding` instances** with
a namespaced kind (`architecture.layer_dependency`,
`architecture.forbidden_dependency`, `architecture.namespace_boundary`).
They go through the same gate machinery as detector findings; no
new gating surface is introduced. This is what makes the e77
output composable with e69 (EvidenceBundle) and e73 (PolicyGate)
without modification.

The drift-finding construction uses a synthetic `DetectorExecutionRef`
with `DetectorAuthority::Gated` because the constraint is admitted.
The digests are deterministic (derived from the constraint kind)
and pass the structural validation. They are not semantically
meaningful — they are placeholders the application can later
replace with real digests when the evaluator is wired into the
canonical detector pipeline.

## Self-hosting seam (e76) reuse

The evaluator is **the first real consumer** of the self-hosting
seam (e76): it reads the source of `cognicode-core` itself. The
self-host test (`tests/architecture_self_host_e2e.rs`):

1. Walks `crates/cognicode-core/src/{domain,application,infrastructure}`.
2. Admits three canonical constraints (the ones from the ownership
   map: domain no infrastructure, domain no application,
   evidence_kernel no presentation).
3. Runs the evaluator on the full source.
4. Asserts that no findings clear the `FindingGate` (B class +
   medium risk).

The test **currently fails** (4 real drifts documented in
`docs/debts/DEBT-SDDK-004.md`) — this is the honest signal. A
green test would mean either (a) the source is clean, or (b) the
evaluator is not load-bearing. Because we know there are drifts,
a green test would be the dangerous case.

We have left the test as `#[ignore]` so the rest of the suite
passes in CI, but the drifts are recorded and the test can be
re-run with `-- --ignored` to reproduce.

## Non-goals (this slice)

* **No pack format** (manifest / capability / authority /
  conformance). The e78 cycle will revisit this once there are at
  least two real consumers.
* **No multi-crate checking.** The evaluator only knows about
  `cognicode-core`. Adding cross-crate checks requires a
  collector that walks other crates' source, which is out of
  scope.
* **No ADR parsing via LLM.** ADRs are referenced by stable id
  (e.g. `ADR-046`); the admission flow takes the reference as a
  string, never the prose. The fundamental rule — "ADR text alone
  has ZERO execution/gating authority" — depends on this.
* **No automatic remediation.** The evaluator reports; humans
  change code (or open a `ChangeProposal` via the e73 surface).
* **No persistence adapter.** The admission service is
  in-memory; a Postgres-backed adapter will be a follow-on cycle
  if needed (likely as part of e83 / Continuous Improvement
  closure).

## Test topology

* **Unit tests** in `domain::architecture::use_parser` (6 tests)
  and `application::architecture::{admission,evaluator,registry}`
  (14 tests) cover the data model, the admission flow, the
  evaluator logic, and the registry composition.
* **E2E tests** in `tests/architecture_drift_e2e.rs` (10 tests)
  cover the adversarial matrix, including the load-bearing case
  (`adr_text_alone_produces_zero_findings`).
* **Self-host test** in `tests/architecture_self_host_e2e.rs`
  (2 tests + 1 ignored) covers the self-host property and the
  synthetic-fixture load-bearing guarantee.

Total: **20 unit + 12 E2E + 1 ignored = 32 passing tests** for e77.

## Update to AGENTS.md

The AGENTS.md **does not** need to change. The architecture rules
of `cognicode-core` (domain may not import `sqlx`/`tokio`,
application may not import infrastructure) are already enforced by
code review; e77 makes them mechanical. The new module follows the
existing conventions.
