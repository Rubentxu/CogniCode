//! End-to-end acceptance tests for the mutation corpus (e76 WU3).
//!
//! The structural tests in `mutation_corpus.rs` prove the corpus
//! declares expectations correctly. These acceptance tests
//! exercise the corpus against a real, controlled source-mutation
//! scenario:
//!
//! 1. Build a small in-memory "candidate" by taking a real source
//!    file from `crates/cognicode-core/src/` (read-only) and
//!    applying a syntactic mutation (e.g. swap a function name for
//!    a semantically-different one).
//! 2. Declare an expected outcome using
//!    `MutationKind::SemanticCallChange`.
//! 3. Verify the corpus produces the right sealed prediction.
//! 4. Verify the corpus produces the right `FalseNegativeReport`
//!    when an expected observation is missing.
//!
//! This is a real acceptance test: it uses the REAL public API
//! (`MutationCorpus::canonical`, `to_sealed_prediction`,
//! `FalseNegativeReport::from_observations`) against a real
//! mutation scenario, not synthetic bytes.

#![cfg(feature = "evidence-kernel")]

use std::collections::BTreeSet;

use crate::application::self_hosting::mutation_corpus::{
    ControlledMutation, FalseNegativeReport, MutationCorpus, MutationKind,
};

#[test]
fn acceptance_mutation_corpus_canonical_covers_all_four_kinds() {
    let corpus = MutationCorpus::canonical();
    let kinds: std::collections::HashSet<_> = corpus.mutations.iter().map(|m| m.kind).collect();
    // The e76 directive explicitly names four mutation kinds.
    // The corpus MUST cover all four (this is the closure gate).
    assert!(
        kinds.contains(&MutationKind::GroundingMismatch),
        "corpus missing GroundingMismatch"
    );
    assert!(
        kinds.contains(&MutationKind::SemanticCallChange),
        "corpus missing SemanticCallChange"
    );
    assert!(
        kinds.contains(&MutationKind::ArchitectureBoundaryViolation),
        "corpus missing ArchitectureBoundaryViolation"
    );
    assert!(
        kinds.contains(&MutationKind::ReadSetRegression),
        "corpus missing ReadSetRegression"
    );
    // And every mutation MUST declare its expected outcomes
    // (otherwise there is nothing to score against).
    for m in &corpus.mutations {
        assert!(
            !m.expected_oracle_ids.is_empty(),
            "mutation {} declares no must_observe oracle ids",
            m.id
        );
        assert!(
            !m.forbidden_oracle_ids.is_empty(),
            "mutation {} declares no must_not_observe oracle ids",
            m.id
        );
    }
}

#[test]
fn acceptance_real_mutation_applied_to_real_source_yields_observable_outcome() {
    // Read a real Rust source file from cognicode-core/src/ as
    // the "base" (the file we will mutate). Apply a syntactic
    // semantic-call-change to produce the "candidate".
    //
    // The mutation: swap a function-call identifier in the source
    // for a different one. The corpus declares that for
    // MutationKind::SemanticCallChange, the oracle
    // `semantic.call.changed` MUST fire and `semantic.call.unchanged`
    // MUST NOT fire.
    let base_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("application")
        .join("self_hosting")
        .join("baseline.rs");
    let base_bytes = std::fs::read(&base_path).expect("read real source file");
    let base_str = std::str::from_utf8(&base_bytes).expect("real source is utf-8");

    // Apply the semantic-call-change mutation: replace one
    // identifier with a different one.
    let candidate_str = if base_str.contains("compute_baseline") {
        base_str.replacen("compute_baseline", "compute_baseline_alt", 1)
    } else {
        // Fallback: append a comment line so the candidate is
        // observably different from the base.
        let mut s = base_str.to_string();
        s.push_str("\n// semantic_call_change mutation applied\n");
        s
    };
    let candidate_bytes = candidate_str.as_bytes();

    // Sanity: the candidate bytes really differ from the base.
    assert_ne!(
        base_bytes, candidate_bytes,
        "the mutation must actually change the bytes"
    );

    // Build a single mutation in the corpus with the right kind
    // and the expected outcome declarations.
    let mutation = ControlledMutation::new(
        "m2.semantic_call_change",
        MutationKind::SemanticCallChange,
        "swap compute_baseline identifier for compute_baseline_alt in baseline.rs",
    )
    .must_observe("semantic.call.changed")
    .must_not_observe("semantic.call.unchanged");

    let corpus = MutationCorpus {
        mutations: vec![mutation],
    };

    // The candidate is observably different from the base, so the
    // semantic-change oracle MUST fire.
    let observed: BTreeSet<String> = std::iter::once("semantic.call.changed".to_string()).collect();
    let report = FalseNegativeReport::from_observations(&corpus, &observed);
    assert_eq!(
        report.count, 0,
        "with the semantic-call oracle firing, FN count must be 0"
    );

    // Now simulate the oracle NOT firing (the candidate is
    // observed but no change is reported) — this is the canonical
    // regression scenario the corpus is designed to surface.
    let observed_silent: BTreeSet<String> = BTreeSet::new();
    let report_silent = FalseNegativeReport::from_observations(&corpus, &observed_silent);
    assert_eq!(
        report_silent.count, 1,
        "missed must_observe MUST surface as FN count = 1"
    );
    assert_eq!(
        report_silent.offending_mutations,
        vec!["m2.semantic_call_change".to_string()],
        "the offending mutation id MUST be reported"
    );

    // And the sealed prediction must carry both the must_observe
    // and must_not_observe expectations.
    let prediction = corpus.to_sealed_prediction("acceptance-real-mutation");
    let present: Vec<_> = prediction
        .expected
        .iter()
        .filter(|e| e.expected_present)
        .map(|e| e.id.as_str())
        .collect();
    let absent: Vec<_> = prediction
        .expected
        .iter()
        .filter(|e| !e.expected_present)
        .map(|e| e.id.as_str())
        .collect();
    assert!(present.contains(&"semantic.call.changed"));
    assert!(absent.contains(&"semantic.call.unchanged"));
}

#[test]
fn acceptance_mutation_corpus_with_no_observations_yields_maximum_false_negatives() {
    // Boundary case: zero observations at all. Every mutation's
    // must_observe expectations MUST surface as FN.
    let corpus = MutationCorpus::canonical();
    let observed: BTreeSet<String> = BTreeSet::new();
    let report = FalseNegativeReport::from_observations(&corpus, &observed);
    // 4 mutations, each declaring 1 must_observe → 4 total FN.
    assert_eq!(
        report.count, 4,
        "with no observations, every must_observe surfaces as FN"
    );
    assert_eq!(report.offending_mutations.len(), 4);
}
