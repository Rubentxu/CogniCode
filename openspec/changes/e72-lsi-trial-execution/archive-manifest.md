# Archive Manifest — cycle e72 — ChangeProposal + TrialEvidence + TrialExecutor

> Cycle: A-lite (degraded-but-governed) | Milestone: M9 — Fork / Trial / Diff / Promote | Phase: archive | Date: 2026-09-17

## Cycle identity

| Item | Value |
|------|-------|
| Cycle | e72 |
| Milestone | M9 — second slice: change-proposal + trial assembly |
| Requirement | ADR-046 (PROPOSED) — software world fork + trial + diff + promote |
| Path | A-lite — degraded-but-governed (DEBT-SDDK-003) |
| Base HEAD | `50e2803d` (e71 WU3) |
| Spec delta | none |

## Lifecycle note (honest)

**e72 was never instantiated as a formal SDDK cycle in the ledger.** Same pattern as e67-e70+e74-e75+e76+e71:
three work units authored with deterministic checkpoints; each WU landed in a separate commit.
No `openspec/changes/e72-*/` directory existed at commit time — this commit creates the change
dir + archive-manifest so the on-disk record is complete.

Closure route: **D3-DEFER administrative closure** (per DEBT-SDDK-002 + DEBT-SDDK-003).

This is **NOT** equivalent to a successful normal orchestrator closure. No SDDK cycle row was
created in the ledger, no gate receipts were fabricated, no worker reviews were simulated.

## Commits

| Commit | WU | Summary |
|--------|----|---------|
| `ca86eeb1` | WU1 | `feat(e72)` — ChangeProposal — intent-only, creation != authority — 511 insertions |
| `1ae8588c` | WU2 | `feat(e72)` — TrialEvidence — lineage envelope over e68/e69/e70, no parallel verdict — 441 insertions, 32 deletions |
| `ac49caa4` | WU3 | `feat(e72)` — TrialExecutor trait + DefaultTrialExecutor — reuses e69 PolicyGate, no parallel verdict — 379 insertions, 26 deletions |

## Delivered

### WU1 — `ChangeProposal` (intent-only)

`crates/cognicode-core/src/application/change_proposal/{proposal,proposal_tests}.rs`

- A `ChangeProposal` describes **what** someone wants to change.
- It carries **no** authority to apply.
- **Umbrella invariant**: *Creation is not authority.* A `ChangeProposal` does not contain
  any field that grants apply power.
- The promotion path is reserved for e73's `PromotionPermit`.

### WU2 — `TrialEvidence` (lineage envelope)

`crates/cognicode-core/src/application/change_proposal/{trial,trial_tests}.rs`

- `TrialEvidence` wraps the e68/e69/e70 outputs under one lineage header.
- The lineage envelope says: this evidence came from these facts + these read-sets + this
  bundle + this gate verdict, in that order.
- **No parallel verdict model.** The e69 `PolicyGate` remains the gate; `TrialEvidence`
  only collects what the gate already evaluated.

### WU3 — `TrialExecutor` trait + `DefaultTrialExecutor`

`crates/cognicode-core/src/application/change_proposal/{executor,executor_tests}.rs`

- `TrialExecutor` trait — anything can implement it (in-memory, remote, etc.).
- `DefaultTrialExecutor` reuses e69's `PolicyGate`. No new verdict model.
- Per the commit message: *"no parallel verdict"*. The trial uses the existing gate.
- The first impl is in-memory for tests; production wiring is the M9 follow-on.

## Architectural decisions (from the code, not inferred)

1. **Creation is not authority.** Mirrored at the application layer: a `ChangeProposal`
   is intent-only. The promotion path is e73's `PromotionPermit`.
2. **No parallel verdict.** Trial evidence wraps existing e69 output; it does not
   re-evaluate. This keeps the gate the gate.
3. **Two-layer split** (proposal + trial) instead of one: matches the M8 separation
   between *what to run* (e68's planner) and *what came back* (e69's bundle). The
   trial envelope is the durable record that ties the two together.
4. **`TrialExecutor` is a trait, not a singleton.** AI / plugins propose; they do not
   mint authority.

## Out of scope (deferred to e73)

- Promotion evaluation (three-way dry-run).
- Promotion permit + apply.
- End-to-end adversarial pipeline gate.

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

## Related cycles

- **e68** (peer): `SemanticFactDelta` — the diff primitive whose result the trial envelope wraps.
- **e69** (peer): `EvidenceBundle` + `PolicyGate` — the gate that `TrialExecutor` reuses.
- **e71** (peer): `SoftwareWorld` foundation + fork + world-diff — produces the worlds that
  the trial runs in.
- **e73** (successor): promotion evaluation + permit + end-to-end gate.

## Related debt

- `docs/debts/DEBT-SDDK-002.md` — release route blocked; closure via direct push.
- `docs/debts/DEBT-SDDK-003.md` — why this cycle ran in degraded-but-governed mode.
