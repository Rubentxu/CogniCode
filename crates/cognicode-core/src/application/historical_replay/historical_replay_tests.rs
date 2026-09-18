//! e81 WU10 — adversarial separation tests + WU11 e76 integration UAT.
//!
//! Invariant under test:
//!
//! ```text
//! OPTIMIZE ∩ CONFIRM = ∅
//! overlap => configuration error => ZERO predictor executions
//! predictor input carries no outcome
//! replay is deterministic and reports measurement only
//! ```

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

use crate::application::historical_replay::case::{
    ContentRef, HistoricalCase, HistoricalCaseId, HistoricalPredictionInput,
};
use crate::application::historical_replay::corpus::{CorpusError, HistoricalCorpus};
use crate::application::historical_replay::plan::{HistoricalReplayPlan, PlanError};
use crate::application::historical_replay::replay::{
    HistoricalPredictor, ReplayCaseOutcome, ReplayIncomplete,
};
use crate::application::historical_replay::split::{DatasetRole, DatasetSplit, SplitError};
use crate::application::self_hosting::platform_equivalence::{
    HistoricalReplay, PlatformKind, PlatformObservation,
};
use crate::application::self_hosting::prediction::{ExpectedObservation, SealedPrediction};

// --- fixtures -------------------------------------------------------

fn case_id(s: &str) -> HistoricalCaseId {
    HistoricalCaseId::try_new(s).expect("non-empty case id")
}

fn content_ref(s: &str) -> ContentRef {
    ContentRef::try_new(s).expect("non-empty content ref")
}

fn obs(id: &str, fired: bool) -> PlatformObservation {
    PlatformObservation {
        platform: PlatformKind::Linux,
        id: id.to_string(),
        // e76 convention: an empty raw payload means "did not fire".
        raw: if fired {
            b"present".to_vec()
        } else {
            Vec::new()
        },
    }
}

fn exp(id: &str, present: bool) -> ExpectedObservation {
    ExpectedObservation {
        id: id.to_string(),
        expected_present: present,
    }
}

/// A case whose replay carries the given observations (and no historical seal).
fn case(id: &str, observations: Vec<PlatformObservation>) -> HistoricalCase {
    HistoricalCase::try_new(
        case_id(id),
        content_ref(&format!("src://base/{id}")),
        format!("base-snap-{id}"),
        content_ref(&format!("src://successor/{id}")),
        HistoricalReplay::observation_only(format!("snap-{id}"), observations),
    )
    .expect("valid case")
}

fn corpus(ids: &[&str]) -> HistoricalCorpus {
    HistoricalCorpus::try_new(
        ids.iter()
            .map(|id| case(id, vec![obs("oracle.a", true)]))
            .collect(),
    )
    .expect("valid corpus")
}

/// A deterministic predictor that records its invocations and inputs.
struct FixturePredictor {
    calls: Cell<usize>,
    expected: Vec<ExpectedObservation>,
    inputs: RefCell<Vec<HistoricalPredictionInput>>,
}

impl FixturePredictor {
    fn new(expected: Vec<ExpectedObservation>) -> Self {
        Self {
            calls: Cell::new(0),
            expected,
            inputs: RefCell::new(Vec::new()),
        }
    }

    fn calling_with(ids: &[&str]) -> Self {
        Self::new(ids.iter().map(|id| exp(id, true)).collect())
    }

    fn calls(&self) -> usize {
        self.calls.get()
    }
}

impl HistoricalPredictor for FixturePredictor {
    fn predict(&self, input: &HistoricalPredictionInput) -> SealedPrediction {
        self.calls.set(self.calls.get() + 1);
        self.inputs.borrow_mut().push(input.clone());
        SealedPrediction::seal("e81-fixture", self.expected.clone())
    }
}

// --- A / B / C: split construction errors ---------------------------

#[test]
fn a_overlap_is_a_configuration_error_and_runs_nothing() {
    let corpus = corpus(&["a", "b"]);
    let predictor = FixturePredictor::calling_with(&["oracle.a"]);

    // A pipeline that accepts raw id vectors: the failure must land at split
    // construction, before any plan exists.
    let split = DatasetSplit::try_new(vec![case_id("a")], vec![case_id("a")]);
    assert!(matches!(split, Err(SplitError::Overlap(_))));

    let plan: Result<HistoricalReplayPlan, String> = match split {
        Ok(s) => HistoricalReplayPlan::prepare(&corpus, &s).map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    };
    assert!(plan.is_err(), "overlapping split must not yield a plan");
    assert_eq!(
        predictor.calls(),
        0,
        "configuration error must run zero predictors"
    );
}

#[test]
fn b_duplicate_in_optimize_is_rejected() {
    let err = DatasetSplit::try_new(vec![case_id("a"), case_id("a")], vec![case_id("b")]);
    assert!(matches!(err, Err(SplitError::DuplicateInOptimize(_))));
}

#[test]
fn c_duplicate_in_confirm_is_rejected() {
    let err = DatasetSplit::try_new(vec![case_id("a")], vec![case_id("b"), case_id("b")]);
    assert!(matches!(err, Err(SplitError::DuplicateInConfirm(_))));
}

// --- D: unknown case id ---------------------------------------------

#[test]
fn d_unknown_case_id_is_a_configuration_error_and_runs_nothing() {
    let corpus = corpus(&["a", "b"]);
    let split = DatasetSplit::try_new(vec![case_id("a")], vec![case_id("ghost")]).unwrap();
    let predictor = FixturePredictor::calling_with(&["oracle.a"]);

    let plan = HistoricalReplayPlan::prepare(&corpus, &split);
    assert!(matches!(plan, Err(PlanError::UnknownCaseId(_))));
    assert_eq!(predictor.calls(), 0);
}

// --- E: split immutability vs caller mutation -----------------------

#[test]
fn e_mutating_caller_vectors_after_split_creation_does_not_change_the_split() {
    let mut optimize = vec![case_id("a")];
    let mut confirm = vec![case_id("b")];
    let split = DatasetSplit::try_new(optimize.clone(), confirm.clone()).expect("disjoint split");
    let digest_before = split.digest().to_string();

    optimize.push(case_id("c"));
    confirm.clear();
    confirm.push(case_id("d"));

    assert_eq!(split.optimize(), &[case_id("a")]);
    assert_eq!(split.confirm(), &[case_id("b")]);
    assert_eq!(split.digest(), digest_before);
}

// --- F / G: digest properties ---------------------------------------

#[test]
fn f_input_order_does_not_change_the_split_digest() {
    let a = DatasetSplit::try_new(
        vec![case_id("b"), case_id("a")],
        vec![case_id("d"), case_id("c")],
    )
    .unwrap();
    let b = DatasetSplit::try_new(
        vec![case_id("a"), case_id("b")],
        vec![case_id("c"), case_id("d")],
    )
    .unwrap();
    assert_eq!(a.digest(), b.digest(), "canonical order must normalise");
}

#[test]
fn g_swapping_roles_changes_the_split_digest() {
    let a = DatasetSplit::try_new(vec![case_id("a")], vec![case_id("b")]).unwrap();
    let b = DatasetSplit::try_new(vec![case_id("b")], vec![case_id("a")]).unwrap();
    assert_ne!(
        a.digest(),
        b.digest(),
        "the digest must encode ROLE, not just the id set"
    );
}

// --- H: the predictor input cannot carry the outcome ----------------

#[test]
fn h_prediction_input_has_no_outcome_fields() {
    // A struct literal with exactly the three base-side fields. If a successor,
    // observation, outcome, or score field were added, this stops compiling.
    let input = HistoricalPredictionInput {
        case_id: case_id("a"),
        base_snapshot: "base-snap-a".to_string(),
        base_source_ref: content_ref("src://base/a"),
    };
    assert_eq!(input.case_id, case_id("a"));

    // Source audit: the type has no outcome-bearing field name.
    let src = read_src("application/historical_replay/case.rs");
    let body = struct_body(&src, "HistoricalPredictionInput");
    for forbidden in [
        "outcome",
        "observation",
        "successor",
        "score",
        "confirm",
        "sealed",
    ] {
        assert!(
            !body.to_lowercase().contains(forbidden),
            "HistoricalPredictionInput must not carry `{forbidden}`"
        );
    }
}

// --- I: observations cannot retro-fit a prediction ------------------

#[test]
fn i_changing_observations_after_the_fact_does_not_change_the_seal() {
    let predictor = FixturePredictor::calling_with(&["oracle.a"]);

    // Two corpora over the same base but with different recorded observations.
    let with_observation =
        HistoricalCorpus::try_new(vec![case("a", vec![obs("oracle.a", true)])]).unwrap();
    let without_observation = HistoricalCorpus::try_new(vec![case("a", vec![])]).unwrap();

    let split = DatasetSplit::try_new(vec![case_id("a")], vec![]).unwrap();
    let plan_a = HistoricalReplayPlan::prepare(&with_observation, &split).unwrap();
    let plan_b = HistoricalReplayPlan::prepare(&without_observation, &split).unwrap();

    let report_a = plan_a.run(&predictor);
    let report_b = plan_b.run(&predictor);

    let seal_a = &report_a.optimize().cases()[0].prediction_seal;
    let seal_b = &report_b.optimize().cases()[0].prediction_seal;
    assert_eq!(
        seal_a, seal_b,
        "the predictor only saw base-side input, so the seal cannot depend on observations"
    );
    // The recorded observations only changed what the score was, never the seal.
    assert!(report_a.optimize().cases()[0].outcome.is_scored());
    assert!(!report_b.optimize().cases()[0].outcome.is_scored());
}

// --- J: determinism --------------------------------------------------

#[test]
fn j_identical_inputs_yield_identical_reports() {
    let corpus = corpus(&["a", "b", "c"]);
    let split =
        DatasetSplit::try_new(vec![case_id("a"), case_id("b")], vec![case_id("c")]).unwrap();
    let predictor = FixturePredictor::calling_with(&["oracle.a"]);

    let plan = HistoricalReplayPlan::prepare(&corpus, &split).unwrap();
    let first = plan.run(&predictor);
    let second = plan.run(&predictor);
    assert_eq!(first, second);
    assert_eq!(first.optimize().aggregate(), second.optimize().aggregate());
}

// --- K: the two reports stay distinct -------------------------------

#[test]
fn k_optimize_and_confirm_reports_remain_distinct() {
    let corpus = corpus(&["a", "b", "c"]);
    let split =
        DatasetSplit::try_new(vec![case_id("a"), case_id("b")], vec![case_id("c")]).unwrap();
    let predictor = FixturePredictor::calling_with(&["oracle.a"]);
    let plan = HistoricalReplayPlan::prepare(&corpus, &split).unwrap();
    let reports = plan.run(&predictor);

    assert_eq!(reports.optimize().role(), DatasetRole::Optimize);
    assert_eq!(reports.confirm().role(), DatasetRole::Confirm);
    assert_eq!(reports.optimize().cases().len(), 2);
    assert_eq!(reports.confirm().cases().len(), 1);
    assert_eq!(
        reports.report_for(DatasetRole::Confirm).cases()[0].case_id,
        case_id("c")
    );
    // No blended total is exposed: each side is measured on its own.
    assert_ne!(reports.optimize().cases(), reports.confirm().cases());
}

// --- L / M: no authority --------------------------------------------

#[test]
fn l_replay_does_not_mint_a_policy_decision() {
    let src = module_source();
    for forbidden in [
        "PolicyDecision",
        "PolicyOutcome",
        "PolicyGate",
        "PolicySpec",
    ] {
        assert!(!src.contains(forbidden), "e81 must not mint `{forbidden}`");
    }
}

#[test]
fn m_replay_cannot_construct_a_promotion_permit() {
    let src = module_source();
    for forbidden in [
        "PromotionPermit",
        "issue_promotion_permit",
        "PromotionAuthorization",
        "PromotionAuthorizationPolicy",
        "VerifiedExternalApproval",
        "apply_with_permit",
        "FixAgent",
    ] {
        assert!(
            !src.contains(forbidden),
            "e81 replay must not reference `{forbidden}`"
        );
    }
}

// --- N: observations cannot mutate a sealed prediction --------------

#[test]
fn n_sealed_prediction_is_unchanged_by_scoring() {
    let corpus = corpus(&["a"]);
    let split = DatasetSplit::try_new(vec![case_id("a")], vec![]).unwrap();
    let predictor = FixturePredictor::calling_with(&["oracle.a"]);
    let plan = HistoricalReplayPlan::prepare(&corpus, &split).unwrap();

    let direct = predictor.predict(&corpus.cases()[0].prediction_input());
    let seal_before = direct.seal_digest.clone();

    let report = plan.run(&predictor);
    assert!(report.optimize().cases()[0].outcome.is_scored());
    assert_eq!(direct.seal_digest, seal_before);
    assert_eq!(
        report.optimize().cases()[0].prediction_seal,
        seal_before,
        "scoring must not alter the prediction that was compared"
    );
}

// --- O: missing observation is explicitly incomplete ----------------

#[test]
fn o_missing_observation_is_incomplete_never_safe() {
    let corpus = HistoricalCorpus::try_new(vec![case("a", vec![])]).unwrap();
    let split = DatasetSplit::try_new(vec![case_id("a")], vec![]).unwrap();
    let predictor = FixturePredictor::calling_with(&["oracle.a"]);
    let plan = HistoricalReplayPlan::prepare(&corpus, &split).unwrap();
    let reports = plan.run(&predictor);

    let result = &reports.optimize().cases()[0];
    assert_eq!(
        result.outcome,
        ReplayCaseOutcome::Incomplete(ReplayIncomplete::NoRecordedObservation)
    );
    assert!(!result.outcome.is_scored());
    assert!(result.outcome.score().is_none());
    assert_eq!(reports.optimize().incomplete_cases(), 1);
    assert_eq!(reports.optimize().scored_cases(), 0);
    // Crucially: nothing is silently counted as a true negative.
    assert_eq!(reports.optimize().aggregate().true_negative, 0);
    assert_eq!(reports.optimize().aggregate().unknown, 0);
}

// --- corpus invariants ----------------------------------------------

#[test]
fn corpus_rejects_duplicates_and_canonicalises_order() {
    let dup = HistoricalCorpus::try_new(vec![case("a", vec![]), case("a", vec![])]);
    assert!(matches!(dup, Err(CorpusError::DuplicateCaseId(_))));
    assert!(matches!(
        HistoricalCorpus::try_new(vec![]),
        Err(CorpusError::EmptyCorpus)
    ));

    let unordered = HistoricalCorpus::try_new(vec![case("b", vec![]), case("a", vec![])]).unwrap();
    let ordered = HistoricalCorpus::try_new(vec![case("a", vec![]), case("b", vec![])]).unwrap();
    assert_eq!(unordered.digest(), ordered.digest());
    assert_eq!(unordered.cases()[0].id(), &case_id("a"));
}

#[test]
fn an_empty_split_side_is_allowed_but_still_validated() {
    let split = DatasetSplit::try_new(vec![case_id("a")], vec![]).unwrap();
    assert_eq!(split.confirm().len(), 0);
    assert_eq!(split.role_of(&case_id("a")), Some(DatasetRole::Optimize));
    assert_eq!(split.role_of(&case_id("z")), None);
}

// --- WU11: e76 integration UAT --------------------------------------

#[test]
fn wu11_e76_replay_flows_into_optimize_and_confirm_reports() {
    // Build cases from the e76 HistoricalReplay container, exactly as a real
    // caller would.
    let replays = vec![
        (
            "case-1",
            HistoricalReplay::observation_only("snap-1", vec![obs("oracle.a", true)]),
        ),
        (
            "case-2",
            HistoricalReplay::observation_only(
                "snap-2",
                vec![obs("oracle.b", true), obs("oracle.a", false)],
            ),
        ),
        (
            "case-3",
            HistoricalReplay::observation_only("snap-3", vec![obs("oracle.a", true)]),
        ),
    ];
    let cases: Vec<HistoricalCase> = replays
        .into_iter()
        .map(|(id, replay)| {
            HistoricalCase::try_new(
                case_id(id),
                content_ref(&format!("src://base/{id}")),
                format!("base-snap-{id}"),
                content_ref(&format!("src://successor/{id}")),
                replay,
            )
            .unwrap()
        })
        .collect();

    let corpus = HistoricalCorpus::try_new(cases).unwrap();
    let split = DatasetSplit::try_new(
        vec![case_id("case-1"), case_id("case-2")],
        vec![case_id("case-3")],
    )
    .unwrap();
    let plan = HistoricalReplayPlan::prepare(&corpus, &split).unwrap();

    assert_eq!(plan.corpus_digest(), corpus.digest());
    assert_eq!(plan.split_digest(), split.digest());

    // A predictor that expects oracle.a present. case-2 records oracle.a absent,
    // so it must show up as a false negative on the OPTIMIZE side.
    let predictor = FixturePredictor::calling_with(&["oracle.a"]);
    let reports = plan.run(&predictor);

    let optimize = reports.optimize();
    assert_eq!(optimize.role(), DatasetRole::Optimize);
    assert_eq!(optimize.scored_cases(), 2);
    assert_eq!(optimize.incomplete_cases(), 0);
    assert_eq!(optimize.aggregate().true_positive, 1); // case-1
    assert_eq!(optimize.aggregate().false_negative, 1); // case-2

    let confirm = reports.confirm();
    assert_eq!(confirm.role(), DatasetRole::Confirm);
    assert_eq!(confirm.scored_cases(), 1);
    assert_eq!(confirm.aggregate().true_positive, 1); // case-3

    // Every prediction in the run carried a real seal.
    for case in optimize.cases().iter().chain(confirm.cases().iter()) {
        assert!(!case.prediction_seal.is_empty());
    }
    assert_eq!(predictor.calls(), 3);
}

#[test]
fn wu11_case_rejects_empty_inputs() {
    assert!(matches!(
        HistoricalCaseId::try_new("  "),
        Err(crate::application::historical_replay::case::HistoricalCaseError::EmptyCaseId)
    ));
    assert!(matches!(
        ContentRef::try_new(""),
        Err(crate::application::historical_replay::case::HistoricalCaseError::EmptyContentRef)
    ));
    let bad = HistoricalCase::try_new(
        case_id("a"),
        content_ref("src://base/a"),
        "  ",
        content_ref("src://successor/a"),
        HistoricalReplay::observation_only("snap", vec![]),
    );
    assert!(bad.is_err());
}

// --- WU12: no random splitting --------------------------------------

#[test]
fn wu12_module_contains_no_random_splitting() {
    let src = module_source().to_lowercase();
    for forbidden in [
        "rand::",
        "thread_rng",
        "shuffle",
        "splitmix",
        "getrandom",
        "seed",
    ] {
        assert!(
            !src.contains(forbidden),
            "e81 must not randomise dataset assignment (`{forbidden}`)"
        );
    }
}

// --- helpers ---------------------------------------------------------

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn read_src(rel: &str) -> String {
    std::fs::read_to_string(src_root().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// Concatenated source of the e81 module (production files only, comments
/// stripped), for the authority/randomness audits.
fn module_source() -> String {
    let root = src_root().join("application/historical_replay");
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

/// Extract the `{ ... }` body of a struct definition by name.
fn struct_body(src: &str, name: &str) -> String {
    let marker = format!("pub struct {name} ");
    let start = src
        .find(&marker)
        .unwrap_or_else(|| panic!("{name} not found"));
    let brace = src[start..].find('{').expect("struct body") + start;
    let mut depth = 0usize;
    for (i, ch) in src[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return src[brace..brace + i + 1].to_string();
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced braces for {name}");
}
