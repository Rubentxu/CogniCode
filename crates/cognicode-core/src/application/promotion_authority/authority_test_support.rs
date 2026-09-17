//! Shared fixtures for the e80a authority test suites.
//!
//! Used by `authority_characterization_tests.rs` (the flipped WU0
//! characterization) and `authority_tests.rs` (the WU7 adversarial suite).

use crate::application::change_proposal::executor::{DefaultTrialExecutor, TrialExecutor};
use crate::application::change_proposal::proposal::{
    ChangeProposal, ChangeProposalId, ProposalKind, RequestedBy,
};
use crate::application::change_proposal::trial::{
    TrialEvidence, TrialId, TrialInput, assemble_trial_evidence,
};
use crate::application::change_tracking::planner::WorkId;
use crate::application::evidence_bundle::{BundleEntry, EvidenceBundle, EvidenceBundleId};
use crate::application::evidence_bundle::{ProducerSlot, ProducerSource};
use crate::application::policy_gate::{GateRule, PolicyOutcome, PolicySpec};
use crate::application::promotion_authority::evaluation::{
    PromotionDryRun, PromotionEvaluationInput, PromotionStatus, evaluate_promotion,
};
use crate::application::software_world::world::{SoftwareWorld, SoftwareWorldId};
use crate::domain::evidence_kernel::ids::{EvidenceId, FactId, SnapshotId};
use crate::domain::findings::ports::{EvidenceDescriptor, FactDescriptor, FactSlot};
use crate::domain::kernel_ids::{EvidenceGrade, ExecutionId};
use crate::domain::naming::NamespacedName;

pub(crate) fn world(id: &str, snap: u64) -> SoftwareWorld {
    SoftwareWorld::new_base(SoftwareWorldId::from_string(id), SnapshotId::new(snap))
}

pub(crate) fn proposal_id(s: &str) -> ChangeProposalId {
    ChangeProposalId::from_string(s)
}

pub(crate) fn proposal(id: &str, requested_by: RequestedBy) -> ChangeProposal {
    ChangeProposal::new(
        proposal_id(id),
        SoftwareWorldId::from_string("w-A"),
        ProposalKind::SourcePatch {
            patch_ref: "patch".to_string(),
        },
        requested_by,
    )
}

pub(crate) fn human() -> RequestedBy {
    RequestedBy::Human {
        user_ref: "alice".to_string(),
    }
}

pub(crate) fn llm_agent() -> RequestedBy {
    RequestedBy::LlmAgent {
        agent_ref: "claude".to_string(),
    }
}

pub(crate) fn plugin() -> RequestedBy {
    RequestedBy::Plugin {
        plugin_ref: "pack-x".to_string(),
    }
}

fn slot(source: ProducerSource, id: &str) -> ProducerSlot {
    ProducerSlot {
        source,
        slot_id: id.to_string(),
    }
}

fn passing_bundle() -> EvidenceBundle {
    let entry = BundleEntry::Evidence {
        slot: slot(ProducerSource::CargoTest, "tests_passed"),
        descriptor: EvidenceDescriptor {
            id: EvidenceId(1),
            grade: EvidenceGrade::Supports,
            fact: FactSlot::Resolved(FactDescriptor {
                id: FactId(1),
                subject: None,
                snapshot: SnapshotId(1),
            }),
        },
    };
    EvidenceBundle::new(
        EvidenceBundleId(100),
        WorkId::new(NamespacedName::new("ci.test").expect("valid name")),
        ExecutionId(1),
        SnapshotId(1),
        vec![entry],
    )
}

fn executor() -> DefaultTrialExecutor {
    let rule = GateRule {
        name: "require:tests_passed".to_string(),
        slot: slot(ProducerSource::CargoTest, "tests_passed"),
        min_grade: Some(EvidenceGrade::Supports),
        required: true,
    };
    DefaultTrialExecutor::new(PolicySpec::new(vec![rule]))
}

/// Run a trial for `p` and assert its gate passed. The trial's candidate world
/// is `w-B` and the snapshots are base 10 / candidate 11.
pub(crate) fn passing_trial_for(p: &ChangeProposal) -> TrialEvidence {
    let input = trial_input_for(p);
    let outcome = executor().run_trial(TrialId::from_string("t-1"), input.clone());
    assert_eq!(
        outcome.gate.outcome,
        PolicyOutcome::Pass,
        "fixture must produce a passing trial"
    );
    assemble_trial_evidence(TrialId::from_string("t-1"), input, outcome.gate.clone())
}

fn trial_input_for(p: &ChangeProposal) -> TrialInput {
    TrialInput {
        proposal: p.clone(),
        world: world("w-B", 10),
        base_snapshot: SnapshotId::new(10),
        candidate_snapshot: SnapshotId::new(11),
        candidate_facts: vec![],
        work_results: vec![],
        evidence_bundle: passing_bundle(),
    }
}

/// A clean three-way dry-run for `proposal_id_str`: base `w-A@10`, candidate
/// `w-B@10`, current `w-C@10`.
pub(crate) fn clean_dry_run(proposal_id_str: &str, trial: &TrialEvidence) -> PromotionDryRun {
    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 10);
    let run = evaluate_promotion(
        PromotionEvaluationInput {
            proposal: proposal_id(proposal_id_str),
            base: &base,
            candidate: &candidate,
            current: &current,
        },
        Some(trial),
    );
    assert_eq!(run.status, PromotionStatus::CleanPromotionReady);
    run
}

/// A dry-run whose base/current snapshots differ (world drift).
pub(crate) fn drifted_dry_run(proposal_id_str: &str, trial: &TrialEvidence) -> PromotionDryRun {
    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 12);
    evaluate_promotion(
        PromotionEvaluationInput {
            proposal: proposal_id(proposal_id_str),
            base: &base,
            candidate: &candidate,
            current: &current,
        },
        Some(trial),
    )
}

/// A dry-run whose candidate world differs from `clean_dry_run`'s, used for
/// the approval-replay case.
pub(crate) fn dry_run_for_candidate(
    proposal_id_str: &str,
    candidate: &SoftwareWorld,
    trial: &TrialEvidence,
) -> PromotionDryRun {
    let base = world("w-A", 10);
    let current = world("w-C", 10);
    evaluate_promotion(
        PromotionEvaluationInput {
            proposal: proposal_id(proposal_id_str),
            base: &base,
            candidate,
            current: &current,
        },
        Some(trial),
    )
}
