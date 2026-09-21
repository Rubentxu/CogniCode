# e80a Characterization — Automated Author Authority

> Cycle: e80a-lsi-automated-author-authority | Phase: WU0 (characterize) | Date: 2026-09-17
> Characterize before design. No production code changes in WU0.

## The gap (proven against current HEAD)

Today a machine-authored proposal can obtain a `PromotionPermit` with no
human involvement and no external approval:

```text
ChangeProposal { requested_by: RequestedBy::LlmAgent }
        +
TrialEvidence with gate.outcome == PolicyOutcome::Pass
        +
evaluate_promotion(..) -> PromotionStatus::CleanPromotionReady
        +
issue_promotion_permit(PromotionPermitId, PromotionDryRun)
        =
Ok(PromotionPermit)          <-- the authority gap e80a closes
```

This is not a hypothetical. It is pinned by an existing test:

* `crates/cognicode-core/src/application/promotion_authority/pipeline_tests.rs:301`
  `pipeline_automated_author_promotion_under_current_gate_contract`

That test sets `requested_by = RequestedBy::LlmAgent { agent_ref: "claude" }`,
runs the trial, evaluates, mints a permit, and asserts the apply succeeds. Its
own doc comment says it "pins the documented M9 assumption that the trial gate
does NOT check author class" and that it is "the breaking point for any future
ADR adding `AutomatedAuthorWithoutCoAuth`".

WU0 adds an explicit characterization test file
(`application/promotion_authority/authority_characterization_tests.rs`) that
makes the hole explicit for both `LlmAgent` and `Plugin`, and separately
records the Human path that must be preserved.

## Why the gap exists structurally

Authorship is *lost* before the authority decision is made:

```text
ChangeProposal.requested_by: RequestedBy        <- authorship lives here
        |
        v   (TrialInput carries the whole ChangeProposal)
TrialEvidence { trial_id, proposal_id, ... }    <- requested_by is NOT carried
        |
        v   (PromotionEvaluationInput.proposal is only a ChangeProposalId)
evaluate_promotion(input, trial) -> PromotionDryRun   <- no authorship in scope
        |
        v   (issue_promotion_permit takes only (id, dry_run))
issue_promotion_permit(id, dry_run) -> PromotionPermit  <- no authorship in scope
        |
        v
apply_with_permit(current, permit)
```

`PromotionEvaluationInput` (evaluation.rs:47) carries `proposal: ChangeProposalId`,
not the `ChangeProposal`. `TrialEvidence` (trial.rs:105) carries `proposal_id`
but drops `requested_by`. So even if one wanted to check authorship inside
`evaluate_promotion`, the information is not in scope there.

This is the structural confirmation of the semantic split below: the authority
decision needs the `ChangeProposal`, so it belongs in a *separate* step that
receives it.

## Semantic boundary (decision)

```text
Trial PASS        = technical evidence
Policy PASS       = policy accepts
Evaluation CLEAN  = the change is promotable (technical readiness / value)
Human approval    = authority
PromotionPermit   = effective capability
```

Therefore:

* `PromotionEvaluation` / `evaluate_promotion` stays **technical readiness**.
  UNCHANGED.
* `PromotionAuthorizationPolicy` is the **authority decision**. NEW.
* `PromotionBlockReason::AutomatedAuthorWithoutCoAuth` (evaluation.rs:93) is a
  **dead variant**: it is declared but never constructed anywhere in the
  workspace (verified by grep; only the declaration and doc references exist).
  It is NOT wired into `evaluate_promotion` just because it exists. It is
  superseded cleanly, with no second competing automated-author rule.

## Ownership / seam map

| Symbol | Location | Role today | e80a decision |
|--------|----------|-----------|---------------|
| `ChangeProposal` | `application/change_proposal/proposal.rs:136` | intent + author class | UNCHANGED; canonical source of authorship |
| `RequestedBy` | `.../proposal.rs:88` | Human / Plugin / LlmAgent (+ `is_automated()`) | UNCHANGED; drives the policy switch |
| `TrialEvidence` | `.../change_proposal/trial.rs:105` | trial verdict envelope | UNCHANGED; note it drops `requested_by` |
| `PromotionEvaluationInput` | `.../promotion_authority/evaluation.rs:47` | three-way input | UNCHANGED |
| `evaluate_promotion` | `.../evaluation.rs:149` | technical readiness | UNCHANGED (stays technical) |
| `PromotionStatus` / `PromotionDryRun` | `.../evaluation.rs:61,122` | readiness value | UNCHANGED |
| `PromotionBlockReason::AutomatedAuthorWithoutCoAuth` | `.../evaluation.rs:93` | dead variant | SUPERSEDED cleanly (deprecated, not rewired) |
| `PromotionPermit` | `.../permit.rs:82` | authority to apply | CHANGED: carries a sealed `PromotionAuthorization`; one private field blocks external literal construction |
| `issue_promotion_permit` | `.../permit.rs:113` | mints on clean dry-run alone | CHANGED: requires `PromotionAuthorization` |
| `apply_with_permit` | `.../permit.rs:179` | fail-closed apply | UNCHANGED semantics; learns nothing about authorship |
| `PromotionPermitId` | `.../permit.rs:55` | permit id | UNCHANGED |
| `ActorRef` / `ActorKind` | `domain/execution/actor.rs:65,30` | actor identity | REUSED as the claimed approver identity; explicitly NOT proof |
| `findings::admission::ApprovalVerifier` | `domain/findings/admission.rs:175` | detector approval port | PATTERN REUSE ONLY (target is detector-specific) |
| `VerifiedPromotion` | `domain/findings/admission.rs:215` | sealed detector promotion | PATTERN REUSE ONLY |
| `RejectAllApprovals` | `domain/findings/admission.rs:182` | fail-closed default | PATTERN REUSE (the default-verifier shape) |
| `PromotionAuthority::verify` | `domain/findings/admission.rs:249` | single minter of `VerifiedPromotion` | PATTERN REUSE (single-minter shape) |

### Pattern reuse, not type reuse

`findings::admission::{ApprovalVerifier, VerifiedPromotion, PromotionAuthority,
RejectAllApprovals}` already implement exactly the shape e80a needs: a
verifier port, a fail-closed default, a private seal, and a single minter.
But their target is detector admission (`DetectorId` + version + semantic
digest + admission source), not a `ChangeProposal` promotion across
`SoftwareWorld` lineage. e80a reuses the *architectural pattern* and does not
generalize the detector types.

Note also the anti-pattern to avoid: `PromotionRequest.approver` in that module
is a plain `String` field. e80a must NOT model approval as a caller-supplied
string/`ActorRef` field; approval is a sealed artifact produced by external
verification.

## Consumer map (blast radius)

* `issue_promotion_permit` has **zero production callers** today; it is called
  only from `permit_tests.rs` and `pipeline_tests.rs`. The signature change in
  WU5 is contained to tests.
* `PromotionPermit` is constructed only inside `permit.rs`.
* `PromotionPermit` is referenced (as a forbidden authority type) by the e79 AI
  boundary tests (`application/ai/boundary_tests.rs`); e80a extends that audit
  rather than breaking it.

## Invariant to establish

```text
automated author
+ perfect trial
+ PolicyGate::Pass
+ CleanPromotionReady
+ forged "human" ActorRef
+ NO trusted external verification

= NO PromotionPermit
```

`ActorRef::Human` is public, caller-constructible data (it even has
`ActorRef::human(id)`), so an `ActorRef` alone can never be proof of authority.
Authority must be a sealed artifact minted only after external verification.

## WU0 exit

* Gap proven for `LlmAgent` and `Plugin`; Human path recorded.
* Seam map complete; semantic boundary decided.
* Dead `AutomatedAuthorWithoutCoAuth` variant accounted for.
* No production code changed yet.
