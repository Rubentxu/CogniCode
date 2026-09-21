# Archive Manifest — cycle e73 — PromotionEvaluation + PromotionPermit + end-to-end pipeline gate

> Cycle: A-lite (degraded-but-governed) | Milestone: M9 — Fork / Trial / Diff / Promote | Phase: archive | Date: 2026-09-17

## Cycle identity

| Item | Value |
|------|-------|
| Cycle | e73 |
| Milestone | M9 — third slice: promotion authority (the last gate before apply) |
| Requirement | ADR-046 (PROPOSED) — software world fork + trial + diff + promote |
| Path | A-lite — degraded-but-governed (DEBT-SDDK-003) |
| Base HEAD | `ac49caa4` (e72 WU3) |
| Spec delta | none |

## Lifecycle note (honest)

**e73 was never instantiated as a formal SDDK cycle in the ledger.** Same pattern as e67-e70+e74-e75+e76+e71+e72:
three work units authored with deterministic checkpoints; each WU landed in a separate commit.
No `openspec/changes/e73-*/` directory existed at commit time — this commit creates the change
dir + archive-manifest so the on-disk record is complete.

Closure route: **D3-DEFER administrative closure** (per DEBT-SDDK-002 + DEBT-SDDK-003).

This is **NOT** equivalent to a successful normal orchestrator closure. No SDDK cycle row was
created in the ledger, no gate receipts were fabricated, no worker reviews were simulated.

## Commits

| Commit | WU | Summary |
|--------|----|---------|
| `1b6a004b` | WU1 | `feat(e73)` — PromotionEvaluation three-way — base/candidate/current + fail-closed promotion gate — 668 insertions |
| `66e982c5` | WU2 | `feat(e73)` — PromotionPermit + apply fail-closed — creation != authority — 538 insertions, 35 deletions |
| `d0c53648` | WU3 | `feat(e73)` — adversarial end-to-end pipeline gate — proposal→trial→evaluation→permit→apply wired — 397 insertions |

## Delivered

### WU1 — `PromotionEvaluation` (three-way)

`crates/cognicode-core/src/application/promotion_authority/{evaluation,evaluation_tests}.rs`

- Three-way dry-run:
  ```text
  ChangeProposal
       +
  base world A     (world when the proposal was created)
       +
  candidate world B (world after the trial ran)
       +
  current world C  (world right now)
         ↓
  three-way dry-run
         ↓
  PromotionDryRun
  ```
- **Fail-closed promotion gate**: if any of the three inputs is missing or has drifted since
  the trial, the evaluation refuses — it does not proceed to permit.
- "Three-way" means we don't only check A→B (the trial delta) — we ALSO check whether the
  current world (C) has drifted from A while the trial was running. Drift = refuse.

### WU2 — `PromotionPermit` + apply (fail-closed)

`crates/cognicode-core/src/application/promotion_authority/{permit,permit_tests}.rs`

- `PromotionPermit` is the only surface that can apply a `ChangeProposal` to the current world.
- **Creation is not authority** — even with a valid `PromotionDryRun`, no apply happens
  unless the explicit permit is granted.
- `apply` is fail-closed: if the permit is invalid, expired, or the world has drifted
  since the evaluation, the apply refuses and rolls back to a clean state.

### WU3 — Adversarial end-to-end pipeline gate

`crates/cognicode-core/src/application/promotion_authority/pipeline_tests.rs`

- Wires the full pipeline:
  ```text
  ChangeProposal (e72 WU1)
       ↓
  Trial (e72 WU2+WU3)
       ↓
  PromotionEvaluation (e73 WU1)
       ↓
  PromotionPermit (e73 WU2)
       ↓
  apply (fail-closed)
  ```
- Adversarial coverage:
  - missing base / candidate / current worlds → fail-closed evaluation;
  - world drift between evaluation and permit → fail-closed apply;
  - permit reused / replayed → fail-closed apply;
  - permit applied to wrong world → fail-closed apply;
  - missing trial evidence → fail-closed evaluation;
  - refuting evidence in the trial → fail-closed evaluation.

## Architectural decisions (from the code, not inferred)

1. **Three-way evaluation, not two-way.** The candidate world (B) is the result of the
   trial, but the current world (C) may have moved. A two-way check would miss C drift.
2. **Permit is the only apply path.** No field on `ChangeProposal` or `TrialEvidence`
   grants apply. The permit is the gate.
3. **Fail-closed at every seam.** Evaluation refuses on missing input; permit refuses
   on drift; apply refuses on stale permit. There is no implicit retry or fallback.
4. **Adversarial pipeline gate** as the WU3 deliverable — the M9 vertical is exercised
   end-to-end at the start of every CI run, not only at design time.

## Out of scope (deferred)

- A scheduler that issues `PromotionPermit` automatically (this would conflate authority
  with automation and is the M11 follow-on — `AutomatedAuthorPromotionPolicy` per the
  user's M10/M11 roadmap note).
- Reversible / compensating promotions (the ADR-046 rollback story); the apply is
  fail-closed but rollback is the next cycle's concern.

## M9 status (per the umbrella `state.yaml`)

```text
M9 implementation        CLOSED  (e71 + e72 + e73 code in main; this archive)
M9 technical verification GREEN  (all scoped test suites pass)
M9 strategic decision    PASSED  by explicit user directive 2026-09-17
ADR-046 status           remains PROPOSED until its own acceptance/evidence
                         requirements are satisfied
administrative closure   D3-DEFER for e71/e72/e73
```

The "M9 strategic decision PASSED" reflects a **roadmap/governance decision** by the user.
It MUST NOT be interpreted as satisfying ADR-046's independent acceptance/evidence
requirements. ADR-046 remains PROPOSED until those requirements are separately demonstrated.

## M11 dependency (declared by the user)

Before the M11 Fix Agent can self-promote, the codebase needs an
`AutomatedAuthorPromotionPolicy` that the e73 pipeline can call into. That policy is
NOT in scope for e73 (the user explicitly excluded it from e77/e78 work). It is the
explicit blocker between M9 closure and M11 start.

## Related cycles

- **e71** (peer): `SoftwareWorld` foundation + fork + world-diff.
- **e72** (peer): `ChangeProposal` + `TrialEvidence` + `TrialExecutor`.
- **e77** (next planned evolutive per user directive): executable architecture constraints
  — reuses the M9 vertical as the substrate for architecture drift evaluation.
- **M11** (blocked until `AutomatedAuthorPromotionPolicy` lands): AI investigation / critic /
  fix agent.

## Related debt

- `docs/debts/DEBT-SDDK-002.md` — release route blocked; closure via direct push.
- `docs/debts/DEBT-SDDK-003.md` — why this cycle ran in degraded-but-governed mode.
