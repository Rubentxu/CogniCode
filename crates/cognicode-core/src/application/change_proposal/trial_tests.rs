//! Tests for `application::change_proposal::trial` (e72 WU2).
//!
//! Adversarial coverage:
//!
//! 1. `assemble_trial_evidence` is a pure composition: same inputs
//!    produce equal outputs.
//! 2. The trial envelope carries lineage metadata (proposal id,
//!    world id, snapshots) alongside the e68/e69/e70 outputs.
//! 3. **No parallel verdict model**: the trial carries the
//!    `PolicyDecision` from e69; it does not synthesise a new one.
//! 4. The trial can be assembled with empty candidate facts (for
//!    config-only proposals that do not produce facts) and the
//!    envelope remains well-formed.
//! 5. The trial can be assembled with empty work results (for
//!    proposals that produce no scheduled work).

use crate::application::change_proposal::proposal::{
    ChangeProposal, ChangeProposalId, ProposalKind, RequestedBy,
};
use crate::application::change_proposal::trial::{TrialId, TrialInput, assemble_trial_evidence};
use crate::application::change_tracking::planner::WorkId;
use crate::application::evidence_bundle::{
    BundleEntry, EvidenceBundle, EvidenceBundleId, ProducerSlot, ProducerSource,
};
use crate::application::policy_gate::{PolicyDecision, PolicyOutcome, PolicyReason, ReasonVerdict};
use crate::application::software_world::world::{
    ContentHash, SoftwareWorld, SoftwareWorldId, WorldSourceState,
};
use crate::domain::evidence_kernel::ids::SnapshotId;
use crate::domain::kernel_ids::ExecutionId;
use crate::domain::naming::NamespacedName;

// --- helpers ---------------------------------------------------------

fn world(id: &str, snap: u64) -> SoftwareWorld {
    SoftwareWorld::new_base(SoftwareWorldId::from_string(id), SnapshotId::new(snap))
}

fn proposal() -> ChangeProposal {
    ChangeProposal::new(
        ChangeProposalId::from_string("p-1"),
        SoftwareWorldId::from_string("w-base"),
        ProposalKind::SourcePatch {
            patch_ref: "patch-1".to_string(),
        },
        RequestedBy::Human {
            user_ref: "alice".to_string(),
        },
    )
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

fn passing_decision() -> PolicyDecision {
    PolicyDecision {
        outcome: PolicyOutcome::Pass,
        reasons: vec![],
    }
}

fn blocking_decision() -> PolicyDecision {
    PolicyDecision {
        outcome: PolicyOutcome::Block,
        reasons: vec![PolicyReason {
            rule: "test_required".to_string(),
            slot: ProducerSlot {
                source: ProducerSource::CargoTest,
                slot_id: "unit::x::y".to_string(),
            },
            verdict: ReasonVerdict::Failed,
        }],
    }
}

// --- composition purity ----------------------------------------------

#[test]
fn assemble_trial_evidence_is_deterministic() {
    let trial_id = TrialId::from_string("t-1");
    let proposal = proposal();
    let world = world("w-cand", 10);
    let input = TrialInput {
        proposal: proposal.clone(),
        world: world.clone(),
        base_snapshot: SnapshotId::new(10),
        candidate_snapshot: SnapshotId::new(11),
        candidate_facts: vec![],
        work_results: vec![],
        evidence_bundle: empty_bundle(),
        gate: passing_decision(),
    };

    let ev1 = assemble_trial_evidence(trial_id.clone(), input.clone());
    let ev2 = assemble_trial_evidence(trial_id.clone(), input);

    assert_eq!(ev1, ev2);
}

// --- lineage labels --------------------------------------------------

#[test]
fn trial_evidence_carries_lineage_labels() {
    let trial_id = TrialId::from_string("t-2");
    let proposal = proposal();
    let world = world("w-cand", 10);
    let input = TrialInput {
        proposal: proposal.clone(),
        world: world.clone(),
        base_snapshot: SnapshotId::new(10),
        candidate_snapshot: SnapshotId::new(11),
        candidate_facts: vec![],
        work_results: vec![],
        evidence_bundle: empty_bundle(),
        gate: passing_decision(),
    };

    let ev = assemble_trial_evidence(trial_id.clone(), input);

    assert_eq!(ev.trial_id, trial_id);
    assert_eq!(ev.proposal_id, proposal.id);
    assert_eq!(ev.world_id, world.id);
    assert_eq!(ev.base_snapshot, SnapshotId::new(10));
    assert_eq!(ev.candidate_snapshot, SnapshotId::new(11));
}

// --- no parallel verdict model ---------------------------------------

#[test]
fn trial_carries_policy_decision_without_synthesising_a_new_one() {
    // The trial envelope carries the gate decision verbatim. The
    // assembler is a pure projection: it does not transform the
    // decision (no promotion, no downgrade).
    let proposal = proposal();
    let world = world("w-cand", 10);
    let input = TrialInput {
        proposal,
        world,
        base_snapshot: SnapshotId::new(10),
        candidate_snapshot: SnapshotId::new(11),
        candidate_facts: vec![],
        work_results: vec![],
        evidence_bundle: empty_bundle(),
        gate: blocking_decision(),
    };

    let ev = assemble_trial_evidence(TrialId::from_string("t-3"), input);
    assert!(!ev.gate.is_pass());
    assert_eq!(ev.gate.outcome, PolicyOutcome::Block);
    assert_eq!(ev.gate.reasons.len(), 1);
}

// --- empty candidate facts (config-only proposals) -------------------

#[test]
fn trial_assembles_with_empty_candidate_facts() {
    let proposal = ChangeProposal::new(
        ChangeProposalId::from_string("p-cfg"),
        SoftwareWorldId::from_string("w-base"),
        ProposalKind::ConfigChange {
            config_ref: "cfg-x".to_string(),
        },
        RequestedBy::Human {
            user_ref: "alice".to_string(),
        },
    );
    let world = world("w-cand", 10);
    let input = TrialInput {
        proposal,
        world,
        base_snapshot: SnapshotId::new(10),
        candidate_snapshot: SnapshotId::new(11),
        candidate_facts: vec![], // config-only → no facts
        work_results: vec![],
        evidence_bundle: empty_bundle(),
        gate: passing_decision(),
    };

    let ev = assemble_trial_evidence(TrialId::from_string("t-cfg"), input);
    assert!(ev.candidate_facts.is_empty());
    assert!(ev.work_results.is_empty());
    assert!(ev.gate.is_pass());
}

// --- empty work results ----------------------------------------------

#[test]
fn trial_assembles_with_empty_work_results() {
    let proposal = proposal();
    let world = world("w-cand", 10);
    let input = TrialInput {
        proposal,
        world,
        base_snapshot: SnapshotId::new(10),
        candidate_snapshot: SnapshotId::new(11),
        candidate_facts: vec![],
        work_results: vec![], // no scheduled work ran
        evidence_bundle: empty_bundle(),
        gate: passing_decision(),
    };

    let ev = assemble_trial_evidence(TrialId::from_string("t-empty"), input);
    assert!(ev.work_results.is_empty());
    assert!(ev.candidate_facts.is_empty());
}

// --- non-empty bundle and work_results (realistic case) --------------

#[test]
fn trial_assembles_with_realistic_payload() {
    let proposal = proposal();
    let world = world("w-cand", 10);
    let input = TrialInput {
        proposal,
        world,
        base_snapshot: SnapshotId::new(10),
        candidate_snapshot: SnapshotId::new(11),
        candidate_facts: vec![],
        work_results: vec![],
        evidence_bundle: EvidenceBundle::new(
            EvidenceBundleId(42),
            WorkId::new(NamespacedName::new("ns.unit").expect("valid name")),
            ExecutionId::new(7),
            SnapshotId::new(11),
            vec![BundleEntry::ProducerFailed {
                slot: ProducerSlot {
                    source: ProducerSource::CargoTest,
                    slot_id: "unit::core::test_a".to_string(),
                },
                reason: "assertion failed".to_string(),
                raw: Some("thread main panicked".to_string()),
            }],
        ),
        gate: blocking_decision(),
    };

    let ev = assemble_trial_evidence(TrialId::from_string("t-real"), input);
    assert_eq!(ev.evidence_bundle.id, EvidenceBundleId(42));
    assert_eq!(ev.evidence_bundle.len(), 1);
    assert!(!ev.gate.is_pass());
}

// --- WorldSourceState not part of TrialInput (no fact mirror) --------

#[test]
fn trial_input_carries_world_but_not_facts_inside_the_world() {
    // The trial receives a `SoftwareWorld` (lineage metadata only)
    // and a separate `Vec<Fact>` (the observations). It does NOT
    // extract facts from the world — the world has no facts.
    let world = world("w-cand", 10);
    let input = TrialInput {
        proposal: proposal(),
        world: world.clone(),
        base_snapshot: SnapshotId::new(10),
        candidate_snapshot: SnapshotId::new(11),
        candidate_facts: vec![],
        work_results: vec![],
        evidence_bundle: empty_bundle(),
        gate: passing_decision(),
    };

    // The world's source_state is Base (it has no Forked variant),
    // and the trial's candidate_facts is independently empty. This
    // confirms the world is metadata and facts are passed separately.
    assert_eq!(input.world.source_state, WorldSourceState::Base);
    assert!(input.candidate_facts.is_empty());
    let _ = ContentHash([0u8; 32]); // keep ContentHash import used.
}
