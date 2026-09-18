//! e83 — governed improvement adversarial + positive acceptance tests.
//!
//! Load-bearing invariants under test:
//!
//! ```text
//! OPTIMIZE improvement  +  CONFIRM regression      = NO governed promotion
//! HeldOut PASS + Trial PASS + approval + FAIL CONFIRM = NO governed permit
//! ```

use std::path::PathBuf;

use crate::application::change_proposal::proposal::{
    ChangeProposal, ChangeProposalId, ProposalKind, RequestedBy,
};
use crate::application::change_proposal::trial::{
    TrialEvidence, TrialId, TrialInput, assemble_trial_evidence,
};
use crate::application::change_tracking::planner::WorkId;
use crate::application::evidence_bundle::{EvidenceBundle, EvidenceBundleId};
use crate::application::governed_improvement::binding::{
    CandidateFreeze, GovernedBindingError, GovernedCandidateBinding,
};
use crate::application::governed_improvement::permit::{
    GovernedImprovementPermit, GovernedImprovementReceipt, GovernedPermitError,
    apply_governed_improvement,
};
use crate::application::governed_improvement::policy::{
    HeldOutGateError, HeldOutPolicySpec, HeldOutPromotionGate, HeldOutRejection,
};
use crate::application::historical_replay::case::{ContentRef, HistoricalCase, HistoricalCaseId};
use crate::application::historical_replay::corpus::HistoricalCorpus;
use crate::application::historical_replay::plan::HistoricalReplayPlan;
use crate::application::historical_replay::replay::HistoricalPredictor;
use crate::application::historical_replay::split::{DatasetRole, DatasetSplit};
use crate::application::policy_gate::{PolicyDecision, PolicyOutcome};
use crate::application::promotion_authority::authorization::{
    ExternalApprovalAuthority, ExternalApprovalRequest, ExternalApprovalVerifier,
    PromotionApprovalTarget, PromotionAuthorizationPolicy, VerifiedExternalApproval,
};
use crate::application::promotion_authority::evaluation::{
    GovernedPromotionInput, PromotionBlockReason, PromotionStatus, evaluate_promotion_lineage,
};
use crate::application::promotion_authority::permit::{
    PromotionApplyOutcome, PromotionPermitId, issue_promotion_permit,
};
use crate::application::self_hosting::platform_equivalence::{
    DefaultNormaliser, HistoricalReplay, PlatformKind, PlatformObservation,
};
use crate::application::self_hosting::prediction::{ExpectedObservation, SealedPrediction};
use crate::application::shadow_evaluation::analyzer::{AnalyzerDescriptor, AnalyzerUnderTest};
use crate::application::shadow_evaluation::classifiers::BuiltinFailureRegimeClassifier;
use crate::application::shadow_evaluation::plan::ShadowEvaluationPlan;
use crate::application::shadow_evaluation::report::{ShadowEvaluator, ShadowRoleReport};
use crate::application::software_world::fork::{SourceMutation, fork};
use crate::application::software_world::world::{ContentHash, SoftwareWorld, SoftwareWorldId};
use crate::domain::evidence_kernel::ids::SnapshotId;
use crate::domain::execution::actor::ActorRef;
use crate::domain::kernel_ids::ExecutionId;
use crate::domain::naming::NamespacedName;

// ── fixtures ────────────────────────────────────────────────────────

fn wid(s: &str) -> SoftwareWorldId {
    SoftwareWorldId::from_string(s)
}

fn case_id(s: &str) -> HistoricalCaseId {
    HistoricalCaseId::try_new(s).expect("non-empty case id")
}

fn obs(id: &str, fired: bool) -> PlatformObservation {
    PlatformObservation {
        platform: PlatformKind::Linux,
        id: id.to_string(),
        raw: if fired {
            b"present".to_vec()
        } else {
            Vec::new()
        },
    }
}

fn case(id: &str, observations: Vec<PlatformObservation>) -> HistoricalCase {
    HistoricalCase::try_new(
        case_id(id),
        ContentRef::try_new(format!("src://base/{id}")).unwrap(),
        format!("base-snap-{id}"),
        ContentRef::try_new(format!("src://successor/{id}")).unwrap(),
        HistoricalReplay::observation_only(format!("snap-{id}"), observations),
    )
    .expect("valid case")
}

#[derive(Default)]
struct Scripted {
    label: String,
    script: Vec<(String, Vec<(String, bool)>)>,
}

impl Scripted {
    fn new(label: &str, script: Vec<(&str, Vec<(&str, bool)>)>) -> Self {
        Self {
            label: label.to_string(),
            script: script
                .into_iter()
                .map(|(id, e)| {
                    (
                        id.to_string(),
                        e.into_iter().map(|(o, p)| (o.to_string(), p)).collect(),
                    )
                })
                .collect(),
        }
    }
}

impl HistoricalPredictor for Scripted {
    fn predict(
        &self,
        input: &crate::application::historical_replay::case::HistoricalPredictionInput,
    ) -> SealedPrediction {
        let expectations: Vec<ExpectedObservation> = self
            .script
            .iter()
            .find(|(id, _)| id == input.case_id.as_str())
            .map(|(_, e)| e)
            .into_iter()
            .flatten()
            .map(|(id, present)| ExpectedObservation {
                id: id.clone(),
                expected_present: *present,
            })
            .collect();
        SealedPrediction::seal(
            format!("{}-{}", self.label, input.case_id.as_str()),
            expectations,
        )
    }
}

/// The full governed fixture.
struct Harness {
    proposal: ChangeProposal,
    base: SoftwareWorld,
    candidate: SoftwareWorld,
    current: SoftwareWorld,
    plan: ShadowEvaluationPlan,
    current_desc: AnalyzerDescriptor,
    candidate_desc: AnalyzerDescriptor,
}

fn harness() -> Harness {
    // base w-A@10  →  candidate w-B (forked, inherits base snapshot)  →  current w-C@10
    let base = SoftwareWorld::new_base(wid("w-A"), SnapshotId::new(10));
    let candidate = fork(
        &base,
        wid("w-B"),
        SourceMutation {
            new_path: PathBuf::from("src/candidate.rs"),
            new_content_hash: ContentHash([7u8; 32]),
        },
    )
    .new_world;
    let current = SoftwareWorld::new_base(wid("w-C"), SnapshotId::new(10));

    let proposal = ChangeProposal::new(
        ChangeProposalId::from_string("p-1"),
        wid("w-A"),
        ProposalKind::SourcePatch {
            patch_ref: "sha256:patch".to_string(),
        },
        RequestedBy::LlmAgent {
            agent_ref: "fix-agent".to_string(),
        },
    );

    let corpus = HistoricalCorpus::try_new(vec![
        case("a", vec![obs("oracle.a", true)]),
        case("b", vec![obs("oracle.b", true)]),
    ])
    .unwrap();
    let split = DatasetSplit::try_new(vec![case_id("a")], vec![case_id("b")]).unwrap();
    let replay = HistoricalReplayPlan::prepare(&corpus, &split).unwrap();

    let current_desc = AnalyzerDescriptor::try_new("analyzer", "rev-current").unwrap();
    let candidate_desc = AnalyzerDescriptor::try_new("analyzer", "rev-candidate").unwrap();
    let plan = ShadowEvaluationPlan::prepare(replay, current_desc.clone(), candidate_desc.clone());

    Harness {
        proposal,
        base,
        candidate,
        current,
        plan,
        current_desc,
        candidate_desc,
    }
}

fn shadow(
    h: &Harness,
    role: DatasetRole,
    current: &dyn HistoricalPredictor,
    candidate: &dyn HistoricalPredictor,
) -> ShadowRoleReport {
    let normaliser = DefaultNormaliser;
    ShadowEvaluator::evaluate_role(
        &h.plan,
        role,
        AnalyzerUnderTest::new(&h.current_desc, current),
        AnalyzerUnderTest::new(&h.candidate_desc, candidate),
        &BuiltinFailureRegimeClassifier::new(&normaliser),
    )
    .expect("shadow evaluation")
}

/// OPTIMIZE improves, CONFIRM regresses.
fn optimize_win_confirm_loss() -> (Harness, ShadowRoleReport, ShadowRoleReport) {
    let h = harness();
    let current = Scripted::new(
        "current",
        vec![("a", vec![]), ("b", vec![("oracle.b", true)])],
    );
    let candidate = Scripted::new(
        "candidate",
        vec![
            ("a", vec![("oracle.a", true)]),
            ("b", vec![("oracle.b", true), ("oracle.missing", true)]),
        ],
    );
    let optimize = shadow(&h, DatasetRole::Optimize, &current, &candidate);
    let confirm = shadow(&h, DatasetRole::Confirm, &current, &candidate);
    (h, optimize, confirm)
}

/// OPTIMIZE improves, CONFIRM clean.
fn optimize_win_confirm_clean() -> (Harness, ShadowRoleReport, ShadowRoleReport) {
    let h = harness();
    let current = Scripted::new(
        "current",
        vec![("a", vec![]), ("b", vec![("oracle.b", true)])],
    );
    let candidate = Scripted::new(
        "candidate",
        vec![
            ("a", vec![("oracle.a", true)]),
            ("b", vec![("oracle.b", true)]),
        ],
    );
    let optimize = shadow(&h, DatasetRole::Optimize, &current, &candidate);
    let confirm = shadow(&h, DatasetRole::Confirm, &current, &candidate);
    (h, optimize, confirm)
}

fn binding_for(h: &Harness) -> Result<GovernedCandidateBinding, GovernedBindingError> {
    GovernedCandidateBinding::bind(
        &h.proposal,
        &h.base,
        &h.candidate,
        h.current_desc.clone(),
        h.candidate_desc.clone(),
        h.plan.evaluation_digest(),
    )
}

fn trial_for(h: &Harness, world: &SoftwareWorld, base_snapshot: SnapshotId) -> TrialEvidence {
    let input = TrialInput {
        proposal: h.proposal.clone(),
        world: world.clone(),
        base_snapshot,
        candidate_snapshot: base_snapshot,
        candidate_facts: vec![],
        work_results: vec![],
        evidence_bundle: EvidenceBundle::new(
            EvidenceBundleId(1),
            WorkId::new(NamespacedName::new("ci.test").unwrap()),
            ExecutionId(1),
            SnapshotId(1),
            vec![],
        ),
    };
    assemble_trial_evidence(
        TrialId::from_string("t-1"),
        input,
        PolicyDecision {
            outcome: PolicyOutcome::Pass,
            reasons: vec![],
        },
    )
}

/// Build a real `PromotionPermit` for the harness, going through the full e80a
/// external-approval path.
fn promotion_permit(
    h: &Harness,
) -> crate::application::promotion_authority::permit::PromotionPermit {
    let dry_run = evaluate_promotion_lineage(
        GovernedPromotionInput {
            proposal: &h.proposal,
            base: &h.base,
            candidate: &h.candidate,
            current: &h.current,
        },
        Some(&trial_for(h, &h.candidate, h.base.base_snapshot)),
    );
    assert_eq!(dry_run.status, PromotionStatus::CleanPromotionReady);

    let target = PromotionApprovalTarget::from_dry_run(&dry_run);
    let approval: VerifiedExternalApproval = ExternalApprovalAuthority::verify(
        &ApproveTarget,
        ExternalApprovalRequest::new(target, ActorRef::human("alice")),
    )
    .expect("verified approval");
    let authorization =
        PromotionAuthorizationPolicy::authorize(&h.proposal, dry_run, Some(approval))
            .expect("automated author with verified approval is authorised");
    issue_promotion_permit(PromotionPermitId::from_string("pm-1"), authorization)
}

struct ApproveTarget;

impl ExternalApprovalVerifier for ApproveTarget {
    fn verify(&self, request: &ExternalApprovalRequest) -> bool {
        request.claimed_approver.kind == crate::domain::execution::actor::ActorKind::Human
    }
}

fn freeze_for(h: &Harness, optimize: &ShadowRoleReport) -> CandidateFreeze {
    CandidateFreeze::freeze(&binding_for(h).unwrap(), optimize).expect("freeze")
}

// ── WU12: OPTIMIZE win + CONFIRM loss ⇒ BLOCK everywhere ────────────

#[test]
fn wu12_optimize_win_confirm_loss_blocks_the_governed_path() {
    let (h, optimize, confirm) = optimize_win_confirm_loss();

    // OPTIMIZE really did improve.
    assert_eq!(optimize.aggregate_delta().score.true_positive, 1);
    assert_eq!(optimize.aggregate_delta().score.false_positive, -1);

    // Freeze succeeds, CONFIRM is evaluated ...
    let freeze = freeze_for(&h, &optimize);
    let policy = HeldOutPolicySpec::strict_no_regression();
    let decision = HeldOutPromotionGate::evaluate(&freeze, &confirm, &policy).unwrap();

    // ... and BLOCKS: CONFIRM regressed by one FN.
    assert_eq!(decision.outcome, PolicyOutcome::Block);
    assert!(decision.reasons.iter().any(|r| matches!(
        r,
        crate::application::governed_improvement::policy::HeldOutReason::FalseNegativeRegression {
            observed: 1,
            allowed: 0
        }
    )));

    // No held-out pass ⇒ no governed permit, despite everything else being green.
    let rejected = HeldOutPromotionGate::require_pass(&freeze, &confirm, &policy);
    assert!(matches!(rejected, Err(HeldOutRejection::Decision(_))));

    // Independently: trial PASS + CleanPromotionReady + verified human approval
    // still produce a PromotionPermit ...
    let permit = promotion_permit(&h);
    assert_eq!(permit.proposal, h.proposal.id);

    // ... but the only constructor of a governed permit requires a sealed
    // HeldOutGatePass. Since no pass can be minted here, the governed apply is
    // structurally impossible: OPTIMIZE cannot pay for CONFIRM regression.
    assert!(
        HeldOutPromotionGate::require_pass(&freeze, &confirm, &policy)
            .unwrap_err()
            .to_string()
            .contains("did not pass")
    );
}

// ── WU13: incomplete CONFIRM ⇒ InsufficientEvidence ─────────────────

#[test]
fn wu13_incomplete_confirm_is_insufficient_evidence() {
    let h = harness();
    let corpus = HistoricalCorpus::try_new(vec![
        case("a", vec![obs("oracle.a", true)]),
        case("b", vec![]), // no recorded observation on CONFIRM
    ])
    .unwrap();
    let split = DatasetSplit::try_new(vec![case_id("a")], vec![case_id("b")]).unwrap();
    let replay = HistoricalReplayPlan::prepare(&corpus, &split).unwrap();
    let plan =
        ShadowEvaluationPlan::prepare(replay, h.current_desc.clone(), h.candidate_desc.clone());
    let current = Scripted::new("current", vec![]);
    let candidate = Scripted::new("candidate", vec![("a", vec![("oracle.a", true)])]);
    let normaliser = DefaultNormaliser;
    let optimize = ShadowEvaluator::evaluate_role(
        &plan,
        DatasetRole::Optimize,
        AnalyzerUnderTest::new(&h.current_desc, &current),
        AnalyzerUnderTest::new(&h.candidate_desc, &candidate),
        &BuiltinFailureRegimeClassifier::new(&normaliser),
    )
    .unwrap();
    let confirm = ShadowEvaluator::evaluate_role(
        &plan,
        DatasetRole::Confirm,
        AnalyzerUnderTest::new(&h.current_desc, &current),
        AnalyzerUnderTest::new(&h.candidate_desc, &candidate),
        &BuiltinFailureRegimeClassifier::new(&normaliser),
    )
    .unwrap();

    let plan_digest = plan.evaluation_digest().to_string();
    let binding = GovernedCandidateBinding::bind(
        &h.proposal,
        &h.base,
        &h.candidate,
        h.current_desc.clone(),
        h.candidate_desc.clone(),
        plan_digest,
    )
    .unwrap();
    let freeze = CandidateFreeze::freeze(&binding, &optimize).unwrap();

    let policy = HeldOutPolicySpec::strict_no_regression();
    let decision = HeldOutPromotionGate::evaluate(&freeze, &confirm, &policy).unwrap();
    assert_eq!(
        decision.outcome,
        PolicyOutcome::InsufficientEvidence,
        "unknown evidence must never be read as zero regression"
    );
    assert!(HeldOutPromotionGate::require_pass(&freeze, &confirm, &policy).is_err());
}

// ── WU14: candidate swap ────────────────────────────────────────────

#[test]
fn wu14_a_confirm_for_a_different_revision_is_rejected_not_blocked() {
    let (h, optimize, _confirm) = optimize_win_confirm_clean();
    let freeze = freeze_for(&h, &optimize);

    // Build a CONFIRM report whose candidate revision differs from the freeze.
    let other_desc = AnalyzerDescriptor::try_new("analyzer", "rev-other").unwrap();
    let plan = ShadowEvaluationPlan::prepare(
        h.plan.replay_plan().clone(),
        h.current_desc.clone(),
        other_desc.clone(),
    );
    let current = Scripted::new("current", vec![("b", vec![("oracle.b", true)])]);
    let candidate = Scripted::new("candidate", vec![("b", vec![("oracle.b", true)])]);
    let normaliser = DefaultNormaliser;
    let swapped = ShadowEvaluator::evaluate_role(
        &plan,
        DatasetRole::Confirm,
        AnalyzerUnderTest::new(&h.current_desc, &current),
        AnalyzerUnderTest::new(&other_desc, &candidate),
        &BuiltinFailureRegimeClassifier::new(&normaliser),
    )
    .unwrap();

    let err = HeldOutPromotionGate::evaluate(
        &freeze,
        &swapped,
        &HeldOutPolicySpec::strict_no_regression(),
    );
    assert!(
        matches!(err, Err(HeldOutGateError::EvaluationDigestMismatch { .. })),
        "a different candidate revision changes the evaluation digest"
    );
}

#[test]
fn wu14_c_same_descriptor_different_evaluation_digest_is_rejected() {
    let (h, optimize, _confirm) = optimize_win_confirm_clean();
    let freeze = freeze_for(&h, &optimize);

    // Same descriptors, different corpus => different evaluation digest.
    let other_corpus =
        HistoricalCorpus::try_new(vec![case("z", vec![obs("oracle.z", true)])]).unwrap();
    let other_split = DatasetSplit::try_new(vec![], vec![case_id("z")]).unwrap();
    let other_replay = HistoricalReplayPlan::prepare(&other_corpus, &other_split).unwrap();
    let other_plan = ShadowEvaluationPlan::prepare(
        other_replay,
        h.current_desc.clone(),
        h.candidate_desc.clone(),
    );
    assert_ne!(other_plan.evaluation_digest(), freeze.evaluation_digest());

    let current = Scripted::new("current", vec![("z", vec![("oracle.z", true)])]);
    let candidate = Scripted::new("candidate", vec![("z", vec![("oracle.z", true)])]);
    let normaliser = DefaultNormaliser;
    let other_report = ShadowEvaluator::evaluate_role(
        &other_plan,
        DatasetRole::Confirm,
        AnalyzerUnderTest::new(&h.current_desc, &current),
        AnalyzerUnderTest::new(&h.candidate_desc, &candidate),
        &BuiltinFailureRegimeClassifier::new(&normaliser),
    )
    .unwrap();

    let err = HeldOutPromotionGate::evaluate(
        &freeze,
        &other_report,
        &HeldOutPolicySpec::strict_no_regression(),
    );
    assert!(matches!(
        err,
        Err(HeldOutGateError::EvaluationDigestMismatch { .. })
    ));
}

#[test]
fn wu14_d_a_candidate_world_outside_the_frozen_lineage_cannot_even_get_a_permit() {
    let (h, optimize, confirm) = optimize_win_confirm_clean();
    let freeze = freeze_for(&h, &optimize);
    let policy = HeldOutPolicySpec::strict_no_regression();
    let pass = HeldOutPromotionGate::require_pass(&freeze, &confirm, &policy).expect("pass");

    // A world outside the frozen lineage cannot produce a CleanPromotionReady
    // dry-run in the first place, so no permit for it can exist.
    let other_candidate = SoftwareWorld::new_base(wid("w-Z"), SnapshotId::new(10));
    let dry_run = evaluate_promotion_lineage(
        GovernedPromotionInput {
            proposal: &h.proposal,
            base: &h.base,
            candidate: &other_candidate,
            current: &h.current,
        },
        None,
    );
    assert!(matches!(
        dry_run.status,
        PromotionStatus::Blocked(PromotionBlockReason::CandidateNotDerivedFromBase { .. })
    ));

    // And the pass can only be paired with a permit for the SAME attempt.
    let permit = promotion_permit(&h);
    let governed = GovernedImprovementPermit::compose(pass, permit)
        .expect("the frozen pass and the frozen permit describe the same attempt");
    assert_eq!(governed.candidate_world(), &wid("w-B"));
}

#[test]
fn wu14_e_pass_for_one_proposal_cannot_pair_with_another_proposal() {
    let (h, optimize, confirm) = optimize_win_confirm_clean();
    let freeze = freeze_for(&h, &optimize);
    let policy = HeldOutPolicySpec::strict_no_regression();
    let pass = HeldOutPromotionGate::require_pass(&freeze, &confirm, &policy).expect("pass");

    // A permit for a different proposal.
    let mut other = harness();
    other.proposal = ChangeProposal::new(
        ChangeProposalId::from_string("p-2"),
        wid("w-A"),
        ProposalKind::SourcePatch {
            patch_ref: "sha256:other".to_string(),
        },
        RequestedBy::LlmAgent {
            agent_ref: "fix-agent".to_string(),
        },
    );
    let other_permit = promotion_permit(&other);

    let err = GovernedImprovementPermit::compose(pass, other_permit).unwrap_err();
    assert!(matches!(err, GovernedPermitError::ProposalMismatch { .. }));
}

// ── WU15: trial / promotion lineage ─────────────────────────────────

#[test]
fn wu15_a_trial_for_a_different_world_is_blocked() {
    let h = harness();
    // The trial measured a different world (current), not the candidate.
    let trial = trial_for(&h, &h.current, h.base.base_snapshot);
    let run = evaluate_promotion_lineage(
        GovernedPromotionInput {
            proposal: &h.proposal,
            base: &h.base,
            candidate: &h.candidate,
            current: &h.current,
        },
        Some(&trial),
    );
    assert!(
        matches!(
            run.status,
            PromotionStatus::Blocked(PromotionBlockReason::TrialWorldMismatch { .. })
        ),
        "a passing trial must not be reusable against another candidate world"
    );
}

#[test]
fn wu15_b_trial_with_a_different_base_snapshot_is_blocked() {
    let h = harness();
    let trial = trial_for(&h, &h.candidate, SnapshotId::new(999));
    let run = evaluate_promotion_lineage(
        GovernedPromotionInput {
            proposal: &h.proposal,
            base: &h.base,
            candidate: &h.candidate,
            current: &h.current,
        },
        Some(&trial),
    );
    assert!(matches!(
        run.status,
        PromotionStatus::Blocked(PromotionBlockReason::TrialBaseSnapshotMismatch { .. })
    ));
}

#[test]
fn wu15_c_proposal_for_a_different_base_world_is_blocked() {
    let h = harness();
    let mut proposal = h.proposal.clone();
    proposal.base_world = wid("w-OTHER");
    let trial = trial_for(&h, &h.candidate, h.base.base_snapshot);
    let run = evaluate_promotion_lineage(
        GovernedPromotionInput {
            proposal: &proposal,
            base: &h.base,
            candidate: &h.candidate,
            current: &h.current,
        },
        Some(&trial),
    );
    assert!(matches!(
        run.status,
        PromotionStatus::Blocked(PromotionBlockReason::ProposalBaseWorldMismatch { .. })
    ));
}

#[test]
fn wu15_d_candidate_not_derived_from_base_is_blocked() {
    let h = harness();
    let orphan = SoftwareWorld::new_base(wid("w-ORPHAN"), SnapshotId::new(10));
    let trial = trial_for(&h, &orphan, h.base.base_snapshot);
    let run = evaluate_promotion_lineage(
        GovernedPromotionInput {
            proposal: &h.proposal,
            base: &h.base,
            candidate: &orphan,
            current: &h.current,
        },
        Some(&trial),
    );
    assert!(matches!(
        run.status,
        PromotionStatus::Blocked(PromotionBlockReason::CandidateNotDerivedFromBase { .. })
    ));
}

#[test]
fn wu15_e_valid_lineage_preserves_the_clean_path() {
    let h = harness();
    let trial = trial_for(&h, &h.candidate, h.base.base_snapshot);
    let run = evaluate_promotion_lineage(
        GovernedPromotionInput {
            proposal: &h.proposal,
            base: &h.base,
            candidate: &h.candidate,
            current: &h.current,
        },
        Some(&trial),
    );
    assert_eq!(run.status, PromotionStatus::CleanPromotionReady);
    assert!(run.lineage.base_matches_current);
}

#[test]
fn wu15_f_world_drift_after_evaluation_is_still_conflict() {
    let h = harness();
    let drifted = SoftwareWorld::new_base(wid("w-C"), SnapshotId::new(12));
    let trial = trial_for(&h, &h.candidate, h.base.base_snapshot);
    let run = evaluate_promotion_lineage(
        GovernedPromotionInput {
            proposal: &h.proposal,
            base: &h.base,
            candidate: &h.candidate,
            current: &drifted,
        },
        Some(&trial),
    );
    assert_eq!(run.status, PromotionStatus::ConflictRequiresReevaluation);
}

// ── WU20: policy matrix ─────────────────────────────────────────────

#[test]
fn wu20_b_fn_delta_equal_to_tolerance_is_allowed() {
    let (h, optimize, confirm) = optimize_win_confirm_loss();
    let freeze = freeze_for(&h, &optimize);
    // FN delta is +1; allow exactly 1 FP and 1 FN.
    let policy = HeldOutPolicySpec::new(1, 1, 0, true, Vec::new());
    let decision = HeldOutPromotionGate::evaluate(&freeze, &confirm, &policy).unwrap();
    assert_eq!(decision.outcome, PolicyOutcome::Pass);
    assert!(HeldOutPromotionGate::require_pass(&freeze, &confirm, &policy).is_ok());
}

#[test]
fn wu20_g_huge_optimize_improvement_does_not_rescue_a_confirm_block() {
    // The gate never sees OPTIMIZE; a large OPTIMIZE win is irrelevant.
    let (h, optimize, confirm) = optimize_win_confirm_loss();
    let freeze = freeze_for(&h, &optimize);
    let strict = HeldOutPolicySpec::strict_no_regression();
    let decision = HeldOutPromotionGate::evaluate(&freeze, &confirm, &strict).unwrap();
    assert_eq!(decision.outcome, PolicyOutcome::Block);
}

#[test]
fn wu20_h_clean_confirm_passes_even_when_optimize_is_poor() {
    let h = harness();
    // OPTIMIZE: candidate is worse (introduces an FN on case a).
    let current = Scripted::new("current", vec![("a", vec![("oracle.a", true)])]);
    let candidate = Scripted::new("candidate", vec![("a", vec![])]);
    let optimize = shadow(&h, DatasetRole::Optimize, &current, &candidate);
    // CONFIRM: identical analyzers => no regression.
    let confirm = shadow(&h, DatasetRole::Confirm, &current, &current);

    let freeze = freeze_for(&h, &optimize);
    let policy = HeldOutPolicySpec::strict_no_regression();
    let decision = HeldOutPromotionGate::evaluate(&freeze, &confirm, &policy).unwrap();
    assert_eq!(
        decision.outcome,
        PolicyOutcome::Pass,
        "the held-out gate answers CONFIRM only; candidate selection is out of scope"
    );
}

#[test]
fn wu20_i_j_determinism_and_policy_digest_sensitivity() {
    let (h, optimize, confirm) = optimize_win_confirm_clean();
    let freeze = freeze_for(&h, &optimize);
    let a = HeldOutPolicySpec::strict_no_regression();
    let d1 = HeldOutPromotionGate::evaluate(&freeze, &confirm, &a).unwrap();
    let d2 = HeldOutPromotionGate::evaluate(&freeze, &confirm, &a).unwrap();
    assert_eq!(d1, d2, "same inputs => identical decision");

    let b = HeldOutPolicySpec::new(1, 0, 0, true, Vec::new());
    assert_ne!(
        a.digest(),
        b.digest(),
        "changed tolerance => changed digest"
    );
    let d3 = HeldOutPromotionGate::evaluate(&freeze, &confirm, &b).unwrap();
    assert_ne!(d1.policy_digest, d3.policy_digest);
}

// ── WU21: authority matrix ──────────────────────────────────────────

#[test]
fn wu21_a_heldout_pass_plus_trial_pass_but_no_approval_yields_no_permit() {
    let (h, optimize, confirm) = optimize_win_confirm_clean();
    let freeze = freeze_for(&h, &optimize);
    let policy = HeldOutPolicySpec::strict_no_regression();
    assert!(HeldOutPromotionGate::require_pass(&freeze, &confirm, &policy).is_ok());

    // Trial PASS and CleanPromotionReady ...
    let dry_run = evaluate_promotion_lineage(
        GovernedPromotionInput {
            proposal: &h.proposal,
            base: &h.base,
            candidate: &h.candidate,
            current: &h.current,
        },
        Some(&trial_for(&h, &h.candidate, h.base.base_snapshot)),
    );
    assert_eq!(dry_run.status, PromotionStatus::CleanPromotionReady);

    // ... but an LlmAgent author with NO verified external approval gets no permit.
    let err = PromotionAuthorizationPolicy::authorize(&h.proposal, dry_run, None).unwrap_err();
    assert!(matches!(
        err,
        crate::application::promotion_authority::authorization::PromotionAuthorizationError::AutomatedAuthorRequiresExternalApproval { .. }
    ));
}

#[test]
fn wu21_h_world_drift_after_permit_is_rejected_by_the_governed_apply() {
    let (h, optimize, confirm) = optimize_win_confirm_clean();
    let freeze = freeze_for(&h, &optimize);
    let policy = HeldOutPolicySpec::strict_no_regression();
    let pass = HeldOutPromotionGate::require_pass(&freeze, &confirm, &policy).unwrap();
    let permit = promotion_permit(&h);
    let governed = GovernedImprovementPermit::compose(pass, permit).unwrap();

    let drifted = SoftwareWorld::new_base(wid("w-C"), SnapshotId::new(12));
    assert!(matches!(
        apply_governed_improvement(&drifted, &governed),
        PromotionApplyOutcome::Rejected(_)
    ));
    // A rejected apply produces no success receipt.
    let outcome = apply_governed_improvement(&drifted, &governed);
    assert!(GovernedImprovementReceipt::record(&governed, &outcome, None, None).is_none());
}

// ── WU16 / WU18: positive governed improvement E2E ──────────────────

#[test]
fn wu16_wu18_governed_improvement_end_to_end_with_receipt() {
    let (h, optimize, confirm) = optimize_win_confirm_clean();

    // 1. OPTIMIZE shows a real improvement.
    assert_eq!(optimize.aggregate_delta().score.true_positive, 1);

    // 2. Bind and freeze.
    let binding = binding_for(&h).expect("binding");
    assert_eq!(binding.proposal_id(), &h.proposal.id);
    assert_eq!(binding.candidate_world(), &h.candidate.id);
    let freeze = CandidateFreeze::freeze(&binding, &optimize).expect("freeze");
    assert_eq!(freeze.evaluation_digest(), h.plan.evaluation_digest());

    // 3. CONFIRM matches the freeze and passes policy.
    let policy = HeldOutPolicySpec::strict_no_regression();
    let decision = HeldOutPromotionGate::evaluate(&freeze, &confirm, &policy).unwrap();
    assert_eq!(decision.outcome, PolicyOutcome::Pass);
    let pass = HeldOutPromotionGate::require_pass(&freeze, &confirm, &policy).expect("pass");

    // 4. The real trial passes and the lineage is clean.
    let trial = trial_for(&h, &h.candidate, h.base.base_snapshot);
    let dry_run = evaluate_promotion_lineage(
        GovernedPromotionInput {
            proposal: &h.proposal,
            base: &h.base,
            candidate: &h.candidate,
            current: &h.current,
        },
        Some(&trial),
    );
    assert_eq!(dry_run.status, PromotionStatus::CleanPromotionReady);

    // 5. Verified external human approval (an automated author requires it).
    let target = PromotionApprovalTarget::from_dry_run(&dry_run);
    let approval = ExternalApprovalAuthority::verify(
        &ApproveTarget,
        ExternalApprovalRequest::new(target, ActorRef::human("alice")),
    )
    .expect("verified approval");
    let authorization =
        PromotionAuthorizationPolicy::authorize(&h.proposal, dry_run, Some(approval))
            .expect("authorised");
    let permit = issue_promotion_permit(PromotionPermitId::from_string("pm-1"), authorization);

    // 6. Compose the governed permit and apply.
    let governed = GovernedImprovementPermit::compose(pass, permit).expect("governed permit");
    assert_eq!(governed.proposal_id(), &h.proposal.id);
    assert_eq!(governed.candidate_world(), &h.candidate.id);

    let outcome = apply_governed_improvement(&h.current, &governed);
    assert!(
        matches!(outcome, PromotionApplyOutcome::Applied { .. }),
        "the governed path applies against the matching world"
    );

    // 7. Audit receipt.
    let receipt = GovernedImprovementReceipt::record(
        &governed,
        &outcome,
        Some("t-1".to_string()),
        Some("pass".to_string()),
    )
    .expect("receipt for an applied improvement");
    assert_eq!(receipt.proposal_id, "p-1");
    assert_eq!(receipt.candidate_world, "w-B");
    assert_eq!(receipt.current_analyzer, "analyzer@rev-current");
    assert_eq!(receipt.candidate_analyzer, "analyzer@rev-candidate");
    assert_eq!(receipt.evaluation_digest, h.plan.evaluation_digest());
    assert_eq!(receipt.heldout_policy_digest, policy.digest());
    assert_eq!(receipt.external_approver.as_deref(), Some("alice"));
    assert_eq!(receipt.promotion_target, "w-A -> w-B");
    assert_eq!(receipt.applied_to_snapshot, 10);

    // The receipt is serializable audit data.
    let json = serde_json::to_string(&receipt).expect("serializable");
    assert!(json.contains("\"proposal_id\":\"p-1\""));
}

// ── binding validation ──────────────────────────────────────────────

#[test]
fn binding_rejects_a_proposal_for_another_base_world() {
    let h = harness();
    let mut proposal = h.proposal.clone();
    proposal.base_world = wid("w-OTHER");
    let err = GovernedCandidateBinding::bind(
        &proposal,
        &h.base,
        &h.candidate,
        h.current_desc.clone(),
        h.candidate_desc.clone(),
        h.plan.evaluation_digest(),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        GovernedBindingError::ProposalBaseWorldMismatch { .. }
    ));
}

#[test]
fn binding_rejects_a_candidate_not_derived_from_the_base() {
    let h = harness();
    let orphan = SoftwareWorld::new_base(wid("w-ORPHAN"), SnapshotId::new(10));
    let err = GovernedCandidateBinding::bind(
        &h.proposal,
        &h.base,
        &orphan,
        h.current_desc.clone(),
        h.candidate_desc.clone(),
        h.plan.evaluation_digest(),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        GovernedBindingError::CandidateNotDerivedFromBase { .. }
    ));
}

#[test]
fn freeze_rejects_a_non_optimize_report() {
    let (h, _optimize, confirm) = optimize_win_confirm_clean();
    let binding = binding_for(&h).unwrap();
    let err = CandidateFreeze::freeze(&binding, &confirm).unwrap_err();
    assert!(matches!(
        err,
        crate::application::governed_improvement::binding::FreezeError::WrongDatasetRole {
            expected: DatasetRole::Optimize,
            ..
        }
    ));
}

#[test]
fn gate_rejects_a_non_confirm_report() {
    let (h, optimize, _confirm) = optimize_win_confirm_clean();
    let freeze = freeze_for(&h, &optimize);
    let err = HeldOutPromotionGate::evaluate(
        &freeze,
        &optimize,
        &HeldOutPolicySpec::strict_no_regression(),
    );
    assert!(matches!(
        err,
        Err(HeldOutGateError::WrongDatasetRole {
            expected: DatasetRole::Confirm,
            ..
        })
    ));
}
