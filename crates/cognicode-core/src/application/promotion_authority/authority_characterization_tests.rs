//! e80a WU0 — authority-gap characterization against the CURRENT code.
//!
//! These tests do not assert a desired invariant; they ASSERT THE HOLE that
//! e80a closes. They pass on the pre-e80a code and are the executable proof
//! that a machine-authored proposal can be promoted with no external human
//! approval. See `characterization.md`.
//!
//! WU5 changes `issue_promotion_permit` to require a sealed
//! `PromotionAuthorization`. When that lands, these characterization tests are
//! rewritten: the "hole" assertions become "refused" assertions, and the
//! human-path test is preserved. Until then they are the baseline evidence.

use crate::application::change_proposal::executor::{DefaultTrialExecutor, TrialExecutor};
use crate::application::change_proposal::proposal::{
    ChangeProposal, ChangeProposalId, ProposalKind, RequestedBy,
};
use crate::application::change_proposal::trial::{
    TrialId, TrialInput, assemble_trial_evidence,
};
use crate::application::change_tracking::planner::WorkId;
use crate::application::evidence_bundle::{BundleEntry, EvidenceBundle, EvidenceBundleId};
use crate::application::evidence_bundle::{ProducerSlot, ProducerSource};
use crate::application::policy_gate::{GateRule, PolicyOutcome, PolicySpec};
use crate::application::promotion_authority::evaluation::{
    PromotionEvaluationInput, PromotionStatus, evaluate_promotion,
};
use crate::application::promotion_authority::permit::{
    PromotionApplyOutcome, PromotionPermitId, apply_with_permit, issue_promotion_permit,
};
use crate::application::software_world::world::{SoftwareWorld, SoftwareWorldId};
use crate::domain::execution::actor::ActorRef;
use crate::domain::evidence_kernel::ids::{EvidenceId, FactId, SnapshotId};
use crate::domain::findings::ports::{EvidenceDescriptor, FactDescriptor, FactSlot};
use crate::domain::kernel_ids::{EvidenceGrade, ExecutionId};
use crate::domain::naming::NamespacedName;

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

/// Build a passing `TrialEvidence` for a proposal with the given author.
fn passing_trial(id: &str, requested_by: RequestedBy) -> crate::application::change_proposal::trial::TrialEvidence {
    let proposal = ChangeProposal::new(
        proposal_id(id),
        SoftwareWorldId::from_string("w-A"),
        ProposalKind::SourcePatch {
            patch_ref: "patch".to_string(),
        },
        requested_by,
    );
    let input = TrialInput {
        proposal,
        world: world("w-B", 10),
        base_snapshot: SnapshotId::new(10),
        candidate_snapshot: SnapshotId::new(11),
        candidate_facts: vec![],
        work_results: vec![],
        evidence_bundle: passing_bundle(),
    };
    let outcome = executor().run_trial(TrialId::from_string("t-1"), input.clone());
    assert_eq!(
        outcome.gate.outcome,
        PolicyOutcome::Pass,
        "characterization fixture must produce a passing trial"
    );
    assemble_trial_evidence(TrialId::from_string("t-1"), input, outcome.gate.clone())
}

/// Evaluate a clean promotion for the given proposal id.
fn clean_dry_run(id: &str, trial: &crate::application::change_proposal::trial::TrialEvidence) -> crate::application::promotion_authority::evaluation::PromotionDryRun {
    let base = world("w-A", 10);
    let candidate = world("w-B", 10);
    let current = world("w-C", 10);
    let run = evaluate_promotion(
        PromotionEvaluationInput {
            proposal: proposal_id(id),
            base: &base,
            candidate: &candidate,
            current: &current,
        },
        Some(trial),
    );
    assert_eq!(run.status, PromotionStatus::CleanPromotionReady);
    run
}

// --- characterization: the hole -------------------------------------

/// HOLE: an LLM-authored proposal with a perfect trial gets a permit with no
/// approval of any kind.
#[test]
fn characterization_llmagent_author_gets_a_permit_with_no_approval() {
    let trial = passing_trial(
        "p-1",
        RequestedBy::LlmAgent {
            agent_ref: "claude".to_string(),
        },
    );
    let run = clean_dry_run("p-1", &trial);

    // The current minting API takes only (id, dry_run). Authorship is not even
    // an argument, so it cannot possibly be checked here.
    let permit = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), run)
        .expect("CURRENT BEHAVIOUR: automated author mints a permit with no approval");

    let current = world("w-C", 10);
    assert!(matches!(
        apply_with_permit(&current, &permit),
        PromotionApplyOutcome::Applied { .. }
    ));
}

/// HOLE: same for a plugin author.
#[test]
fn characterization_plugin_author_gets_a_permit_with_no_approval() {
    let trial = passing_trial(
        "p-1",
        RequestedBy::Plugin {
            plugin_ref: "pack-x".to_string(),
        },
    );
    let run = clean_dry_run("p-1", &trial);

    let permit = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), run)
        .expect("CURRENT BEHAVIOUR: plugin author mints a permit with no approval");
    assert_eq!(permit.proposal, proposal_id("p-1"));
}

/// The Human-authored path that e80a must PRESERVE: a clean dry-run still
/// promotes, with no new co-approval requirement.
#[test]
fn characterization_human_author_path_is_preserved() {
    let trial = passing_trial(
        "p-1",
        RequestedBy::Human {
            user_ref: "alice".to_string(),
        },
    );
    let run = clean_dry_run("p-1", &trial);

    let permit = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), run)
        .expect("human author with a clean dry-run must keep promoting");
    let current = world("w-C", 10);
    assert!(matches!(
        apply_with_permit(&current, &permit),
        PromotionApplyOutcome::Applied { .. }
    ));
}

/// An `ActorRef` is caller-constructible data, not proof of authority. This is
/// why e80a models approval as a sealed artifact and never as an `ActorRef`
/// field.
#[test]
fn characterization_actorref_human_is_caller_constructible_data() {
    let forged = ActorRef::human("alice");
    assert_eq!(forged.kind, crate::domain::execution::actor::ActorKind::Human);
    assert_eq!(forged.id, "alice");
    // Any caller can write the line above. Nothing about it proves that
    // "alice" approved anything.
}
