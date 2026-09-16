//! Adversarial end-to-end pipeline tests for M9 (e73 WU3).
//!
//! These tests exercise the full pipeline
//! `proposal → trial → evaluation → permit → apply` using the e72
//! `DefaultTrialExecutor` together with the e73 evaluation and permit
//! stages. They are the M9 adversarial gate that wires e71+e72+e73
//! into a single fail-closed pipeline.
//!
//! ## Scope
//!
//! - `pipeline_trial_with_missing_required_evidence_blocks_permit`:
//   bundle has no entry for the required slot → `InsufficientEvidence`
//!   → `TrialGateNotPassing` → permit refused.
//! - `pipeline_trial_with_passing_evidence_yields_clean_promotion_and_permit`:
//!   bundle has the required slot at the required grade → `Pass` →
//!   `CleanPromotionReady` → permit → `Applied` + lineage audit.
//! - `pipeline_trial_with_insufficient_grade_blocks_permit`:
//!   bundle has the slot but at a grade below `min_grade` →
//!   `InsufficientEvidence` → permit refused (no auto-promote).
//! - `pipeline_automated_author_promotion_under_current_gate_contract`:
//!   pins the documented M9 assumption that the trial gate does NOT
//!   check author class. Automated authors can still promote under
//!   the current contract; this test is the breaking point for any
//!   future ADR adding `AutomatedAuthorWithoutCoAuth`.
//! - `pipeline_world_drift_after_permit_rejects_apply_with_audit_marker`:
//!   world drifts after permit issued → `WorldDriftedSincePermit` at
//!   apply → marker preserved as audit evidence.

use crate::application::change_proposal::executor::{DefaultTrialExecutor, TrialExecutor};
use crate::application::change_proposal::proposal::{
    ChangeProposal, ChangeProposalId, ProposalKind, RequestedBy,
};
use crate::application::change_proposal::trial::{TrialId, TrialInput, assemble_trial_evidence};
use crate::application::change_tracking::planner::WorkId;
use crate::application::evidence_bundle::{BundleEntry, EvidenceBundle, EvidenceBundleId};
use crate::application::evidence_bundle::{ProducerSlot, ProducerSource};
use crate::application::policy_gate::{GateRule, PolicyOutcome, PolicySpec, ReasonVerdict};
use crate::application::promotion_authority::evaluation::{
    PromotionBlockReason, PromotionEvaluationInput, PromotionStatus, evaluate_promotion,
};
use crate::application::promotion_authority::permit::{
    PromotionApplyError, PromotionApplyOutcome, PromotionPermitError, PromotionPermitId,
    apply_with_permit, assert_world_matches_permit, issue_promotion_permit,
};
use crate::application::software_world::world::{SoftwareWorld, SoftwareWorldId};
use crate::domain::evidence_kernel::ids::{EvidenceId, FactId, SnapshotId};
use crate::domain::findings::ports::{EvidenceDescriptor, FactDescriptor, FactSlot};
use crate::domain::kernel_ids::{EvidenceGrade, ExecutionId};
use crate::domain::naming::NamespacedName;

// --- helpers ---------------------------------------------------------

fn world(id: &str, snap: u64) -> SoftwareWorld {
    SoftwareWorld::new_base(SoftwareWorldId::from_string(id), SnapshotId::new(snap))
}

fn proposal_id(s: &str) -> ChangeProposalId {
    ChangeProposalId::from_string(s)
}

fn slot(source: ProducerSource, id: &str) -> ProducerSlot {
    ProducerSlot {
        source,
        slot_id: id.to_string(),
    }
}

fn ev_descriptor(grade: EvidenceGrade) -> EvidenceDescriptor {
    EvidenceDescriptor {
        id: EvidenceId(1),
        grade,
        fact: FactSlot::Resolved(FactDescriptor {
            id: FactId(1),
            subject: None,
            snapshot: SnapshotId(1),
        }),
    }
}

/// Build an `EvidenceBundle` whose only entry is an `Evidence` on the
/// `"tests_passed"` slot at the Adequate grade. The default `required:
/// true` rule sees a graded entry.
fn bundle_with_passing_tests() -> EvidenceBundle {
    let entry = BundleEntry::Evidence {
        slot: slot(ProducerSource::CargoTest, "tests_passed"),
        descriptor: ev_descriptor(EvidenceGrade::Supports),
    };
    EvidenceBundle::new(
        EvidenceBundleId(100),
        WorkId::new(NamespacedName::new("ci.test").expect("valid name")),
        ExecutionId(1),
        SnapshotId(1),
        vec![entry],
    )
}

fn empty_bundle() -> EvidenceBundle {
    EvidenceBundle::new(
        EvidenceBundleId(101),
        WorkId::new(NamespacedName::new("ci.test").expect("valid name")),
        ExecutionId(1),
        SnapshotId(1),
        vec![],
    )
}

fn executor_requiring(min_grade: EvidenceGrade) -> DefaultTrialExecutor {
    let rule = GateRule {
        name: "require:tests_passed".to_string(),
        slot: slot(ProducerSource::CargoTest, "tests_passed"),
        min_grade: Some(min_grade),
        required: true,
    };
    DefaultTrialExecutor::new(PolicySpec::new(vec![rule]))
}

fn build_trial_input(proposal_id_str: &str, bundle: EvidenceBundle) -> TrialInput {
    let proposal = ChangeProposal::new(
        proposal_id(proposal_id_str),
        SoftwareWorldId::from_string("w-A"),
        ProposalKind::SourcePatch {
            patch_ref: "patch".to_string(),
        },
        RequestedBy::Human {
            user_ref: "alice".to_string(),
        },
    );
    TrialInput {
        proposal,
        world: world("w-B", 10),
        base_snapshot: SnapshotId::new(10),
        candidate_snapshot: SnapshotId::new(11),
        candidate_facts: vec![],
        work_results: vec![],
        evidence_bundle: bundle,
    }
}

// --- tests ----------------------------------------------------------

#[test]
fn pipeline_trial_with_missing_required_evidence_blocks_permit() {
    // Trial runs against an empty bundle. The required rule is
    // missing → `ProducerMissing` → `InsufficientEvidence` (NOT
    // `Block`, because the producer never ran). The permit must be
    // refused.
    let exec = executor_requiring(EvidenceGrade::Supports);
    let input = build_trial_input("p-1", empty_bundle());
    let trial_outcome = exec.run_trial(TrialId::from_string("t-1"), input.clone());
    let trial = assemble_trial_evidence(
        TrialId::from_string("t-1"),
        input,
        trial_outcome.gate.clone(),
    );
    assert_eq!(
        trial_outcome.gate.outcome,
        PolicyOutcome::InsufficientEvidence
    );
    // The gate's empty-bundle shortcut reports a single
    // "<empty>" reason with verdict `Absent`.
    assert!(matches!(
        trial_outcome.gate.reasons.as_slice(),
        [r] if r.rule == "<empty>" && matches!(r.verdict, ReasonVerdict::Absent)
    ));

    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 10);
    let run = evaluate_promotion(
        PromotionEvaluationInput {
            proposal: proposal_id("p-1"),
            base: &base,
            candidate: &candidate,
            current: &current,
        },
        Some(&trial),
    );
    assert_eq!(
        run.status,
        PromotionStatus::Blocked(PromotionBlockReason::TrialGateNotPassing {
            actual: PolicyOutcome::InsufficientEvidence
        })
    );

    let err = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), run)
        .expect_err("blocked dry-run must not issue a permit");
    assert!(matches!(
        err,
        PromotionPermitError::DryRunNotClean {
            actual: PromotionStatus::Blocked(_),
        }
    ));
}

#[test]
fn pipeline_trial_with_passing_evidence_yields_clean_promotion_and_permit() {
    // Trial runs against a bundle with the required rule at the
    // required grade. Trial gate passes, evaluation is
    // CleanPromotionReady, permit issues, apply yields Applied, and
    // the lineage is available for audit.
    let exec = executor_requiring(EvidenceGrade::Supports);
    let input = build_trial_input("p-1", bundle_with_passing_tests());
    let trial_outcome = exec.run_trial(TrialId::from_string("t-1"), input.clone());
    let trial = assemble_trial_evidence(
        TrialId::from_string("t-1"),
        input,
        trial_outcome.gate.clone(),
    );
    assert_eq!(trial_outcome.gate.outcome, PolicyOutcome::Pass);

    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 10);
    let run = evaluate_promotion(
        PromotionEvaluationInput {
            proposal: proposal_id("p-1"),
            base: &base,
            candidate: &candidate,
            current: &current,
        },
        Some(&trial),
    );
    assert_eq!(run.status, PromotionStatus::CleanPromotionReady);

    let permit = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), run)
        .expect("clean dry-run should issue a permit");
    assert_eq!(permit.issued_for_snapshot, SnapshotId::new(10));

    let outcome = apply_with_permit(&current, &permit);
    match outcome {
        PromotionApplyOutcome::Applied {
            applied_to_snapshot,
            ..
        } => assert_eq!(applied_to_snapshot, SnapshotId::new(10)),
        PromotionApplyOutcome::Rejected(e) => {
            panic!("expected Applied, got Rejected({e:?})")
        }
    }

    assert_eq!(permit.dry_run.lineage.base_snapshot, SnapshotId::new(10));
    assert_eq!(permit.dry_run.lineage.current_snapshot, SnapshotId::new(10));
    assert!(permit.dry_run.lineage.base_matches_current);
}

#[test]
fn pipeline_trial_with_insufficient_grade_blocks_permit() {
    // Trial runs against a bundle whose entry has a grade below the
    // policy's `min_grade`. The gate must come back as
    // `InsufficientEvidence` (the entry is present but too weak) and
    // the permit must be refused — *not* auto-promoted.
    let exec = executor_requiring(EvidenceGrade::Refutes);
    let input = build_trial_input("p-1", bundle_with_passing_tests()); // Supports, not Refutes
    let trial_outcome = exec.run_trial(TrialId::from_string("t-1"), input.clone());
    let trial = assemble_trial_evidence(
        TrialId::from_string("t-1"),
        input,
        trial_outcome.gate.clone(),
    );
    assert_eq!(
        trial_outcome.gate.outcome,
        PolicyOutcome::InsufficientEvidence
    );

    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 10);
    let run = evaluate_promotion(
        PromotionEvaluationInput {
            proposal: proposal_id("p-1"),
            base: &base,
            candidate: &candidate,
            current: &current,
        },
        Some(&trial),
    );
    assert_eq!(
        run.status,
        PromotionStatus::Blocked(PromotionBlockReason::TrialGateNotPassing {
            actual: PolicyOutcome::InsufficientEvidence
        })
    );

    let err = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), run)
        .expect_err("insufficient evidence must not issue a permit");
    assert!(matches!(
        err,
        PromotionPermitError::DryRunNotClean {
            actual: PromotionStatus::Blocked(_),
        }
    ));
}

/// Documented M9 assumption pin: the trial gate does not check author
/// class. An automated author (plugin or LLM agent) whose proposal has
/// no recorded human co-authorisation can still build a trial and a
/// dry-run. This is the contract being pinned. Any future ADR that
/// promotes automated authors to "require human co-auth" must extend
/// `evaluate_promotion` with a
/// `PromotionBlockReason::AutomatedAuthorWithoutCoAuth` check, at which
/// point this assertion is the breaking point.
#[test]
fn pipeline_automated_author_promotion_under_current_gate_contract() {
    let exec = executor_requiring(EvidenceGrade::Supports);
    let mut input = build_trial_input("p-1", bundle_with_passing_tests());
    // Override the proposal's author with an automated class.
    input.proposal = ChangeProposal::new(
        proposal_id("p-1"),
        SoftwareWorldId::from_string("w-A"),
        ProposalKind::DetectorChange {
            detector_ref: "det-x".to_string(),
        },
        RequestedBy::LlmAgent {
            agent_ref: "claude".to_string(),
        },
    );
    let trial_outcome = exec.run_trial(TrialId::from_string("t-1"), input.clone());
    let trial = assemble_trial_evidence(
        TrialId::from_string("t-1"),
        input,
        trial_outcome.gate.clone(),
    );
    assert_eq!(trial_outcome.gate.outcome, PolicyOutcome::Pass);

    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 10);
    let run = evaluate_promotion(
        PromotionEvaluationInput {
            proposal: proposal_id("p-1"),
            base: &base,
            candidate: &candidate,
            current: &current,
        },
        Some(&trial),
    );
    // Pinned: current contract lets the permit through.
    assert_eq!(run.status, PromotionStatus::CleanPromotionReady);
    let permit = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), run)
        .expect("permit should still issue under the current gate contract");
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
fn pipeline_world_drift_after_permit_rejects_apply_with_audit_marker() {
    // Permit issued over a clean dry-run. The world then drifts (new
    // base_snapshot). Apply must be rejected with
    // WorldDriftedSincePermit and the permit must remain an audit
    // marker — never silently re-applied.
    let exec = executor_requiring(EvidenceGrade::Supports);
    let input = build_trial_input("p-1", bundle_with_passing_tests());
    let trial_outcome = exec.run_trial(TrialId::from_string("t-1"), input.clone());
    let trial = assemble_trial_evidence(
        TrialId::from_string("t-1"),
        input,
        trial_outcome.gate.clone(),
    );

    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 10);
    let run = evaluate_promotion(
        PromotionEvaluationInput {
            proposal: proposal_id("p-1"),
            base: &base,
            candidate: &candidate,
            current: &current,
        },
        Some(&trial),
    );
    let permit = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), run).unwrap();

    let drifted = world("w-C", 12);
    let outcome = apply_with_permit(&drifted, &permit);
    assert_eq!(
        outcome,
        PromotionApplyOutcome::Rejected(PromotionApplyError::WorldDriftedSincePermit {
            permit_snapshot: SnapshotId::new(10),
            current_snapshot: SnapshotId::new(12),
        })
    );

    let preflight = assert_world_matches_permit(&drifted, &permit);
    assert!(matches!(
        preflight,
        Err(PromotionApplyError::WorldDriftedSincePermit { .. })
    ));
}
