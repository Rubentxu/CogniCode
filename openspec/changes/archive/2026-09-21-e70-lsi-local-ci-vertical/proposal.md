# e70 — Local CI Orchestration + Explanations

## Status

```text
e69 implementation   CLOSED
e69 verification     PASS
e70 implementation   CLOSED
e70 verification     PASS
e69 SDDK lifecycle   not formally instantiated
e70 SDDK lifecycle   not formally instantiated
```

## Goal

Glue e67 + e68 + e69 into a single local-first orchestration vertical
that produces a structured `LocalVerticalReport`. The vertical
consumes:

- `FactDelta` (e68 WU1) for what changed
- `AffectedWorkPlan` (e68 WU2) for what to run
- `EvidenceBundle` + `PolicyDecision` (e69 WU3) for what came back

and emits a `LocalVerticalReport` with two **separately structured**
explanations:

- `why_scheduled` — explains selection (which WorkIds and why)
- `why_decided` — explains authority (which PolicyDecision and why)

```
source change
   ↓
FactDelta                                ← e68
   ↓
AffectedWorkPlan                         ← e68
   ↓
local execution (WorkExecutor trait)
   ↓
WorkResults
   ↓
EvidenceBundle                           ← e69
   ↓
PolicyGate.evaluate                      ← e69
   ↓
PolicyDecision
   ↓
LocalVerticalReport {
   why_scheduled: Vec<WhyScheduledEntry>,
   why_decided: Vec<WhyDecidedEntry>,
}
```

## Non-goals (deferred to e70.x or later)

- GitHub Actions remote.
- Jenkins, distributed workers, cloud runtimes.
- Real shell-out to cargo/just/cogh (we use in-memory executors
  in e70; the trait reserves the slot).
- Persistent EvidenceBundle (e69 was in-memory; e70 stays in-memory).
- The `why_scheduled` / `why_decided` CLI renderer (a consumer
  can be added later; e70 only defines the data structures).

## Architectural decisions

1. **`WorkExecutor` is a trait, not a concrete type**. The first impl
   is `InMemoryWorkExecutor`, which holds a map of
   `WorkId → Box<dyn Fn() -> Vec<ProducerOutput>>` and returns
   the registered outputs. This keeps e70 deterministic and testable
   while reserving the slot for a real async-shell-out executor
   (e70.x or later). It is the only trait e70 introduces.

2. **Two explanations, not one**. Per the user's explicit framing:
   - `why_scheduled` answers "why did we run this work?" — it
     consumes `SchedulingReason` (from e68).
   - `why_decided` answers "why did the gate end in Pass/Warn/
     Block/InsufficientEvidence?" — it consumes `PolicyReason`
     (from e69 WU3).
   These are deliberately different types. A work item has exactly
   one `why_decided`; it may have one or more `why_scheduled`
   reasons (e.g. removed+changed+added → three reasons, one
   disposition).

3. **LocalVerticalReport is a derived value**. It is not persisted
   as a canonical Fact. It is the in-process operational summary
   for one vertical run.

4. **`LocalVerticalError` is total**. Every error variant names a
   concrete reason (no catch-all `Other`). The error type is the
   contract between e70 and downstream consumers.

5. **Determinism**: the report's `why_scheduled` is sorted by
   `(work_id, reason)`; `why_decided` is sorted by `(work_id,
   rule)`. The same vertical run produces the same report.

## Architecture budget

- Crates/modules touched:
  - `crates/cognicode-core/src/application/local_ci/{mod,executor,
    vertical, why}.rs`
- New domain types: **none**.
- New dependencies: **none**.
- New traits: exactly one (`WorkExecutor`).
- APIs NOT modified: `FactDelta`, `AffectedWorkPlan`,
  `EvidenceBundle`, `PolicyDecision`, `SchedulingReason`, all
  upstream modules.

## Risk budget

- Allowed uncertainties:
  - Exact shape of `WhyScheduledEntry` and `WhyDecidedEntry`
    fields (the user can refine post-e70 once they see real
    consumption).
- Mandatory fallbacks:
  - Conservative fallback of e68 planner is preserved end-to-end.
  - Gate's "absence is never Pass" rule is preserved end-to-end.
- Tolerable regressions: none.
- Invariant metrics:
  - All upstream regression (e66/e67/e68/e69) remains green.
  - `just lsi-equivalence` 7/7.
  - New e70 WU3 includes an end-to-end "demo" test that walks a
    small scenario (e.g. read A → A changed → re-run → gate Pass).

## Decomposition (three WUs)

### WU1 — WorkExecutor trait + InMemoryWorkExecutor + LocalVertical

- `WorkExecutor` trait: `fn execute(&self, work: &WorkId) ->
  Vec<ProducerOutput>`.
- `InMemoryWorkExecutor` impl: holds
  `BTreeMap<WorkId, Box<dyn Fn(...) -> Vec<ProducerOutput>>>` for
  deterministic registration.
- `run_local_vertical(delta, deps, exec, spec) -> Result<
  LocalVerticalReport, LocalVerticalError>` orchestrates
  delta → planner → execute (per affected WorkId) → aggregate
  evidence → gate → report.

UAT:
- empty deps → empty report, no producer executed
- one work affected → one execution, one bundle, one decision,
  one why_scheduled + one why_decided
- producer failure preserved through bundle (gate may still
  Block); why_decided explains
- conservative fallback of e68 preserved: Unknown/Unaffected
  work items are not executed

### WU2 — why_scheduled + why_decided

- `WhyScheduledEntry { work, disposition, reasons }` — pulled
  verbatim from e68 `WorkDecision`.
- `WhyDecidedEntry { work, decision, reasons }` — pulled verbatim
  from e69 `PolicyDecision`.
- `LocalVerticalReport { run_id, from_snapshot, to_snapshot,
  why_scheduled: Vec<WhyScheduledEntry>, why_decided:
  Vec<WhyDecidedEntry> }`.
- Sorting: stable, by `work_id`.

UAT:
- one work item, one decision → one of each entry
- reasons preserved through the round-trip (SchedulingReason +
  PolicyReason, no lossy translation)
- sort is stable

### WU3 — End-to-end adversarial + invariants

- `demo_test: works` — a minimal walkthrough:
  - source: "fact A read by work X"
  - snapshot N+1: "fact A changed"
  - planner: Affected
  - execution: produces one Evidence entry
  - gate: Pass
  - report: 1 why_scheduled + 1 why_decided
- Conservative fallback preserved end-to-end:
  - one work Unknown → not executed → no bundle → no decision
  - the planner's Unknown propagates correctly
- Determinism: same vertical run → identical report bytes.
- Adversarial cases:
  - empty delta → no work to schedule → empty why_scheduled
  - all-Unaffected → empty why_scheduled
  - one work Affected, one work Unknown → mixed report
  - decision InsufficientEvidence → why_decided reflects it
  - decision Block → why_decided reflects it
