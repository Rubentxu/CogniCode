#![cfg(feature = "evidence-kernel")]
//! E38 M3 identity benchmark (design D7) — scores the tiered continuity
//! matcher over the ground-truth `sandbox/fixtures/lsi-identity/` cases.
//!
//! Declared contract (spec `identity-benchmark`, requirement "Pinned
//! scoring gates and fixture coverage", stated BEFORE any scoring):
//!
//! - pinned gates: [`harness::PRECISION_THRESHOLD`] = 0.95,
//!   [`harness::RECALL_THRESHOLD`] = 0.90,
//!   [`harness::LINE_SHIFT_RETENTION_THRESHOLD`] = 1.00,
//!   [`harness::MOVE_RETENTION_THRESHOLD`] = 0.99;
//! - scoring: precision/recall over the Matched-pair multiset per case,
//!   pooled across the seven declared fixture cases (rename/move with and
//!   without edits, line shift, colliding names, unchanged control);
//! - `Ambiguous` outcomes count as NEITHER match nor miss and are reported
//!   separately (quarantine pattern); a case EXPECTING `Ambiguous` that
//!   returns any other status FAILS the run (inverted fail-closed check);
//! - a missed gate fails the run naming the gate and its measured value;
//! - the matcher/fingerprint convention is pinned through
//!   [`harness::PINNED_MATCHER_DIGEST`] — a digest SEPARATE from e37's
//!   fact-identity digest; any convention change fails the run until the
//!   digest is explicitly re-pinned (re-pin duty).

#[path = "identity_benchmark/harness.rs"]
mod harness;

use std::path::{Path, PathBuf};

use cognicode_core::domain::evidence_kernel::continuity::{
    ContinuityOutcome, ContinuityResult, ContinuityStatus, MatchTier,
};
use cognicode_core::domain::evidence_kernel::ids::{EntityId, OccurrenceId, StableEntityId};

use harness::{
    ALL_CASES, COLLIDING_CASE, CaseReport, ExpectedEntity, ExpectedMapping, ExpectedStatus,
    PINNED_MATCHER_DIGEST,
};

/// Locates the fixture root (`sandbox/fixtures/lsi-identity`) from the crate.
fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures/lsi-identity")
        .canonicalize()
        .expect("sandbox/fixtures/lsi-identity resolves")
}

/// Spec scenario "Missed gate fails the run": one declared gate not met
/// fails the run AND names the gate and its measured value.
#[test]
fn missed_gate_fails_the_run() {
    // Synthetic report: 2 expected pairs, only 1 matched → recall 0.5.
    let report = CaseReport {
        case: "line-shift".to_string(),
        passed: false,
        actual_matched: vec![(
            "src/engine.rs:tick:1".to_string(),
            "src/engine.rs:tick:6".to_string(),
        )],
        expected_matched: vec![
            (
                "src/engine.rs:tick:1".to_string(),
                "src/engine.rs:tick:6".to_string(),
            ),
            (
                "src/engine.rs:tick:2".to_string(),
                "src/engine.rs:tick:7".to_string(),
            ),
        ],
        ambiguous: Vec::new(),
        failures: Vec::new(),
    };

    let gate = harness::evaluate_gates(&[report]);

    assert!(!gate.passed, "a missed gate must fail the run");
    assert!(
        gate.failures
            .iter()
            .any(|f| f.contains("recall") && f.contains("0.5")),
        "the failure must name the gate and its measured value: {:?}",
        gate.failures
    );
}

/// Spec scenario "Missed gate fails the run", precision variant: extra
/// matched pairs that the ground truth does not declare drag precision
/// below the gate, and the failure names gate + value.
#[test]
fn precision_gate_failure_names_gate_and_value() {
    let report = CaseReport {
        case: "pure-rename".to_string(),
        passed: false,
        actual_matched: vec![
            (
                "src/lib.rs:fetch_total:1".to_string(),
                "src/lib.rs:settle_total:1".to_string(),
            ),
            (
                "src/lib.rs:wrong:1".to_string(),
                "src/lib.rs:target:1".to_string(),
            ),
        ],
        expected_matched: vec![(
            "src/lib.rs:fetch_total:1".to_string(),
            "src/lib.rs:settle_total:1".to_string(),
        )],
        ambiguous: Vec::new(),
        failures: Vec::new(),
    };

    let gate = harness::evaluate_gates(&[report]);

    assert!(!gate.passed);
    assert!(
        gate.failures
            .iter()
            .any(|f| f.contains("precision") && f.contains("0.5")),
        "the failure must name the precision gate and its measured value: {:?}",
        gate.failures
    );
}

/// Spec scenario "Ambiguous outcome reported separately": the colliding
/// names fixture expecting ambiguity is reported as ambiguous, and its
/// ambiguous pair is excluded from the scored precision/recall multisets.
#[test]
fn ambiguous_outcome_reported_separately() {
    let run = harness::run(&fixtures_root());

    let colliding = run
        .reports
        .iter()
        .find(|r| r.case == COLLIDING_CASE)
        .unwrap_or_else(|| panic!("case {COLLIDING_CASE} must be reported"));
    assert!(
        !colliding.ambiguous.is_empty(),
        "the colliding-names case must report its ambiguous finding separately"
    );
    for finding in &colliding.ambiguous {
        assert!(
            finding.matched,
            "expected-Ambiguous must be verified as Ambiguous with the expected candidate set: {:?}",
            finding
        );
        // The actual candidate set is recorded and equals the expectation.
        assert_eq!(
            finding.actual_candidates, finding.expected_candidates,
            "the reported candidates must be the recorded actual candidates"
        );
        // The ambiguous pair enters NEITHER scoring multiset: it is absent
        // from the case's matched pairs (neither match nor miss).
        assert!(
            !colliding
                .actual_matched
                .iter()
                .any(|(_, after)| after == &finding.after),
            "an Ambiguous outcome must be excluded from the matched multiset"
        );
        println!(
            "quarantined ambiguous: case '{}' after='{}' candidates={:?}",
            finding.case, finding.after, finding.expected_candidates
        );
    }
}

/// Spec scenario "Ambiguous outcome reported separately" inverted
/// fail-closed check: a case EXPECTING `Ambiguous` that returns any other
/// status (e.g. a forced Matched) FAILS the case.
#[test]
fn expected_ambiguous_returning_matched_fails_the_case() {
    let expected = ExpectedMapping {
        schema_version: 1,
        case: "synthetic".to_string(),
        entities: vec![ExpectedEntity {
            before: None,
            after: Some("src/c.rs:dup:1".to_string()),
            status: ExpectedStatus::Ambiguous,
            tier: None,
            candidates: vec!["src/a.rs:dup:1".to_string(), "src/b.rs:dup:1".to_string()],
        }],
    };
    // The actual result force-matched the pair instead of failing closed.
    let result = ContinuityResult {
        outcomes: vec![ContinuityOutcome {
            fqn: "src/c.rs:dup:1".to_string(),
            occurrence: OccurrenceId::from_entity(EntityId::new(1)),
            snapshot: harness::SNAPSHOT_AFTER,
            stable_id: Some(StableEntityId::new(1)),
            status: ContinuityStatus::Matched {
                tier: MatchTier::Fingerprint { score: 0.9 },
                confidence: 0.9,
            },
        }],
    };

    let report = harness::verify_mapping(
        "synthetic",
        &expected,
        &result,
        &["src/a.rs:dup:1".to_string(), "src/b.rs:dup:1".to_string()],
    );

    assert!(
        !report.passed,
        "an expected-Ambiguous that returns Matched must FAIL the case"
    );
    assert!(
        report.failures.iter().any(|f| f.contains("mbiguous")),
        "the failure must name the inverted ambiguous expectation: {:?}",
        report.failures
    );
}

/// Spec requirement "Separate pinned convention digest", scenario
/// "Convention change requires explicit re-pin": the current convention
/// matches its own pin, ANY convention change fails until the digest is
/// explicitly re-pinned, and the pinned digest is separate from e37's
/// fact-identity digest.
#[test]
fn convention_change_requires_explicit_re_pin() {
    // The current convention matches the pin…
    let current = harness::matcher_convention_digest();
    harness::verify_matcher_pin(&current)
        .expect("current matcher convention must match the pinned digest");

    // …and ANY convention change fails with a re-pin instruction.
    let changed =
        harness::matcher_convention_digest_for("e38 matcher convention v2: tuned thresholds");
    let err = harness::verify_matcher_pin(&changed)
        .expect_err("a changed convention must require an explicit re-pin");
    assert!(
        err.contains("re-pin"),
        "failure must instruct explicit re-pinning: {err}"
    );

    // Separate from e37's fact-identity digest (not a reused pin).
    assert_ne!(
        PINNED_MATCHER_DIGEST,
        harness::E37_FACT_IDENTITY_DIGEST,
        "the matcher convention digest must be its own pin, not e37's"
    );
    assert_ne!(
        current,
        harness::E37_FACT_IDENTITY_DIGEST.to_string(),
        "the live convention must not collide with e37's fact-identity digest"
    );
}

/// E38.1 CP-6 self-check: the pinned matcher convention text states its
/// load-bearing strings (1-based identity grammar + pinned threshold
/// constants) — a cheap guard against a truncated or stale convention
/// edit. The matcher convention does NOT reference E38.1-changed internals
/// (no `SymbolFqn`/codec wording), so it was deliberately NOT re-pinned.
#[test]
fn matcher_convention_states_the_declared_rules() {
    assert!(
        harness::MATCHER_CONVENTION
            .contains("identity grammar '{file}:{name}:{line}' with 1-based line"),
        "the matcher convention must state the 1-based identity grammar"
    );
    assert!(
        harness::MATCHER_CONVENTION.contains("jaccard_threshold=0.6 epsilon=0.05 rename_floor=0.5"),
        "the matcher convention must state the pinned threshold constants"
    );
    harness::verify_matcher_pin(&harness::matcher_convention_digest())
        .expect("the pinned digest must match the current convention text");
}

/// Spec requirement "Pinned scoring gates and fixture coverage": the seven
/// declared ground-truth cases all pass and every gate is met.
#[test]
fn all_cases_meet_the_declared_gates() {
    let run = harness::run(&fixtures_root());

    println!("== E38 identity benchmark ==");
    println!("matcher convention digest: {}", run.digest);
    for report in &run.reports {
        println!("{}", report.describe());
    }
    println!(
        "gates: precision={:.4} (>= {}) recall={:.4} (>= {}) line_shift_retention={:.4} (== {}) move_retention={:.4} (>= {})",
        run.gate.precision,
        harness::PRECISION_THRESHOLD,
        run.gate.recall,
        harness::RECALL_THRESHOLD,
        run.gate.line_shift_retention,
        harness::LINE_SHIFT_RETENTION_THRESHOLD,
        run.gate.move_retention,
        harness::MOVE_RETENTION_THRESHOLD,
    );

    // The convention pin must hold.
    assert!(
        run.digest_ok,
        "matcher convention digest {} does not match the pinned baseline {}",
        run.digest, PINNED_MATCHER_DIGEST
    );

    // Every declared case scored and passed.
    for name in ALL_CASES {
        let report = run
            .reports
            .iter()
            .find(|r| r.case == name)
            .unwrap_or_else(|| panic!("missing report for declared case {name}"));
        assert!(report.passed, "case {name} failed: {:?}", report.failures);
    }

    // The pooled gates are met.
    assert!(
        run.gate.precision >= harness::PRECISION_THRESHOLD,
        "precision gate missed: {:.4}",
        run.gate.precision
    );
    assert!(
        run.gate.recall >= harness::RECALL_THRESHOLD,
        "recall gate missed: {:.4}",
        run.gate.recall
    );
    assert!(
        run.gate.line_shift_retention >= harness::LINE_SHIFT_RETENTION_THRESHOLD,
        "line-shift retention gate missed: {:.4}",
        run.gate.line_shift_retention
    );
    assert!(
        run.gate.move_retention >= harness::MOVE_RETENTION_THRESHOLD,
        "move retention gate missed: {:.4}",
        run.gate.move_retention
    );
    assert!(
        run.passed,
        "harness run failed: {:?}",
        run.failures.join(" | ")
    );
}
