//! Do `IncrementalComplete` and `FullRebuild` agree over a real corpus?
//!
//! The corpus is `testdata/mode_equivalence`: a Rust, a Java, a Kotlin, a
//! Go, a Terraform and a Helm part, each edited three ways (simple,
//! semantic and malformed), plus edits that cross parts. The harness in
//! `mode_equivalence/harness.rs` commits each edit into a throwaway Git
//! repository and compares the two modes through the production seams.
//!
//! Every scenario is judged over two projections of the corpus. Helm
//! templates are not YAML, so over the full corpus every revision has gaps
//! and no verdict can be `Pass`; the clean projection drops them so the
//! comparison also covers the `Pass` path.
//!
//! `corpus_is_certified` is the loud one: any divergence between the modes
//! fails it and prints the table that says where.

mod harness;

use harness::{
    Certification, EditClass, Fault, PARTS, Projection, certify, corpus_root, evaluate,
    evaluate_with_fault, scenarios,
};

fn certification() -> Certification {
    let corpus = corpus_root();
    let results = Projection::ALL
        .into_iter()
        .flat_map(|projection| {
            scenarios(&corpus)
                .into_iter()
                .map(move |scenario| (scenario, projection))
        })
        .map(|(scenario, projection)| evaluate(&corpus, &scenario, projection))
        .collect();
    certify(results)
}

#[test]
fn every_language_has_all_three_edit_classes() {
    let found = scenarios(&corpus_root());
    for part in PARTS.iter().filter(|p| **p != "cross") {
        for class in EditClass::ALL {
            assert!(
                found.iter().any(|s| s.part == *part && s.class == class),
                "the corpus has no {part}/{} scenario",
                class.dir()
            );
        }
    }
}

#[test]
fn corpus_is_certified() {
    let certification = certification();
    let table = certification.render();
    println!("{table}");
    assert!(
        certification.certified,
        "the modes diverge over the corpus; it is NOT certified:\n{table}"
    );
}

/// Malformed edits that still receive `Pass` in both modes: the extractor
/// tolerates the syntax errors and reports no gap, so nothing blocks. This
/// pins the behavior as measured; when a scenario stops being listed the
/// defect is fixed and the entry must go.
const MALFORMED_EDITS_THAT_STILL_PASS: [&str; 5] = [
    "cross/malformed [clean]",
    "go/malformed [clean]",
    "java/malformed [clean]",
    "kotlin/malformed [clean]",
    "rust/malformed [clean]",
];

#[test]
fn malformed_edits_that_still_receive_pass_are_pinned() {
    let mut actual = certification().false_passes();
    actual.sort();
    assert_eq!(
        actual, MALFORMED_EDITS_THAT_STILL_PASS,
        "the set of malformed edits that receive Pass changed; update the pin only if the change is intended"
    );
}

#[test]
fn a_planner_that_skips_required_reruns_is_caught() {
    let corpus = corpus_root();
    let scenario = scenarios(&corpus)
        .into_iter()
        .find(|s| s.part == "rust" && s.class == EditClass::Semantic)
        .expect("the rust semantic scenario exists");
    let result = evaluate_with_fault(
        &corpus,
        &scenario,
        Projection::WithoutHelmTemplates,
        Fault::SkipPlannedReruns,
    );
    let certification = certify(vec![result]);
    assert!(
        !certification.certified,
        "an incremental side that skips planned reruns must not be certified:\n{}",
        certification.render()
    );
}
