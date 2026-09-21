# Archive Manifest — e80a Automated Author Authority

> Cycle: e80a-lsi-automated-author-authority | Phase: archive | Date: 2026-09-17
> Closure pattern: **administrative archive** (git-level), consistent with
> e79/e79.1. The cycle was not instantiated as a formal SDDK cycle record; the
> work is characterized (+ executable evidence), implemented, tested, and
> archived here.

## Cycle summary

| Phase | Status | Artifact |
|-------|--------|----------|
| WU0 characterize | DONE | `characterization.md` + `authority_characterization_tests.rs` (commit `4bdb597a`) |
| WU1 approval target | DONE | `authorization.rs` (`PromotionApprovalTarget`) |
| WU2 verification seam | DONE | `ExternalApprovalRequest` / `ExternalApprovalVerifier` / `RejectAllExternalApprovals` |
| WU3 sealed approval | DONE | `VerifiedExternalApproval` (private fields + seal, no serde) |
| WU4 authorization policy | DONE | `PromotionAuthorizationPolicy` / `PromotionAuthorization` |
| WU5 minting bypass closed | DONE | `permit.rs` (`issue_promotion_permit` requires an authorization) |
| WU6 apply layering | DONE | `apply_with_permit` unchanged; authorship-agnostic |
| WU7 adversarial tests | DONE | `authority_tests.rs` (cases A-L) |
| WU8 audit/events | DONE (STOPPED sub-task, as directed) | documented adapter seam, no event architecture |
| archive | DONE | this document + `state.yaml` update |

## Architecture

```text
ChangeProposal (requested_by)
      │
      ▼
Trial
      │
      ▼
PromotionEvaluation ──► PromotionDryRun ──► CleanPromotionReady
      │
      ▼
PromotionAuthorizationPolicy::authorize(proposal, dry_run, external_approval)
      │
      ├── Human
      │      └── authorization
      │
      └── Plugin | LlmAgent
             │
             └── VERIFIED external human approval bound to the exact target
                         │
                         ▼
                 authorization
                         │
                         ▼
        issue_promotion_permit(id, authorization) ──► PromotionPermit
                         │
                         ▼
                 apply_with_permit(current, permit)
```

## Invariant established

```text
automated author
+ perfect trial
+ PolicyGate::Pass
+ CleanPromotionReady
+ forged "human" ActorRef
+ NO trusted external verification

= NO PromotionPermit
```

`ActorRef::Human` remains caller-constructible data (it even has
`ActorRef::human(id)`). It is an identity *claim*, not proof. Authority is the
sealed `VerifiedExternalApproval` / `PromotionAuthorization`, minted only by
their designated functions.

## WU6 layering note

`apply_with_permit` was deliberately left authorship-agnostic:

```text
authorship/policy
        ↓
PromotionAuthorization (sealed)
        ↓
PromotionPermit (minted only from an authorization)
        ↓
apply_with_permit (validates the permit + world drift only)
```

The permit already embodies the authority, so `apply` needs no `LlmAgent` /
`Plugin` / `Human` / verifier knowledge.

## Verification

| Check | Result |
|-------|--------|
| `cargo test -p cognicode-core --lib` | terminates; failure set == `scripts/known_failures.yaml` exactly (41 entries, no new regressions) |
| same, `--features evidence-kernel` | 2508 passed / 2 failed (the 2 are pre-existing `interproc_summary` entries = DEBT-SDDK-006) |
| e80a `promotion_authority` (WU7 A-L included) | 42 passed / 0 failed |
| e69 `policy_gate` | 14 passed / 0 failed |
| e64 behavior (lib) | 19 passed / 0 failed |
| e79 `application::ai` boundary | 25 passed / 0 failed |
| e77.1 `architecture_e77_1_wu3` canonical grounding | 10 passed / 0 failed |
| `behavior_authority_e2e` + `behavior_budget_e2e` | 12 passed / 0 failed |
| `intelligence_event_log_e2e` + `findings_canonical_grounding_e2e` | 14 passed / 0 failed |
| `cargo build --workspace` | GREEN |
| clippy on the touched surface | no new warnings |
| import audit (WU7-L) | no `application/ai` or `domain/ai` module reaches the permit-minting surface |

## Exit condition

```text
automated proposal creation       allowed
automated trial                   allowed under existing behavior policy
automated technical evaluation    allowed
automated self-approval           impossible
automated permit minting          impossible without verified external approval
human-authored existing path      preserved
apply without permit              impossible
world-drift fail-closed           preserved
```

Honest note on "impossible": the two impossible properties are enforced
structurally (private fields + private seal + a minting function that requires
the sealed authorization value), verified by the WU7 adversarial tests and the
source audits in `authority_tests.rs`. They are not enforced by a runtime check
that a caller could bypass.

## Explicit non-goals (honored)

No FixAgent, no `SourcePatch` generation, no automatic apply, no live LLM API,
no Backstage integration, no Control Plane, no cryptographic signatures, no
trusted identity registry implementation, no e78 Packs, no P1.4 refactor, no
evidence-grade thresholds, no "two independent sources" policy.

The `interproc_summary` known-failure defect was NOT fixed here; it is recorded
as DEBT-SDDK-006.

## Debts

| Debt | Status |
|------|--------|
| DEBT-SDDK-002 | RESOLVED (e79.1) |
| DEBT-SDDK-005 | RESOLVED (e79.1) |
| DEBT-SDDK-004 | PARTIAL: 3/4 repaired; P1.4 deferred with architectural trigger |
| DEBT-SDDK-006 | OPEN (new): `interproc_summary` feature-gating drift in two baseline entries |

P1.4 (`domain::behaviors::runtime` → `application::intelligence_log::CausalRecorder`)
is explicitly carried forward. It does not block e80a/e80b. It becomes a hard
prerequisite before the e77 architecture evaluation may participate in an
authority-bearing gate (tentatively after e82, before e83). It was NOT solved
opportunistically here.

The DEBT records live under the gitignored `docs/` tree and remain local-only
working documents.

## Commits

* **WU0 characterization:** `4bdb597a` — `test(e80a): characterize the
  automated-author authority gap (WU0)`.
* **Implementation:** `cfefd548` — `feat(e80a): enforce authorship-aware
  promotion authority`.
* **Archive:** this commit — this manifest + `state.yaml` umbrella update.

## Files

New:
* `application/promotion_authority/authorization.rs`
* `application/promotion_authority/authority_tests.rs`
* `application/promotion_authority/authority_test_support.rs`

Changed:
* `application/promotion_authority/permit.rs`
* `application/promotion_authority/evaluation.rs`
* `application/promotion_authority/mod.rs`
* `application/promotion_authority/permit_tests.rs`
* `application/promotion_authority/pipeline_tests.rs`
* `application/promotion_authority/authority_characterization_tests.rs`

## State transition

```text
e80a CLOSED
automated authors cannot self-promote without verified external approval

NEXT: e80b (Fix Agent -> ChangeProposal only)
```

## STOP

Architectural stop after e80a. `e80b` is NOT started by this cycle. The Fix
Agent remains unimplemented; e80a established only the authority boundary it
will have to cross.
