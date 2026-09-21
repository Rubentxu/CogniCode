# Explore Report — e77 LSI Executable Architecture

> Cycle: A-lite | Milestone: M10 — Executable Architecture (e77 first slice) | Phase: explore | Date: 2026-09-17

## Problem

CogniCode's own `cognicode-core` crate has a stated architecture
(domain must not import I/O crates; application must not import
infrastructure; etc.) that lives **only in prose** — in
`docs/adr/` and in code review comments. There is no mechanical
check. Drift happens silently, gets caught late (if at all), and
becomes legacy.

The proposal (e77) converts this intent into machine-executable
constraints. Before designing the module, we need to map the
existing surface:

* What rule families are real today?
* Which existing types can we reuse?
* Where does the new module live in the layer cake?

## Surface map

### Existing types we will reuse (do not reinvent)

| Type | Where | Why we reuse it |
|------|-------|-----------------|
| `NamespacedName` | `domain::naming` | Stable `namespace.name` grammar (cycle e63). Constraint ids use the same grammar. |
| `Finding` / `FindingKind` | `domain::findings::finding` | Drift findings are normal findings with `architecture.*` kinds. No new finding type. |
| `DetectorExecutionRef` / `DetectorDigests` | `domain::findings::{detector_ir,digest}` | Drift findings carry a `DetectorExecutionRef` (synthetic, but the type is real). |
| `FindingGate` | `domain::findings::finding` | The same gate the existing findings use. |
| `AdmitterRole` (new) | `domain::architecture::constraint` | Needed because the existing `AdmissionSource` is for detector admission, not architecture admission. Distinct type, distinct purpose. |

### What we explicitly do **not** reuse

* `AdmissionSource` (`domain::trust`) — this governs detector
  admission. The architecture module needs its own admission
  vocabulary because the *who* is different (CI / human
  promoter, not "human curated / ai generated").
* `DetectorAuthority::Candidate` / `Gated` — these govern
  whether a detector may block CI. The architecture module maps
  `AdmitterRole → DetectorAuthority` at evaluation time
  (`Promoter/CiPromoter → Gated`).

### Layers

The new module is split into two layers:

```text
domain::architecture
  ├── constraint    (data model)
  └── use_parser    (pure string -> struct)

application::architecture
  ├── admission     (orchestration: role, idempotency, clock)
  ├── evaluator     (match logic + drift finding construction)
  └── registry      (composition root: admission + evaluator)
```

Both layers stay I/O-free. The evaluator receives its source as
a plain `ArchitectureSource` data structure; nothing in the
module touches the filesystem. The self-host test wires the
filesystem walk; the production code can wire a different
collector without changing the module.

## Real rule families (today, in prose)

After reading `docs/adr/`, the current architecture is:

1. **Layer rule**: `domain` may not import `infrastructure`,
   `application`, or any I/O crate. Already enforced by review.
2. **Forbidden dependency**: domain may not import
   `sqlx`, `tokio`, `reqwest`, `hyper`. Same.
3. **Namespace boundary**: `domain::evidence_kernel` may not
   drive UI / presentation. Same.

These three are the **only** rule families e77 needs to encode.
We deliberately do **not** enumerate more — adding a fourth rule
requires evidence (a real drift the three above cannot express).

## Real drifts the evaluator must find

Running the evaluator on the current `cognicode-core` source
(found by the self-host test, ignored but reproducible):

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

These are documented in `docs/debts/DEBT-SDDK-004.md` and are
**out of scope** for e77 — fixing them is architectural work that
belongs to a separate cycle.

## Risk analysis

| Risk | Mitigation |
|------|-----------|
| Parser misses a `use` shape (false negative). | Tests cover the most common shapes; if a real shape fails, we extend the parser. We do not add `syn` as a runtime dep. |
| Constraint id collisions across the org. | `NamespacedName` grammar requires `a.b.c`; collisions are caught by `AlreadyAdmitted`. |
| Drift findings pollute the gate. | Findings use `DetectorAuthority::Gated`, which means the existing `FindingGate` decides whether they block. No new gate surface. |
| Admitter role proliferation. | Two roles for now: `HumanPromoter` and `CiPromoter`. `Other` is rejected. Adding a third role requires a cycle. |
| Persistence: admitted set lost on restart. | Out of scope; the in-memory service is sufficient for the first slice. A Postgres adapter is a follow-on. |

## Non-goals (verified)

* **No LLM ADR parsing.** ADRs are referenced by id; the
  evaluator never sees their prose.
* **No multi-crate checking.** Only `cognicode-core` is in
  scope.
* **No automatic remediation.** Findings are reported; humans
  act.
* **No generalised Pack format.** Deferred to e78 (consumer-count
  checkpoint required).

## Conclusion

The surface map is clean. The new module fits in two existing
layers without disturbing anything. The three rule families are
exactly the ones the codebase already enforces manually. The
self-host test is the proof of load-bearing; the ignored test
documents 4 real drifts as honest signal, not as a hidden bug.
