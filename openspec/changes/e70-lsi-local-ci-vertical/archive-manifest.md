# Archive Manifest — cycle e70 — Local CI Orchestration + Explanations

> Cycle: A-lite (degraded-but-governed) | Milestone: M8 — Scheduler + CI | Phase: archive | Date: 2026-09-17

## Cycle identity

| Item | Value |
|------|-------|
| Cycle | e70 |
| Milestone | M8 — Scheduler + CI (vertical: planner + executor + bundle + gate + explanations) |
| Requirement | glue e67+e68+e69 into a single local-first orchestration vertical |
| Path | A-lite — degraded-but-governed (DEBT-SDDK-003) |
| Base HEAD | `b39bb524` (e69) |
| Spec delta | none (foundation cycle; spec lives in `proposal.md`) |

## Lifecycle note (honest)

**e70 was never instantiated as a formal SDDK cycle in the ledger.** Same pattern as e67+e68+e69:
three work units authored with deterministic checkpoints, single commit `c0975660` carrying
all three modules together (WU2 and WU3 are types in the same file as WU1 — they would not
compile in isolation). Verification report (already on disk) is the durable record.

**Mandatory STOP after e70** (declared in the proposal). M9 (Fork / Trial / Promote) MUST NOT
be opened automatically — strategic architecture review required first.

This archive commit closes the on-disk artifact gap (proposal + verification-report were
already committed; archive-manifest was missing).

## Commits

| Commit | Kind | Summary |
|--------|------|---------|
| `c0975660` | `feat(e70)` | WU1+WU2+WU3 in a single commit (1543 insertions) |

## Delivered

### WU1 — WorkExecutor trait + InMemoryWorkExecutor (`crates/cognicode-core/src/application/local_ci/mod.rs`)

- `WorkExecutor` trait — anything can implement it; the first impl is `InMemoryWorkExecutor`
  for tests. AI / plugins PROPOSE; they do not mint authority.
- `LocalVerticalReport` — the structured output of the vertical.
- Empty-dependencies path yields empty report (deterministic).

### WU2 — `why_scheduled` + `why_decided` (separate explanations)

- `why_scheduled` explains selection (which `WorkId`s and why) — driven by e68's
  `AffectedWorkPlan`.
- `why_decided` explains authority (which `PolicyDecision` and why) — driven by e69's
  `PolicyGate`.
- Two SEPARATELY structured explanations; no overlap, no conflation.

### WU3 — End-to-end adversarial + invariants

- 15/15 tests in `application::local_ci::tests`.
- Adversarial: Unknown/incomplete NEVER becomes "safe":
  - All-Unaffected is never executed (the planner decided nothing to run → no execution).
  - Planner `Unknown` IS executed (work proceeds conservatively).
  - A `Missing` slot on a required rule still yields `InsufficientEvidence`.
  - Producer failure on a required slot blocks the gate.

## Architectural decisions (per proposal)

1. **Authority ≠ Execution**: anything can implement `WorkExecutor`, but the executor
   cannot mint authority. The gate is the gate.
2. **`why_scheduled` and `why_decided` are separately typed** (no shared envelope).
   They answer different questions; conflating them breaks the audit trail.
3. **Unknown is not Unaffected**: the planner's `Unknown` decision means "execute
   conservatively", not "skip". The gate then independently evaluates evidence.
4. **All-Unaffected → no execution**: if the planner says nothing to run, the vertical
   emits an empty report; it does NOT execute "just in case".

## Vertical pipeline

```text
source change
   ↓
FactDelta                                ← e68 WU1
   ↓
AffectedWorkPlan                         ← e68 WU2
   ↓
WorkExecutor (InMemory / future real)    ← e70 WU1
   ↓
EvidenceBundle                           ← e69 WU1
   ↓
PolicyGate → GateVerdict                 ← e69 WU3
   ↓
LocalVerticalReport {
   why_scheduled: Vec<SchedulingReason>,
   why_decided: Vec<PolicyReason>,
}
```

## Mandatory STOP (per authorized envelope)

After this cycle closes, **M9 (Fork / Trial / Promote) MUST NOT be opened automatically.**
A strategic architecture review is required before any M9 work begins.

## Non-goals (deferred to M9+)

- Remote CI orchestration (GitHub Actions, Jenkins, distributed workers).
- A scheduler with retries.
- A global policy registry.
- Trial-and-Promote (M9 follow-on, BLOCKED pending review).

## Acceptance

The verification report enumerates 15/15 tests in `application::local_ci::tests` + the
6-case adversarial matrix. All GREEN.

## Static gates (all clean)

- `cargo fmt --check -p cognicode-core` → clean.
- `cargo clippy -p cognicode-core --all-targets --features evidence-kernel -- -D warnings`
  → clean on touched paths.

## Related debt

- `docs/debts/DEBT-SDDK-002.md` — release route blocked; closure via direct push.
- `docs/debts/DEBT-SDDK-003.md` — why this cycle ran in degraded-but-governed mode.

## Related cycles

- **e67** (foundation): production grounding activation.
- **e68** (peer): semantic diff + affected work planner.
- **e69** (peer): EvidenceBundle + PolicyGate.
- **M9** (BLOCKED pending review): Fork / Trial / Promote.
