//! e82 WU18 — adversarial shadow-evaluation tests + WU22 vertical UAT.
//!
//! Invariant under test:
//!
//! ```text
//! candidate improves something  !=  candidate may be promoted
//! ```

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use crate::application::historical_replay::case::{
    ContentRef, HistoricalCase, HistoricalCaseId, HistoricalPredictionInput,
};
use crate::application::historical_replay::corpus::HistoricalCorpus;
use crate::application::historical_replay::plan::HistoricalReplayPlan;
use crate::application::historical_replay::replay::{
    HistoricalPredictor, ReplayCaseOutcome, ReplayCaseResult, ReplayIncomplete,
};
use crate::application::historical_replay::report::ReplayReport;
use crate::application::historical_replay::split::{DatasetRole, DatasetSplit};
use crate::application::self_hosting::platform_equivalence::{
    DefaultNormaliser, HistoricalReplay, PlatformKind, PlatformObservation,
};
use crate::application::self_hosting::prediction::{
    ExpectedObservation, ScoreMatrix, SealedPrediction,
};
use crate::application::shadow_evaluation::analyzer::{
    AnalyzerDescriptor, AnalyzerSide, AnalyzerUnderTest,
};
use crate::application::shadow_evaluation::classifiers::BuiltinFailureRegimeClassifier;
use crate::application::shadow_evaluation::compare::{ShadowError, pair_case_results};
use crate::application::shadow_evaluation::plan::ShadowEvaluationPlan;
use crate::application::shadow_evaluation::regime::{
    FailureRegime, FailureRegimeClassifier, FailureRegimeContext, FailureRegimeEvidence,
    FailureRegimeOccurrence, FailureRegimeSubject,
};
use crate::application::shadow_evaluation::report::ShadowEvaluator;

// --- fixtures -------------------------------------------------------

fn case_id(s: &str) -> HistoricalCaseId {
    HistoricalCaseId::try_new(s).expect("non-empty case id")
}

fn obs(id: &str, fired: bool) -> PlatformObservation {
    obs_raw(
        PlatformKind::Linux,
        id,
        if fired { b"present" } else { b"" },
    )
}

fn obs_raw(platform: PlatformKind, id: &str, raw: &[u8]) -> PlatformObservation {
    PlatformObservation {
        platform,
        id: id.to_string(),
        raw: raw.to_vec(),
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

/// A deterministic analyzer scripted per case id.
#[derive(Default)]
struct ScriptedAnalyzer {
    label: String,
    script: Vec<(String, Vec<(String, bool)>)>,
    seen: RefCell<Vec<HistoricalPredictionInput>>,
}

impl ScriptedAnalyzer {
    fn new(label: &str, script: Vec<(&str, Vec<(&str, bool)>)>) -> Self {
        Self {
            label: label.to_string(),
            script: script
                .into_iter()
                .map(|(id, expectations)| {
                    (
                        id.to_string(),
                        expectations
                            .into_iter()
                            .map(|(o, p)| (o.to_string(), p))
                            .collect(),
                    )
                })
                .collect(),
            seen: RefCell::new(Vec::new()),
        }
    }

    fn calls(&self) -> usize {
        self.seen.borrow().len()
    }
}

impl HistoricalPredictor for ScriptedAnalyzer {
    fn predict(&self, input: &HistoricalPredictionInput) -> SealedPrediction {
        self.seen.borrow_mut().push(input.clone());
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

fn descriptor(label: &str, revision: &str) -> AnalyzerDescriptor {
    AnalyzerDescriptor::try_new(label, revision).expect("valid descriptor")
}

fn plan_for(
    cases: Vec<HistoricalCase>,
    optimize: &[&str],
    confirm: &[&str],
) -> ShadowEvaluationPlan {
    let corpus = HistoricalCorpus::try_new(cases).expect("valid corpus");
    let split = DatasetSplit::try_new(
        optimize.iter().map(|id| case_id(id)).collect(),
        confirm.iter().map(|id| case_id(id)).collect(),
    )
    .expect("valid split");
    let replay = HistoricalReplayPlan::prepare(&corpus, &split).expect("valid plan");
    ShadowEvaluationPlan::prepare(
        replay,
        descriptor("current", "rev-current"),
        descriptor("candidate", "rev-candidate"),
    )
}

fn builtin(
    normaliser: &DefaultNormaliser,
) -> BuiltinFailureRegimeClassifier<'_, DefaultNormaliser> {
    BuiltinFailureRegimeClassifier::new(normaliser)
}

/// Build a report from hand-crafted case results (comparison-level tests).
fn report(role: DatasetRole, cases: Vec<(&str, ReplayCaseOutcome)>) -> ReplayReport {
    ReplayReport::from_cases(
        role,
        cases
            .into_iter()
            .map(|(id, outcome)| ReplayCaseResult {
                case_id: case_id(id),
                role,
                prediction_seal: format!("seal-{id}"),
                outcome,
            })
            .collect(),
    )
}

fn scored(tp: usize, fp: usize, fneg: usize, tn: usize) -> ReplayCaseOutcome {
    ReplayCaseOutcome::Scored(ScoreMatrix {
        true_positive: tp,
        false_positive: fp,
        false_negative: fneg,
        true_negative: tn,
        unknown: 0,
    })
}

fn incomplete() -> ReplayCaseOutcome {
    ReplayCaseOutcome::Incomplete(ReplayIncomplete::NoRecordedObservation)
}

// --- A: control experiment ------------------------------------------

#[test]
fn a_same_analyzer_against_itself_yields_zero_deltas() {
    let normaliser = DefaultNormaliser;
    let plan = plan_for(
        vec![
            case("a", vec![obs("oracle.a", true)]),
            case("b", vec![obs("oracle.b", true)]),
        ],
        &["a"],
        &["b"],
    );
    let script = vec![
        ("a", vec![("oracle.a", true)]),
        ("b", vec![("oracle.b", true)]),
    ];
    let left = ScriptedAnalyzer::new("same", script.clone());
    let right = ScriptedAnalyzer::new("same", script);

    let current = AnalyzerUnderTest::new(plan.current(), &left);
    let candidate = AnalyzerUnderTest::new(plan.candidate(), &right);
    let evaluation =
        ShadowEvaluator::evaluate_all(&plan, current, candidate, &builtin(&normaliser))
            .expect("shadow evaluation");

    for report in [evaluation.optimize(), evaluation.confirm()] {
        assert!(
            report.aggregate_delta().score.is_zero(),
            "control must yield zero deltas"
        );
        assert_eq!(report.aggregate_delta().scored_cases, 0);
        assert_eq!(report.aggregate_delta().incomplete_cases, 0);
        assert!(report.failure_regimes().is_empty());
    }
}

// --- B / C: raw deltas, no verdicts ---------------------------------

#[test]
fn b_candidate_gaining_a_true_positive_shows_a_raw_delta_only() {
    let normaliser = DefaultNormaliser;
    let plan = plan_for(vec![case("a", vec![obs("oracle.a", true)])], &["a"], &[]);
    // e76 score() semantics (characterized): an observation that fires counts as
    // a TP when it appears in the prediction's expected set, and as an FP when it
    // does not. So the difference here is presence in the expected set.
    let current = ScriptedAnalyzer::new("current", vec![]);
    let candidate = ScriptedAnalyzer::new("candidate", vec![("a", vec![("oracle.a", true)])]);

    let report = ShadowEvaluator::evaluate_role(
        &plan,
        DatasetRole::Optimize,
        AnalyzerUnderTest::new(plan.current(), &current),
        AnalyzerUnderTest::new(plan.candidate(), &candidate),
        &builtin(&normaliser),
    )
    .expect("shadow evaluation");

    let delta = report.aggregate_delta().score;
    assert_eq!(
        delta.true_positive, 1,
        "raw TP delta must reflect the change"
    );
    assert_eq!(delta.false_positive, -1);
    assert!(!delta.is_zero());
}

#[test]
fn c_candidate_gaining_a_false_negative_shows_a_raw_delta_only() {
    let normaliser = DefaultNormaliser;
    let plan = plan_for(vec![case("b", vec![obs("oracle.b", true)])], &[], &["b"]);
    let current = ScriptedAnalyzer::new("current", vec![("b", vec![("oracle.b", true)])]);
    // Candidate additionally expects an observation that never fired.
    let candidate = ScriptedAnalyzer::new(
        "candidate",
        vec![("b", vec![("oracle.b", true), ("oracle.missing", true)])],
    );

    let report = ShadowEvaluator::evaluate_role(
        &plan,
        DatasetRole::Confirm,
        AnalyzerUnderTest::new(plan.current(), &current),
        AnalyzerUnderTest::new(plan.candidate(), &candidate),
        &builtin(&normaliser),
    )
    .expect("shadow evaluation");

    assert_eq!(report.aggregate_delta().score.false_negative, 1);
}

// --- D: roles stay independent --------------------------------------

#[test]
fn d_favourable_optimize_and_adverse_confirm_are_both_preserved() {
    let normaliser = DefaultNormaliser;
    let plan = plan_for(
        vec![
            case("a", vec![obs("oracle.a", true)]),
            case("b", vec![obs("oracle.b", true)]),
        ],
        &["a"],
        &["b"],
    );
    let current = ScriptedAnalyzer::new(
        "current",
        vec![("a", vec![]), ("b", vec![("oracle.b", true)])],
    );
    let candidate = ScriptedAnalyzer::new(
        "candidate",
        vec![
            ("a", vec![("oracle.a", true)]),
            ("b", vec![("oracle.b", true), ("oracle.missing", true)]),
        ],
    );

    let evaluation = ShadowEvaluator::evaluate_all(
        &plan,
        AnalyzerUnderTest::new(plan.current(), &current),
        AnalyzerUnderTest::new(plan.candidate(), &candidate),
        &builtin(&normaliser),
    )
    .expect("shadow evaluation");

    // OPTIMIZE looks favourable ...
    assert_eq!(
        evaluation.optimize().aggregate_delta().score.true_positive,
        1
    );
    // ... while CONFIRM shows an adverse delta. Both survive, unblended.
    assert_eq!(
        evaluation.confirm().aggregate_delta().score.false_negative,
        1
    );
    assert_ne!(
        evaluation.optimize().aggregate_delta(),
        evaluation.confirm().aggregate_delta()
    );
    assert_eq!(evaluation.optimize().role(), DatasetRole::Optimize);
    assert_eq!(evaluation.confirm().role(), DatasetRole::Confirm);
}

// --- E / F / G: incomplete stays explicit ---------------------------

#[test]
fn e_candidate_incomplete_produces_a_regime_and_no_fake_score() {
    let current = report(DatasetRole::Optimize, vec![("a", scored(1, 0, 0, 0))]);
    let candidate = report(DatasetRole::Optimize, vec![("a", incomplete())]);
    let comparisons = pair_case_results(&current, &candidate, DatasetRole::Optimize).unwrap();
    assert_eq!(comparisons.len(), 1);
    assert!(
        comparisons[0].score_delta.is_none(),
        "an incomplete side must not be projected to zeros"
    );

    let case = case("a", vec![obs("oracle.a", true)]);
    let normaliser = DefaultNormaliser;
    let context = FailureRegimeContext {
        case: &case,
        role: DatasetRole::Optimize,
        current: &comparisons[0].current,
        candidate: &comparisons[0].candidate,
    };
    let regimes = builtin(&normaliser).classify(&context);
    assert!(regimes.iter().any(|r| {
        r.regime == FailureRegime::EvidenceIncomplete
            && r.subject == FailureRegimeSubject::Candidate
            && r.evidence == FailureRegimeEvidence::IncompleteObservation
    }));
    // Only the candidate side is incomplete here.
    assert_eq!(
        regimes
            .iter()
            .filter(|r| r.regime == FailureRegime::EvidenceIncomplete)
            .count(),
        1
    );
}

#[test]
fn f_current_incomplete_candidate_scored_is_an_explicit_asymmetry() {
    let current = report(DatasetRole::Confirm, vec![("b", incomplete())]);
    let candidate = report(DatasetRole::Confirm, vec![("b", scored(1, 0, 0, 0))]);
    let comparisons = pair_case_results(&current, &candidate, DatasetRole::Confirm).unwrap();
    assert!(comparisons[0].score_delta.is_none());
    assert!(comparisons[0].candidate.score().is_some());
    assert!(comparisons[0].current.score().is_none());
}

#[test]
fn g_both_incomplete_is_explicit_and_not_equal_to_safe() {
    let current = report(DatasetRole::Optimize, vec![("c", incomplete())]);
    let candidate = report(DatasetRole::Optimize, vec![("c", incomplete())]);
    let comparisons = pair_case_results(&current, &candidate, DatasetRole::Optimize).unwrap();
    assert!(comparisons[0].score_delta.is_none());
    assert_eq!(current.aggregate().true_negative, 0);
    assert_eq!(candidate.aggregate().true_negative, 0);
    assert_eq!(current.scored_cases(), 0);
    assert_eq!(current.incomplete_cases(), 1);
}

// --- H / I / J: pairing is by case id -------------------------------

#[test]
fn h_reordered_case_results_still_pair_by_case_id() {
    let current = report(
        DatasetRole::Optimize,
        vec![("a", scored(1, 0, 0, 0)), ("b", scored(0, 1, 0, 0))],
    );
    let candidate_ordered = report(
        DatasetRole::Optimize,
        vec![("a", scored(0, 1, 0, 0)), ("b", scored(1, 0, 0, 0))],
    );
    let candidate_reversed = report(
        DatasetRole::Optimize,
        vec![("b", scored(1, 0, 0, 0)), ("a", scored(0, 1, 0, 0))],
    );

    let ordered = pair_case_results(&current, &candidate_ordered, DatasetRole::Optimize).unwrap();
    let reversed = pair_case_results(&current, &candidate_reversed, DatasetRole::Optimize).unwrap();

    let project =
        |cs: &[crate::application::shadow_evaluation::compare::ShadowCaseComparison]| {
            cs.iter()
                .map(|c| (c.case_id.clone(), c.score_delta))
                .collect::<Vec<_>>()
        };
    assert_eq!(
        project(&ordered),
        project(&reversed),
        "pairing must be order-independent"
    );
}

#[test]
fn i_missing_case_on_one_side_fails_loud() {
    let current = report(
        DatasetRole::Optimize,
        vec![("a", scored(1, 0, 0, 0)), ("b", scored(1, 0, 0, 0))],
    );
    let candidate = report(DatasetRole::Optimize, vec![("a", scored(1, 0, 0, 0))]);
    let err = pair_case_results(&current, &candidate, DatasetRole::Optimize).unwrap_err();
    assert!(matches!(
        err,
        ShadowError::MissingCase {
            side: AnalyzerSide::Candidate,
            ..
        }
    ));

    let err2 = pair_case_results(&candidate, &current, DatasetRole::Optimize).unwrap_err();
    assert!(matches!(
        err2,
        ShadowError::MissingCase {
            side: AnalyzerSide::Current,
            ..
        }
    ));
}

#[test]
fn j_role_mismatch_fails_loud() {
    let current = report(DatasetRole::Confirm, vec![("a", scored(1, 0, 0, 0))]);
    let candidate = report(DatasetRole::Confirm, vec![("a", scored(1, 0, 0, 0))]);
    let err = pair_case_results(&current, &candidate, DatasetRole::Optimize).unwrap_err();
    assert!(matches!(
        err,
        ShadowError::RoleMismatch {
            side: AnalyzerSide::Current,
            ..
        }
    ));
}

// --- K / L: evaluation digest ---------------------------------------

#[test]
fn k_identical_inputs_yield_a_deterministic_evaluation_digest() {
    let cases = || vec![case("a", vec![obs("oracle.a", true)])];
    let first = plan_for(cases(), &["a"], &[]);
    let second = plan_for(cases(), &["a"], &[]);
    assert_eq!(first.evaluation_digest(), second.evaluation_digest());
}

#[test]
fn l_changed_candidate_revision_changes_the_evaluation_digest() {
    let corpus = HistoricalCorpus::try_new(vec![case("a", vec![obs("oracle.a", true)])]).unwrap();
    let split = DatasetSplit::try_new(vec![case_id("a")], vec![]).unwrap();
    let replay = HistoricalReplayPlan::prepare(&corpus, &split).unwrap();

    let first = ShadowEvaluationPlan::prepare(
        replay.clone(),
        descriptor("current", "rev-1"),
        descriptor("candidate", "rev-1"),
    );
    let second = ShadowEvaluationPlan::prepare(
        replay,
        descriptor("current", "rev-1"),
        descriptor("candidate", "rev-2"),
    );
    assert_ne!(first.evaluation_digest(), second.evaluation_digest());
}

// --- M / N: execution isolation -------------------------------------

#[test]
fn m_n_both_sides_receive_independently_constructed_identical_base_input() {
    let normaliser = DefaultNormaliser;
    let plan = plan_for(vec![case("a", vec![obs("oracle.a", true)])], &["a"], &[]);
    let current = ScriptedAnalyzer::new("current", vec![("a", vec![("oracle.a", true)])]);
    let candidate = ScriptedAnalyzer::new("candidate", vec![("a", vec![("oracle.a", true)])]);

    let _ = ShadowEvaluator::evaluate_role(
        &plan,
        DatasetRole::Optimize,
        AnalyzerUnderTest::new(plan.current(), &current),
        AnalyzerUnderTest::new(plan.candidate(), &candidate),
        &builtin(&normaliser),
    )
    .expect("shadow evaluation");

    let current_seen = current.seen.borrow();
    let candidate_seen = candidate.seen.borrow();
    assert_eq!(current.calls(), 1);
    assert_eq!(candidate.calls(), 1);
    // Both sides saw the same base-side input for the same case.
    assert_eq!(current_seen[0], candidate_seen[0]);
    assert_eq!(current_seen[0].case_id, case_id("a"));
    // The input type has no field that could carry the other side's output.
    let HistoricalPredictionInput {
        case_id: _,
        base_snapshot: _,
        base_source_ref: _,
    } = &current_seen[0];
}

// --- O: no regime by string parsing ---------------------------------

#[test]
fn o_observation_strings_cannot_forge_an_architecture_miss() {
    let case = case(
        "a",
        vec![obs_raw(
            PlatformKind::Linux,
            "architecture.boundary.violated",
            b"x",
        )],
    );
    let normaliser = DefaultNormaliser;
    let context = FailureRegimeContext {
        case: &case,
        role: DatasetRole::Optimize,
        current: &scored(1, 0, 0, 0),
        candidate: &scored(1, 0, 0, 0),
    };
    let regimes = builtin(&normaliser).classify(&context);
    assert!(
        regimes
            .iter()
            .all(|r| r.regime != FailureRegime::ArchitectureMiss),
        "an observation id must never be parsed into a regime"
    );
}

// --- P: typed incomplete source -------------------------------------

#[test]
fn p_typed_incomplete_source_is_classified() {
    let case = case("a", vec![]);
    let normaliser = DefaultNormaliser;
    let context = FailureRegimeContext {
        case: &case,
        role: DatasetRole::Confirm,
        current: &incomplete(),
        candidate: &incomplete(),
    };
    let regimes = builtin(&normaliser).classify(&context);
    assert_eq!(
        regimes
            .iter()
            .filter(|r| r.regime == FailureRegime::EvidenceIncomplete)
            .count(),
        2,
        "both sides are incomplete and each must be reported"
    );
    assert!(
        regimes
            .iter()
            .all(|r| r.evidence == FailureRegimeEvidence::IncompleteObservation)
    );
}

// --- Q: real platform divergence ------------------------------------

#[test]
fn q_real_platform_divergence_is_classified() {
    let divergent = case(
        "d",
        vec![
            obs_raw(PlatformKind::MacOs, "oracle.d", b"value-mac"),
            obs_raw(PlatformKind::Windows, "oracle.d", b"value-windows"),
        ],
    );
    let normaliser = DefaultNormaliser;
    let context = FailureRegimeContext {
        case: &divergent,
        role: DatasetRole::Optimize,
        current: &scored(1, 0, 0, 0),
        candidate: &scored(1, 0, 0, 0),
    };
    let regimes = builtin(&normaliser).classify(&context);
    assert!(regimes.iter().any(|r| {
        r.regime == FailureRegime::PlatformDivergence
            && r.subject == FailureRegimeSubject::SharedEvaluation
            && matches!(&r.evidence, FailureRegimeEvidence::PlatformDivergence { observation_id, .. } if observation_id == "oracle.d")
    }));

    // A single-platform case is NEVER divergence.
    let single = case("s", vec![obs("oracle.s", true)]);
    let context_single = FailureRegimeContext {
        case: &single,
        role: DatasetRole::Optimize,
        current: &scored(1, 0, 0, 0),
        candidate: &scored(1, 0, 0, 0),
    };
    let regimes_single = builtin(&normaliser).classify(&context_single);
    assert!(
        regimes_single
            .iter()
            .all(|r| r.regime != FailureRegime::PlatformDivergence)
    );
}

// --- classifier seam for the four non-built-in regimes --------------

/// A deterministic adapter demonstrating the documented seam for regimes whose
/// subsystems do not yet expose typed per-case signals.
struct TableClassifier {
    table: Vec<(String, FailureRegime)>,
}

impl FailureRegimeClassifier for TableClassifier {
    fn classify(&self, context: &FailureRegimeContext<'_>) -> Vec<FailureRegimeOccurrence> {
        self.table
            .iter()
            .filter(|(id, _)| id == context.case.id().as_str())
            .map(|(id, regime)| FailureRegimeOccurrence {
                case_id: context.case.id().clone(),
                dataset_role: context.role,
                subject: FailureRegimeSubject::SharedEvaluation,
                regime: *regime,
                evidence: FailureRegimeEvidence::Adapter {
                    adapter: "table-classifier".to_string(),
                    detail: format!("table entry for {id}"),
                },
            })
            .collect()
    }
}

#[test]
fn classifier_adapter_can_supply_domain_regimes_with_typed_evidence() {
    let case = case("a", vec![obs("oracle.a", true)]);
    let adapter = TableClassifier {
        table: vec![
            ("a".to_string(), FailureRegime::ArchitectureMiss),
            ("a".to_string(), FailureRegime::GroundingFailure),
        ],
    };
    let context = FailureRegimeContext {
        case: &case,
        role: DatasetRole::Optimize,
        current: &scored(1, 0, 0, 0),
        candidate: &scored(1, 0, 0, 0),
    };
    let regimes = adapter.classify(&context);
    assert_eq!(regimes.len(), 2);
    assert!(
        regimes
            .iter()
            .all(|r| matches!(r.evidence, FailureRegimeEvidence::Adapter { .. }))
    );
}

// --- R / S / T: authority audits ------------------------------------

#[test]
fn r_s_t_shadow_module_has_no_authority_surface() {
    let src = module_source();
    for forbidden in [
        "PolicyDecision",
        "PolicyOutcome",
        "PromotionPermit",
        "issue_promotion_permit",
        "PromotionAuthorization",
        "PromotionAuthorizationPolicy",
        "VerifiedExternalApproval",
        "apply_with_permit",
        "FixAgent",
        "ChangeProposal",
    ] {
        assert!(
            !src.contains(forbidden),
            "e82 shadow evaluation must not reference `{forbidden}`"
        );
    }
}

#[test]
fn no_hidden_thresholds_in_the_shadow_module() {
    let src = module_source().to_lowercase();
    for forbidden in ["threshold", "tolerance", "is_better", "is_worse", "winner"] {
        assert!(
            !src.contains(forbidden),
            "e82 must not encode metric policy (`{forbidden}`)"
        );
    }
}

// --- WU22: vertical UAT ---------------------------------------------

#[test]
fn wu22_vertical_shadow_evaluation_over_the_e81_stack() {
    let normaliser = DefaultNormaliser;

    // Corpus:
    //   a (OPTIMIZE) scored; current expects it absent (FP), candidate expects
    //                it present (TP) — candidate improves OPTIMIZE.
    //   d (OPTIMIZE) real platform divergence.
    //   b (CONFIRM)  current expects present (TP); candidate adds an expectation
    //                that never fires — candidate regresses CONFIRM.
    //   c (CONFIRM)  no recorded observation -> incomplete.
    let plan = plan_for(
        vec![
            case("a", vec![obs("oracle.a", true)]),
            case(
                "d",
                vec![
                    obs_raw(PlatformKind::MacOs, "oracle.d", b"mac"),
                    obs_raw(PlatformKind::Windows, "oracle.d", b"windows"),
                ],
            ),
            case("b", vec![obs("oracle.b", true)]),
            case("c", vec![]),
        ],
        &["a", "d"],
        &["b", "c"],
    );

    let current = ScriptedAnalyzer::new(
        "current",
        vec![
            ("a", vec![]),
            ("d", vec![("oracle.d", true)]),
            ("b", vec![("oracle.b", true)]),
        ],
    );
    let candidate = ScriptedAnalyzer::new(
        "candidate",
        vec![
            ("a", vec![("oracle.a", true)]),
            ("d", vec![("oracle.d", true)]),
            ("b", vec![("oracle.b", true), ("oracle.missing", true)]),
        ],
    );

    let evaluation = ShadowEvaluator::evaluate_all(
        &plan,
        AnalyzerUnderTest::new(plan.current(), &current),
        AnalyzerUnderTest::new(plan.candidate(), &candidate),
        &builtin(&normaliser),
    )
    .expect("shadow evaluation");

    // OPTIMIZE: candidate turns an FP into a TP.
    let optimize = evaluation.optimize();
    assert_eq!(optimize.role(), DatasetRole::Optimize);
    assert_eq!(optimize.aggregate_delta().score.true_positive, 1);
    assert_eq!(optimize.aggregate_delta().score.false_positive, -1);
    assert_eq!(optimize.aggregate_delta().incomplete_cases, 0);
    assert!(optimize
        .regimes_of(FailureRegime::PlatformDivergence)
        .iter()
        .any(|r| matches!(&r.evidence, FailureRegimeEvidence::PlatformDivergence { observation_id, .. } if observation_id == "oracle.d")));

    // CONFIRM: candidate introduces a false negative, and case c is incomplete.
    let confirm = evaluation.confirm();
    assert_eq!(confirm.role(), DatasetRole::Confirm);
    assert_eq!(confirm.aggregate_delta().score.false_negative, 1);
    // Incompleteness is unchanged between the sides (both have exactly one
    // incomplete case), so the delta is zero; the absolute counts are asserted
    // separately below.
    assert_eq!(confirm.aggregate_delta().incomplete_cases, 0);
    assert_eq!(confirm.current_report().incomplete_cases(), 1);
    assert_eq!(confirm.candidate_report().incomplete_cases(), 1);
    assert_eq!(
        confirm.regimes_of(FailureRegime::EvidenceIncomplete).len(),
        2,
        "both sides are incomplete on case c"
    );

    // The two roles stay separate and neither carries a selection.
    assert_ne!(optimize.aggregate_delta(), confirm.aggregate_delta());
    assert_eq!(optimize.evaluation_digest(), confirm.evaluation_digest());
    assert_eq!(
        optimize.current_descriptor().revision_digest(),
        "rev-current"
    );
    assert_eq!(
        optimize.candidate_descriptor().revision_digest(),
        "rev-candidate"
    );

    // Each analyzer is invoked once per case per role: OPTIMIZE has {a, d} and
    // CONFIRM has {b, c}, so four invocations per side.
    assert_eq!(current.calls(), 4);
    assert_eq!(candidate.calls(), 4);
}

// --- helpers ---------------------------------------------------------

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn module_source() -> String {
    let root = src_root().join("application/shadow_evaluation");
    let mut out = String::new();
    for entry in walk_rs(&root) {
        let name = entry.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.ends_with("_tests.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&entry).unwrap_or_default();
        for line in text.lines() {
            out.push_str(line.split("//").next().unwrap_or(""));
            out.push('\n');
        }
    }
    out
}

fn walk_rs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk_rs(&p));
        } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
            out.push(p);
        }
    }
    out
}
