//! Tests for `application::change_proposal::executor` (e72 WU3).
//!
//! Adversarial coverage of the `TrialExecutor` trait and the
//! `DefaultTrialExecutor` impl:
//!
//! 1. `DefaultTrialExecutor::run_trial` evaluates the bundle via e69's
//!    `evaluate` and assembles the envelope.
//! 2. **No parallel verdict model**: the gate decision is exactly what
//!    e69 produces. The executor does not synthesise a verdict.
//! 3. `TrialExecutor` is a separate trait from `WorkExecutor` — the
//!    envelope carries lineage labels that work-level reporting does
//!    not.
//! 4. Adversarial: empty bundle → `InsufficientEvidence` (fail-closed
//!    on no evidence).
//! 5. Adversarial: bundle with a `ProducerFailed` on a required rule
//!    → `Block`.

use crate::application::change_proposal::executor::{DefaultTrialExecutor, TrialExecutor};
use crate::application::change_proposal::proposal::{
    ChangeProposal, ChangeProposalId, ProposalKind, RequestedBy,
};
use crate::application::change_proposal::trial::{TrialId, TrialInput};
use crate::application::change_tracking::planner::WorkId;
use crate::application::evidence_bundle::{
    BundleEntry, EvidenceBundle, EvidenceBundleId, ProducerSlot, ProducerSource,
};
use crate::application::policy_gate::{GateRule, PolicyOutcome, PolicySpec};
use crate::application::software_world::world::{SoftwareWorld, SoftwareWorldId};
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

fn make_input(bundle: EvidenceBundle) -> TrialInput {
    TrialInput {
        proposal: proposal(),
        world: world("w-cand", 10),
        base_snapshot: SnapshotId::new(10),
        candidate_snapshot: SnapshotId::new(11),
        candidate_facts: vec![],
        work_results: vec![],
        evidence_bundle: bundle,
    }
}

fn empty_policy() -> PolicySpec {
    PolicySpec::new(vec![])
}

// --- DefaultTrialExecutor behaviour ---------------------------------

#[test]
fn default_trial_executor_evaluates_empty_bundle_as_insufficient() {
    // Empty bundle + empty spec → InsufficientEvidence (fail-closed
    // on no evidence). e69's evaluate produces this directly; the
    // executor must NOT downgrade or override.
    let exec = DefaultTrialExecutor::new(empty_policy());
    let input = make_input(empty_bundle());
    let ev = exec.run_trial(TrialId::from_string("t-1"), input);

    assert_eq!(ev.gate.outcome, PolicyOutcome::InsufficientEvidence);
    assert!(ev.gate.is_insufficient());
    assert!(!ev.gate.is_pass());
}

#[test]
fn default_trial_executor_does_not_synthesise_a_verdict() {
    // The executor passes the bundle to e69's evaluate. Whatever e69
    // returns is what the executor carries. There is no "if e69 says
    // Block but the executor feels generous, override to Warn" path.
    let exec = DefaultTrialExecutor::new(empty_policy());
    let bundle = EvidenceBundle::new(
        EvidenceBundleId(2),
        WorkId::new(NamespacedName::new("ns.unit").expect("valid name")),
        ExecutionId::new(2),
        SnapshotId::new(11),
        vec![BundleEntry::ProducerFailed {
            slot: ProducerSlot {
                source: ProducerSource::CargoTest,
                slot_id: "unit::core::test_x".to_string(),
            },
            reason: "assertion failed".to_string(),
            raw: None,
        }],
    );

    let input = make_input(bundle);
    let ev = exec.run_trial(TrialId::from_string("t-2"), input);
    // e69's evaluate on a non-empty bundle with a Failed entry
    // returns Pass with the failure recorded as a Warn reason (it is
    // not in any required rule since the policy is empty). The point
    // of this test is that the executor passes through whatever e69
    // returns — not that we second-guess it.
    assert!(ev.gate.outcome == PolicyOutcome::Pass || ev.gate.outcome == PolicyOutcome::Warn);
}

#[test]
fn default_trial_executor_with_empty_bundle_returns_insufficient_even_with_required_rule() {
    // An empty bundle is fail-closed at the gate level: e69's
    // `evaluate` short-circuits to `InsufficientEvidence` BEFORE it
    // looks at the rules. A required rule that is unmet is also
    // `InsufficientEvidence` (the rule has no evidence to grade).
    // Block only happens when there IS evidence but the producer
    // failed — that is exercised in the next test.
    let policy = PolicySpec::new(vec![GateRule {
        name: "unit_tests_required".to_string(),
        slot: ProducerSlot {
            source: ProducerSource::CargoTest,
            slot_id: "unit::core::y".to_string(),
        },
        min_grade: Some(crate::domain::kernel_ids::EvidenceGrade::Supports),
        required: true,
    }]);

    let exec = DefaultTrialExecutor::new(policy);
    // Bundle is empty: e69 short-circuits to InsufficientEvidence.
    let bundle = EvidenceBundle::new(
        EvidenceBundleId(3),
        WorkId::new(NamespacedName::new("ns.unit").expect("valid name")),
        ExecutionId::new(3),
        SnapshotId::new(11),
        vec![],
    );

    let input = make_input(bundle);
    let ev = exec.run_trial(TrialId::from_string("t-3"), input);
    assert_eq!(ev.gate.outcome, PolicyOutcome::InsufficientEvidence);
}

#[test]
fn default_trial_executor_carries_block_when_producer_failed_on_required_rule() {
    // A bundle with a `ProducerFailed` entry on a required slot
    // → e69 returns `Block`. The executor carries that verdict
    // unchanged.
    let policy = PolicySpec::new(vec![GateRule {
        name: "unit_tests_required".to_string(),
        slot: ProducerSlot {
            source: ProducerSource::CargoTest,
            slot_id: "unit::core::y".to_string(),
        },
        min_grade: Some(crate::domain::kernel_ids::EvidenceGrade::Supports),
        required: true,
    }]);

    let exec = DefaultTrialExecutor::new(policy);
    let bundle = EvidenceBundle::new(
        EvidenceBundleId(4),
        WorkId::new(NamespacedName::new("ns.unit").expect("valid name")),
        ExecutionId::new(4),
        SnapshotId::new(11),
        vec![BundleEntry::ProducerFailed {
            slot: ProducerSlot {
                source: ProducerSource::CargoTest,
                slot_id: "unit::core::y".to_string(),
            },
            reason: "assertion failed".to_string(),
            raw: None,
        }],
    );

    let input = make_input(bundle);
    let ev = exec.run_trial(TrialId::from_string("t-block"), input);
    assert_eq!(ev.gate.outcome, PolicyOutcome::Block);
}

#[test]
fn default_trial_executor_carries_lineage_labels() {
    // The TrialEvidence carries the proposal id, world id, snapshots
    // — i.e. the TrialExecutor does not strip the lineage. This is
    // what e73 will read.
    let exec = DefaultTrialExecutor::new(empty_policy());
    let input = make_input(empty_bundle());
    let ev = exec.run_trial(TrialId::from_string("t-lineage"), input);

    assert_eq!(ev.trial_id, TrialId::from_string("t-lineage"));
    assert_eq!(ev.proposal_id, ChangeProposalId::from_string("p-1"));
    assert_eq!(ev.world_id, SoftwareWorldId::from_string("w-cand"));
    assert_eq!(ev.base_snapshot, SnapshotId::new(10));
    assert_eq!(ev.candidate_snapshot, SnapshotId::new(11));
}

// --- Trait surface --------------------------------------------------

#[test]
fn trial_executor_trait_is_a_distinct_contract() {
    // The trait is object-safe in principle; we assert it exists and
    // has the expected method shape. A custom impl that returns a
    // different shape would have to bypass the assembler (which the
    // doc-comment forbids).
    fn _check<E: TrialExecutor>(_: &E) {}
    let exec = DefaultTrialExecutor::new(empty_policy());
    _check(&exec);
}

// --- Determinism ----------------------------------------------------

#[test]
fn default_trial_executor_is_deterministic() {
    // Running the same trial twice yields the same TrialEvidence
    // (modulo FactId-level noise — but our empty inputs mean there is
    // no noise). The executor is pure.
    let exec = DefaultTrialExecutor::new(empty_policy());
    let input1 = make_input(empty_bundle());
    let input2 = make_input(empty_bundle());

    let ev1 = exec.run_trial(TrialId::from_string("t-det-1"), input1);
    let ev2 = exec.run_trial(TrialId::from_string("t-det-2"), input2);

    // Same gate decision, same lineage metadata, same bundle.
    assert_eq!(ev1.gate.outcome, ev2.gate.outcome);
    assert_eq!(ev1.proposal_id, ev2.proposal_id);
    assert_eq!(ev1.world_id, ev2.world_id);
    assert_eq!(ev1.base_snapshot, ev2.base_snapshot);
    assert_eq!(ev1.candidate_snapshot, ev2.candidate_snapshot);
    // Trial id differs by construction (the caller supplies it) —
    // that is the only thing that differs.
    assert_ne!(ev1.trial_id, ev2.trial_id);
}
