//! Tests for `application::promotion_authority::evaluation` (e73 WU1).
//!
//! Adversarial coverage of the three-way promotion evaluation rules:
//!
//! 1. No trial evidence → `Blocked(NoTrialEvidence)`.
//! 2. Trial with `Block` gate → `Blocked(TrialGateNotPassing)`.
//! 3. Trial with `InsufficientEvidence` gate →
//!    `Blocked(TrialGateNotPassing)`.
//! 4. Trial with `Warn` gate → `Blocked(TrialGateNotPassing)` (only
//!    Pass is acceptable).
//! 5. Trial for a different proposal → `Blocked(TrialProposalMismatch)`.
//! 6. `current` diverged from `base` (different `base_snapshot`) →
//!    `ConflictRequiresReevaluation`.
//! 7. `base == current`, trial passed → `CleanPromotionReady`.
//! 8. Lineage metadata is carried on every outcome.
//! 9. PromotionDryRun does not contain a permit/apply authority
//!    field — it is descriptive only.

use crate::application::change_proposal::proposal::{
    ChangeProposal, ChangeProposalId, ProposalKind, RequestedBy,
};
use crate::application::change_proposal::trial::{
    TrialEvidence, TrialId, TrialInput, assemble_trial_evidence,
};
use crate::application::change_tracking::planner::WorkId;
use crate::application::evidence_bundle::EvidenceBundle;
use crate::application::evidence_bundle::EvidenceBundleId;
use crate::application::policy_gate::PolicyDecision;
use crate::application::policy_gate::PolicyOutcome;
use crate::application::promotion_authority::evaluation::{
    PromotionBlockReason, PromotionDryRun, PromotionEvaluationInput, PromotionStatus,
    evaluate_promotion,
};
use crate::application::software_world::world::SoftwareWorld;
use crate::application::software_world::world::SoftwareWorldId;
use crate::application::software_world::world::WorldSourceState;
use crate::domain::evidence_kernel::ids::SnapshotId;
use crate::domain::kernel_ids::ExecutionId;
use crate::domain::naming::NamespacedName;

// --- helpers ---------------------------------------------------------

fn world(id: &str, snap: u64) -> SoftwareWorld {
    SoftwareWorld::new_base(SoftwareWorldId::from_string(id), SnapshotId::new(snap))
}

fn proposal_id(s: &str) -> ChangeProposalId {
    ChangeProposalId::from_string(s)
}

fn empty_bundle() -> EvidenceBundle {
    EvidenceBundle::new(
        EvidenceBundleId(1),
        WorkId::new(NamespacedName::new("ns.work").expect("valid name")),
        ExecutionId::new(1),
        SnapshotId::new(10),
        vec![],
    )
}

fn make_trial_with_outcome(proposal: ChangeProposalId, outcome: PolicyOutcome) -> TrialEvidence {
    let proposal_full = ChangeProposal::new(
        proposal.clone(),
        SoftwareWorldId::from_string("w-base"),
        ProposalKind::SourcePatch {
            patch_ref: "p".to_string(),
        },
        RequestedBy::Human {
            user_ref: "alice".to_string(),
        },
    );
    let input = TrialInput {
        proposal: proposal_full,
        world: world("w-cand", 10),
        base_snapshot: SnapshotId::new(10),
        candidate_snapshot: SnapshotId::new(11),
        candidate_facts: vec![],
        work_results: vec![],
        evidence_bundle: empty_bundle(),
    };
    let decision = PolicyDecision {
        outcome,
        reasons: vec![],
    };
    assemble_trial_evidence(TrialId::from_string("t-x"), input, decision)
}

// --- 1. No trial evidence -------------------------------------------

#[test]
fn no_trial_evidence_blocks_promotion() {
    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 10);
    let input = PromotionEvaluationInput {
        proposal: proposal_id("p-1"),
        base: &base,
        candidate: &candidate,
        current: &current,
    };

    let run = evaluate_promotion(input, None);
    assert_eq!(
        run.status,
        PromotionStatus::Blocked(PromotionBlockReason::NoTrialEvidence)
    );
}

// --- 2-4. Trial gate outcomes ---------------------------------------

#[test]
fn trial_with_block_outcome_blocks_promotion() {
    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 10);
    let trial = make_trial_with_outcome(proposal_id("p-1"), PolicyOutcome::Block);
    let input = PromotionEvaluationInput {
        proposal: proposal_id("p-1"),
        base: &base,
        candidate: &candidate,
        current: &current,
    };

    let run = evaluate_promotion(input, Some(&trial));
    assert_eq!(
        run.status,
        PromotionStatus::Blocked(PromotionBlockReason::TrialGateNotPassing {
            actual: PolicyOutcome::Block
        })
    );
}

#[test]
fn trial_with_insufficient_outcome_blocks_promotion() {
    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 10);
    let trial = make_trial_with_outcome(proposal_id("p-1"), PolicyOutcome::InsufficientEvidence);
    let input = PromotionEvaluationInput {
        proposal: proposal_id("p-1"),
        base: &base,
        candidate: &candidate,
        current: &current,
    };

    let run = evaluate_promotion(input, Some(&trial));
    assert_eq!(
        run.status,
        PromotionStatus::Blocked(PromotionBlockReason::TrialGateNotPassing {
            actual: PolicyOutcome::InsufficientEvidence
        })
    );
}

#[test]
fn trial_with_warn_outcome_blocks_promotion() {
    // Only Pass lets promotion proceed. Warn means "every required
    // rule satisfied but at least one optional slot has
    // failed/missing/unknown" — that is *not* a clean promotion.
    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 10);
    let trial = make_trial_with_outcome(proposal_id("p-1"), PolicyOutcome::Warn);
    let input = PromotionEvaluationInput {
        proposal: proposal_id("p-1"),
        base: &base,
        candidate: &candidate,
        current: &current,
    };

    let run = evaluate_promotion(input, Some(&trial));
    assert_eq!(
        run.status,
        PromotionStatus::Blocked(PromotionBlockReason::TrialGateNotPassing {
            actual: PolicyOutcome::Warn
        })
    );
}

// --- 5. Trial proposal mismatch --------------------------------------

#[test]
fn trial_for_different_proposal_blocks_promotion() {
    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 10);
    let trial = make_trial_with_outcome(proposal_id("p-OTHER"), PolicyOutcome::Pass);
    let input = PromotionEvaluationInput {
        proposal: proposal_id("p-1"),
        base: &base,
        candidate: &candidate,
        current: &current,
    };

    let run = evaluate_promotion(input, Some(&trial));
    assert_eq!(
        run.status,
        PromotionStatus::Blocked(PromotionBlockReason::TrialProposalMismatch {
            expected: proposal_id("p-1"),
            actual: proposal_id("p-OTHER"),
        })
    );
}

// --- 6. C diverged from A --------------------------------------------

#[test]
fn current_diverged_from_base_requires_reevaluation() {
    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 11); // base_snapshot = 11, base = 10 → diverged
    let trial = make_trial_with_outcome(proposal_id("p-1"), PolicyOutcome::Pass);
    let input = PromotionEvaluationInput {
        proposal: proposal_id("p-1"),
        base: &base,
        candidate: &candidate,
        current: &current,
    };

    let run = evaluate_promotion(input, Some(&trial));
    assert_eq!(run.status, PromotionStatus::ConflictRequiresReevaluation);
    assert!(!run.lineage.base_matches_current);
}

// --- 7. Clean promotion ----------------------------------------------

#[test]
fn base_matches_current_with_passing_trial_yields_clean_promotion() {
    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 10); // same base_snapshot as base
    let trial = make_trial_with_outcome(proposal_id("p-1"), PolicyOutcome::Pass);
    let input = PromotionEvaluationInput {
        proposal: proposal_id("p-1"),
        base: &base,
        candidate: &candidate,
        current: &current,
    };

    let run = evaluate_promotion(input, Some(&trial));
    assert_eq!(run.status, PromotionStatus::CleanPromotionReady);
    assert!(run.lineage.base_matches_current);
}

// --- 8. Lineage metadata --------------------------------------------

#[test]
fn lineage_is_carried_on_every_outcome() {
    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 10);
    let input = PromotionEvaluationInput {
        proposal: proposal_id("p-1"),
        base: &base,
        candidate: &candidate,
        current: &current,
    };

    // Blocked case (no trial).
    let run = evaluate_promotion(input.clone(), None);
    assert_eq!(run.lineage.proposal, proposal_id("p-1"));
    assert_eq!(run.lineage.base_world, SoftwareWorldId::from_string("w-A"));
    assert_eq!(
        run.lineage.candidate_world,
        SoftwareWorldId::from_string("w-B")
    );
    assert_eq!(
        run.lineage.current_world,
        SoftwareWorldId::from_string("w-C")
    );
    assert_eq!(run.lineage.base_snapshot, SnapshotId::new(10));
    assert_eq!(run.lineage.current_snapshot, SnapshotId::new(10));
}

// --- 9. PromotionDryRun is descriptive, not authoritative -----------

#[test]
fn promotion_dry_run_carries_no_authority_field() {
    // The PromotionDryRun has exactly two fields: status and lineage.
    // Any future field that smells like authority (a permit, a
    // capability, an apply token) would break this test.
    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 10);
    let input = PromotionEvaluationInput {
        proposal: proposal_id("p-1"),
        base: &base,
        candidate: &candidate,
        current: &current,
    };
    let run = evaluate_promotion(input, None);

    // Destructuring asserts the field count is exactly 2.
    let PromotionDryRun {
        status: _,
        lineage: _,
    } = &run;
    // We assert this by the destructuring above; if a permit field
    // were added, the pattern would fail to compile.
    let _ = run; // keep the binding alive.
}

// --- Determinism ----------------------------------------------------

#[test]
fn evaluate_promotion_is_deterministic() {
    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 10);
    let trial = make_trial_with_outcome(proposal_id("p-1"), PolicyOutcome::Pass);
    let input = PromotionEvaluationInput {
        proposal: proposal_id("p-1"),
        base: &base,
        candidate: &candidate,
        current: &current,
    };

    let run1 = evaluate_promotion(input.clone(), Some(&trial));
    let run2 = evaluate_promotion(input, Some(&trial));
    assert_eq!(run1, run2);
}

// --- adversarial: a forked world has the same base_snapshot ---------

#[test]
fn forked_world_inherits_base_snapshot_and_can_be_current() {
    // When the current world is a fork of the base world, it inherits
    // the base_snapshot. This is the "no conflict" path.
    use crate::application::software_world::fork::SourceMutation;
    use crate::application::software_world::fork::fork;
    use crate::application::software_world::world::ContentHash;
    use std::path::PathBuf;

    let base = world("w-A", 10);
    let forked_current = fork(
        &base,
        SoftwareWorldId::from_string("w-A-forked"),
        SourceMutation::new_source(PathBuf::from("/fork"), ContentHash([0u8; 32])),
    )
    .new_world;
    // The fork inherits the base_snapshot.
    assert_eq!(forked_current.base_snapshot, base.base_snapshot);
    assert!(matches!(
        forked_current.source_state,
        WorldSourceState::Forked { .. }
    ));

    let candidate = world("w-B", 10);
    let trial = make_trial_with_outcome(proposal_id("p-1"), PolicyOutcome::Pass);
    let input = PromotionEvaluationInput {
        proposal: proposal_id("p-1"),
        base: &base,
        candidate: &candidate,
        current: &forked_current,
    };

    let run = evaluate_promotion(input, Some(&trial));
    assert_eq!(run.status, PromotionStatus::CleanPromotionReady);
}
