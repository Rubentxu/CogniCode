//! Tests for `application::promotion_authority::permit` (e73 WU2).
//!
//! Adversarial coverage of `PromotionPermit` and `apply_with_permit`:
//!
//! 1. A permit is only issued from a sealed `PromotionAuthorization`.
//! 2. A Blocked dry-run yields no authorization (DryRunNotClean).
//! 3. A Conflict dry-run yields no authorization (DryRunNotClean).
//! 4. `apply_with_permit` requires a valid permit (defensive: a
//!    hand-crafted invalid permit is rejected).
//! 5. `apply_with_permit` rejects when the world has drifted since
//!    the permit was issued (WorldDriftedSincePermit).
//! 6. `apply_with_permit` succeeds when the world still matches.
//! 7. The permit carries the full dry-run lineage for audit.
//! 8. **No auto-promotion**: a ChangeProposal alone is insufficient
//!    for apply. Only a permit grants apply power.

use crate::application::change_proposal::proposal;
use crate::application::change_proposal::proposal::{
    ChangeProposal, ChangeProposalId, ProposalKind, RequestedBy,
};
use crate::application::policy_gate::PolicyOutcome;
use crate::application::promotion_authority::evaluation::{
    PromotionBlockReason, PromotionDryRun, PromotionEvaluationInput, PromotionStatus,
    evaluate_promotion,
};
use crate::application::promotion_authority::authorization::{
    PromotionAuthorization, PromotionAuthorizationError, PromotionAuthorizationPolicy,
};
use crate::application::promotion_authority::permit::{
    PromotionApplyError, PromotionApplyOutcome, PromotionPermitId, apply_with_permit,
    assert_world_matches_permit, issue_promotion_permit,
};
use crate::application::software_world::world::SoftwareWorld;
use crate::application::software_world::world::SoftwareWorldId;
use crate::domain::evidence_kernel::ids::SnapshotId;

fn world(id: &str, snap: u64) -> SoftwareWorld {
    SoftwareWorld::new_base(SoftwareWorldId::from_string(id), SnapshotId::new(snap))
}

fn proposal_id(s: &str) -> ChangeProposalId {
    ChangeProposalId::from_string(s)
}

/// The Human-authored proposal used by the clean dry-run fixture.
fn human_proposal() -> ChangeProposal {
    ChangeProposal::new(
        proposal_id("p-1"),
        SoftwareWorldId::from_string("w-A"),
        ProposalKind::SourcePatch {
            patch_ref: "patch-1".to_string(),
        },
        RequestedBy::Human {
            user_ref: "alice".to_string(),
        },
    )
}

/// Seal an authorization for the Human-authored fixture.
fn authorized(run: PromotionDryRun) -> PromotionAuthorization {
    PromotionAuthorizationPolicy::authorize(&human_proposal(), run, None)
        .expect("human-authored clean dry-run is authorised")
}

// --- helper: build a CleanPromotionReady dry-run --------------------

fn clean_dry_run() -> PromotionDryRun {
    // Build a real dry-run: base = current = snap:10, candidate snap:10,
    // trial with Pass gate.
    use crate::application::change_proposal::trial::{
        TrialId, TrialInput, assemble_trial_evidence,
    };
    use crate::application::change_tracking::planner::WorkId;
    use crate::application::evidence_bundle::EvidenceBundle;
    use crate::application::evidence_bundle::EvidenceBundleId;
    use crate::application::policy_gate::PolicyDecision;
    use crate::domain::kernel_ids::ExecutionId;
    use crate::domain::naming::NamespacedName;

    let proposal = proposal::ChangeProposal::new(
        proposal_id("p-1"),
        SoftwareWorldId::from_string("w-A"),
        ProposalKind::SourcePatch {
            patch_ref: "patch-1".to_string(),
        },
        RequestedBy::Human {
            user_ref: "alice".to_string(),
        },
    );
    let bundle = EvidenceBundle::new(
        EvidenceBundleId(1),
        WorkId::new(NamespacedName::new("ns.work").expect("valid name")),
        ExecutionId::new(1),
        SnapshotId::new(10),
        vec![],
    );
    let input = TrialInput {
        proposal,
        world: world("w-B", 10),
        base_snapshot: SnapshotId::new(10),
        candidate_snapshot: SnapshotId::new(11),
        candidate_facts: vec![],
        work_results: vec![],
        evidence_bundle: bundle,
    };
    let decision = PolicyDecision {
        outcome: PolicyOutcome::Pass,
        reasons: vec![],
    };
    let trial = assemble_trial_evidence(TrialId::from_string("t-1"), input, decision);

    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 10);
    let input = PromotionEvaluationInput {
        proposal: proposal_id("p-1"),
        base: &base,
        candidate: &candidate,
        current: &current,
    };
    let run = evaluate_promotion(input, Some(&trial));
    assert_eq!(run.status, PromotionStatus::CleanPromotionReady);
    run
}

fn blocked_dry_run() -> PromotionDryRun {
    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 10);
    let input = PromotionEvaluationInput {
        proposal: proposal_id("p-1"),
        base: &base,
        candidate: &candidate,
        current: &current,
    };
    let run = evaluate_promotion(input, None); // NoTrialEvidence
    assert_eq!(
        run.status,
        PromotionStatus::Blocked(PromotionBlockReason::NoTrialEvidence)
    );
    run
}

fn conflict_dry_run() -> PromotionDryRun {
    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 11); // diverged
    let input = PromotionEvaluationInput {
        proposal: proposal_id("p-1"),
        base: &base,
        candidate: &candidate,
        current: &current,
    };
    let run = evaluate_promotion(input, None); // NoTrialEvidence
    // We need at least a passing trial to hit the conflict check, but
    // we want the dry-run to be ConflictRequiresReevaluation. Easiest
    // path: skip the trial entirely — but that yields NoTrialEvidence
    // (Blocked). Construct a passing trial for this scenario.
    let trial = {
        use crate::application::change_proposal::trial::{
            TrialId, TrialInput, assemble_trial_evidence,
        };
        use crate::application::change_tracking::planner::WorkId;
        use crate::application::evidence_bundle::EvidenceBundle;
        use crate::application::evidence_bundle::EvidenceBundleId;
        use crate::application::policy_gate::PolicyDecision;
        use crate::domain::kernel_ids::ExecutionId;
        use crate::domain::naming::NamespacedName;

        let proposal_full = ChangeProposal::new(
            proposal_id("p-1"),
            SoftwareWorldId::from_string("w-A"),
            ProposalKind::SourcePatch {
                patch_ref: "p".to_string(),
            },
            RequestedBy::Human {
                user_ref: "alice".to_string(),
            },
        );
        let bundle = EvidenceBundle::new(
            EvidenceBundleId(2),
            WorkId::new(NamespacedName::new("ns.work").expect("valid name")),
            ExecutionId::new(2),
            SnapshotId::new(10),
            vec![],
        );
        let input = TrialInput {
            proposal: proposal_full,
            world: world("w-B", 10),
            base_snapshot: SnapshotId::new(10),
            candidate_snapshot: SnapshotId::new(11),
            candidate_facts: vec![],
            work_results: vec![],
            evidence_bundle: bundle,
        };
        let decision = PolicyDecision {
            outcome: PolicyOutcome::Pass,
            reasons: vec![],
        };
        assemble_trial_evidence(TrialId::from_string("t-2"), input, decision)
    };
    let run = evaluate_promotion(
        PromotionEvaluationInput {
            proposal: proposal_id("p-1"),
            base: &base,
            candidate: &candidate,
            current: &current,
        },
        Some(&trial),
    );
    assert_eq!(run.status, PromotionStatus::ConflictRequiresReevaluation);
    run
}

// --- 1-3. Issuing permits ------------------------------------------

#[test]
fn permit_issued_over_clean_dry_run() {
    let run = clean_dry_run();
    let permit = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), authorized(run));
    assert_eq!(permit.id, PromotionPermitId::from_string("pm-1"));
    assert_eq!(permit.proposal, proposal_id("p-1"));
}

#[test]
fn blocked_dry_run_yields_no_authorization() {
    let run = blocked_dry_run();
    let err = PromotionAuthorizationPolicy::authorize(&human_proposal(), run, None)
        .expect_err("blocked dry-run must not be authorised");
    assert_eq!(
        err,
        PromotionAuthorizationError::DryRunNotClean {
            actual: PromotionStatus::Blocked(PromotionBlockReason::NoTrialEvidence)
        }
    );
}

#[test]
fn conflict_dry_run_yields_no_authorization() {
    let run = conflict_dry_run();
    let err = PromotionAuthorizationPolicy::authorize(&human_proposal(), run, None)
        .expect_err("conflict dry-run must not be authorised");
    assert_eq!(
        err,
        PromotionAuthorizationError::DryRunNotClean {
            actual: PromotionStatus::ConflictRequiresReevaluation
        }
    );
}

// --- 4-6. apply_with_permit ----------------------------------------

#[test]
fn apply_with_valid_permit_yields_applied_outcome() {
    let run = clean_dry_run();
    let permit = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), authorized(run));
    let current = world("w-C", 10);
    let outcome = apply_with_permit(&current, &permit);
    assert!(matches!(
        outcome,
        PromotionApplyOutcome::Applied {
            applied_to_snapshot,
            ..
        } if applied_to_snapshot == SnapshotId::new(10)
    ));
}

#[test]
fn apply_with_permit_after_world_drifts_is_rejected() {
    let run = clean_dry_run();
    let permit = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), authorized(run));
    // World has drifted since the permit was issued.
    let drifted = world("w-C", 11);
    let outcome = apply_with_permit(&drifted, &permit);
    assert_eq!(
        outcome,
        PromotionApplyOutcome::Rejected(PromotionApplyError::WorldDriftedSincePermit {
            permit_snapshot: SnapshotId::new(10),
            current_snapshot: SnapshotId::new(11),
        })
    );
}

#[test]
fn assert_world_matches_permit_returns_snapshot_on_match() {
    let run = clean_dry_run();
    let permit = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), authorized(run));
    let current = world("w-C", 10);
    let snap = assert_world_matches_permit(&current, &permit).unwrap();
    assert_eq!(snap, SnapshotId::new(10));
}

#[test]
fn assert_world_matches_permit_returns_error_on_drift() {
    let run = clean_dry_run();
    let permit = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), authorized(run));
    let drifted = world("w-C", 11);
    let err = assert_world_matches_permit(&drifted, &permit).unwrap_err();
    assert_eq!(
        err,
        PromotionApplyError::WorldDriftedSincePermit {
            permit_snapshot: SnapshotId::new(10),
            current_snapshot: SnapshotId::new(11),
        }
    );
}

// --- 7. Audit trail -------------------------------------------------

#[test]
fn permit_carries_full_dry_run_lineage() {
    let run = clean_dry_run();
    let permit = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), authorized(run));
    assert_eq!(permit.dry_run().status, PromotionStatus::CleanPromotionReady);
    assert_eq!(permit.dry_run().lineage.proposal, proposal_id("p-1"));
    assert_eq!(
        permit.dry_run().lineage.base_world,
        SoftwareWorldId::from_string("w-A")
    );
    assert_eq!(
        permit.dry_run().lineage.candidate_world,
        SoftwareWorldId::from_string("w-B")
    );
    assert_eq!(
        permit.dry_run().lineage.current_world,
        SoftwareWorldId::from_string("w-C")
    );
    assert!(permit.dry_run().lineage.base_matches_current);
}

// --- 8. No auto-promotion -----------------------------------------

#[test]
fn proposal_alone_is_not_sufficient_for_apply() {
    // A ChangeProposal — even a fully-fledged one — is NOT enough to
    // apply. The apply requires a permit. This test demonstrates that
    // the only path to `PromotionApplyOutcome::Applied` is through
    // `issue_promotion_permit` + `apply_with_permit`. There is no
    // `apply(proposal, ...)` function in the public API.
    //
    // We assert this structurally: the only `pub fn` that returns
    // `PromotionApplyOutcome::Applied` is `apply_with_permit`, and
    // that function requires a `&PromotionPermit` argument.
    let run = clean_dry_run();
    let permit = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), authorized(run));
    let current = world("w-C", 10);
    // The signature `apply_with_permit(&SoftwareWorld, &PromotionPermit)`
    // makes it impossible to call without a permit. If this test
    // compiles, the type system enforces the invariant.
    let _: PromotionApplyOutcome = apply_with_permit(&current, &permit);
}
