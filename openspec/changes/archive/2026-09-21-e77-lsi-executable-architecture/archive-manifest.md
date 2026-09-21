# Archive Manifest — e77 LSI Executable Architecture

> Cycle: A-lite | Milestone: M10 — Executable Architecture (e77 first slice) | Phase: archive | Date: 2026-09-17

## Cycle summary

| Phase | Status | Artifact |
|-------|--------|----------|
| explore | DONE | `explore-report.md` |
| propose | DONE | `proposal.md` |
| design | DONE | `design.md` |
| tasks | DONE | `tasks.md` |
| apply | DONE | commits (see below) |
| verify | DONE | `verification-report.md` |
| archive | DONE | this document |

## Commits

This cycle is **single-impl + single-archive**. The implementation
commit lands all of WU0..WU6 in one go because the module is
strongly connected (WU1's types are WU2's service surface, WU3's
matcher consumes WU1's rules, etc.) and a multi-commit split would
create intermediate states where the tests do not compile.

* Implementation commit: `feat(e77): executable architecture
  (constraint model + admission + evaluator + self-host + UAT)`.
  Adds:
  - `crates/cognicode-core/src/domain/architecture/{mod,constraint,use_parser}.rs`
  - `crates/cognicode-core/src/application/architecture/{mod,admission,evaluator,registry}.rs`
  - `crates/cognicode-core/tests/architecture_drift_e2e.rs`
  - `crates/cognicode-core/tests/architecture_self_host_e2e.rs`
  - `docs/analysis/e77-architecture-ownership-map.md`
  - Updates to `domain/mod.rs` and `application/mod.rs`.

* Archive commit: `docs(e77): archive LSI Executable Architecture
  (M10 first slice)`. Adds the four lifecycle artifacts
  (`design.md`, `tasks.md`, `explore-report.md`,
  `verification-report.md`, this `archive-manifest.md`) and
  updates the umbrella `state.yaml`.

## Capability deltas

e77 introduces **no new capability spec** in `openspec/specs/`
because:

* The drift findings use the existing `Finding` model
  (`openspec/specs/lsi-finding-model/spec.md` is unchanged).
* The constraint admission is internal — it does not face an
  external consumer (yet).
* The self-host test is the first real consumer and lives
  inside the crate.

When the next external consumer arrives (e.g. an Explorer view
of architecture drifts, or a CI runner that consumes the
findings), a capability spec will be authored in a follow-on
cycle.

## Consumer-count checkpoint

Per the proposal's gate, e78 (Pack Ecosystem) opens only if there
are at least two real consumers of the architecture module.

**Confirmed consumers**: 1 (`cognicode-core` self-host test).

**Decision**: **e78 DEFERRED**. Revisit trigger:

* Detector packs need the manifest surface (capability
  advertisement).
* Another extension pack needs authority / conformance.
* A third-party plugin reaches the admission surface.

The deferral is recorded in the umbrella `state.yaml`.

## Honest signals

The self-host test (`self_host_evaluator_finds_zero_drift_on_clean_source`)
is `#[ignore]` because it fails on the current source. The 4
documented drifts are:

```text
src/domain/evidence_kernel/bootstrap.rs:71
  use infrastructure::evidence_kernel::in_memory::InMemorySchemaRegistry
src/domain/traits/code_verifier.rs:6
  use application::error::AppResult
src/domain/behaviors/runtime.rs:43
  use application::behaviors::Clock
src/domain/behaviors/runtime.rs:44
  use application::intelligence_log::CausalRecorder
```

These are recorded in `docs/debts/DEBT-SDDK-004.md` as a debt.
Fixing them is **out of scope** for e77 — they require
architectural work that belongs to a separate cycle.

## M10 status

M10 (Executable Architecture) **first slice CLOSED** via e77.

What is done:

* Typed constraint vocabulary (Layer, Forbidden, Namespace).
* Admission flow with promoted-admitter gate and idempotency.
* Evaluator that produces real `Finding` instances.
* Self-host test proving the evaluator is load-bearing.
* UAT adversarial matrix proving the load-bearing property
  ("ADR text alone → ZERO findings").

What is **not** done (and intentionally out of scope):

* Persistence adapter.
* Cross-crate checking.
* Pack format / capability advertisement.
* Wire-up to the EvidenceBundle / PolicyGate pipeline (the
  shape is ready; the wire is a follow-on).
* Remediation of the 4 real drifts.

## Roadmap update

Per the user directive (2026-09-17), the planned remaining
roadmap is:

| Cycle | Capability | Status |
|-------|-----------|--------|
| e78 | Pack Ecosystem | DEFERRED (consumer-count checkpoint failed) |
| e79 | AI Foundation + read-only agents | NEXT |
| e80 | AutomatedAuthorPolicy + Fix Agent | pending |
| e81 | Historical Replay + OPTIMIZE/CONFIRM | pending |
| e82 | FailureRegime + Shadow Evaluation | pending |
| e83 | Held-out promotion + governed self-improvement | closure |

M11 (AI Agents) remains BLOCKED until
`AutomatedAuthorPromotionPolicy` lands (e80).
