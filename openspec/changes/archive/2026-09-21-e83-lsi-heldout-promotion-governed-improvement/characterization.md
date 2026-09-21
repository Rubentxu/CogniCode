# e83 Characterization — Held-out Promotion + Governed Improvement

> Cycle: e83-lsi-heldout-promotion-governed-improvement | Phase: WU0 | Date: 2026-09-17

## WU0-A — baseline transient preflight

```text
5 consecutive `scripts/check_known_failures.py` runs   → all exact (41 entries)
1 full `cargo test -p cognicode-core --lib`            → exact maintained failure set
```

The e82.1 DRIFT observation is therefore **NOT REPRODUCED**. It remains an open
observation only, not a blocker. `known_failures.yaml` was not modified.

## WU0-B — promotion lineage holes (characterized before fixing)

Current `TrialEvidence` carries `world_id`, `base_snapshot` and
`candidate_snapshot`, but `evaluate_promotion` only validated `Pass` plus
`proposal_id`. Observed pre-fix behaviour of the lineage cases:

| Case | Pre-fix result |
|------|----------------|
| A. `proposal.base_world != base.id` | CleanPromotionReady (unchecked) |
| B. `trial.world_id != candidate.id` | CleanPromotionReady (unchecked) |
| C. `trial.base_snapshot != base.base_snapshot` | CleanPromotionReady (unchecked) |
| D. `candidate.parent_world != base.id` | CleanPromotionReady (unchecked) |
| E. `candidate.base_snapshot != base.base_snapshot` | CleanPromotionReady (unchecked) |

So a passing trial could be replayed against a different candidate world, and a
proposal could be evaluated against a base it never targeted. e83 closes this.

## WU1 — lineage hardening

Added `evaluate_promotion_lineage(GovernedPromotionInput { proposal: &ChangeProposal,
base, candidate, current }, trial)` implementing all six rules in order:

```text
proposal.base_world      == base.id
candidate.parent_world   == Some(base.id)
candidate.base_snapshot  == base.base_snapshot
trial.proposal_id        == proposal.id
trial.world_id           == candidate.id          (NEW)
trial.base_snapshot      == base.base_snapshot    (NEW)
```

with new typed block reasons `ProposalBaseWorldMismatch`,
`CandidateNotDerivedFromBase`, `CandidateBaseSnapshotMismatch`,
`TrialWorldMismatch`, `TrialBaseSnapshotMismatch`.

`TrialEvidence.candidate_snapshot` remains audit data: the current
`SoftwareWorld` model has no exact invariant to validate it against, and e83 does
not invent one.

**Scope note (WU10).** `evaluate_promotion` (id-based) is untouched and remains
the *technical* evaluator for generic M9 promotion. The authority-bearing entry
point is `evaluate_promotion_lineage`, and every governed promotion goes through
it. The generic path stays generic on purpose; the guarantee is exact and scoped:
any promotion through `governed_improvement` validates full lineage.

## Design

```text
application/governed_improvement/
  binding.rs   GovernedCandidateBinding + CandidateFreeze (sealed)
  policy.rs    HeldOutPolicySpec + HeldOutPromotionGate + decision + HeldOutGatePass (sealed)
  permit.rs    GovernedImprovementPermit (sealed) + apply_governed_improvement + Receipt
```

Three distinct "permits", kept separate:

```text
HeldOutGatePass         "CONFIRM does not block"
PromotionAuthorization  "the authority permits"        (e80a)
PromotionPermit         "effective apply capability"   (e73)
GovernedImprovementPermit = pass + permit for the SAME attempt
```

Deliberate non-goals recorded in the module docs:

* no synthetic canonical `Evidence` for the held-out result (WU8);
* no binary/compiler attestation (WU17);
* no source mutation claim (WU16);
* no event bus (WU19) — typed outcomes + `GovernedImprovementReceipt` are the
  audit artifacts, and the Intelligence Event Log seam stays documented.

## WU23 — stability closure gate

```text
5 consecutive checker runs          exact (41 entries)
3 full lib failure-set captures     md5-identical (f27744948a9c475a5d0513968a5cda1c)
```

The transient did not reproduce. e83 is therefore archivable, with the e82.1
observation still recorded as unexplained.
