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

mod bench;
mod harness;
mod oracle;

use harness::{
    Certification, EditClass, Fault, PARTS, Projection, certify, corpus_root, evaluate,
    evaluate_with_fault, materialize, produce, scenarios,
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

/// Unparsable input must never yield `Pass`: the extractor reports each
/// broken file as a gap, so the verdict of a malformed edit blocks in both
/// modes, in every language of the corpus.
#[test]
fn no_malformed_edit_receives_pass() {
    let false_passes = certification().false_passes();
    assert!(
        false_passes.is_empty(),
        "malformed edits received Pass: {false_passes:?}"
    );
}

/// The extraction reads exactly the definitions the hand-labelled oracle
/// expects, in every scenario whose edit leaves the file parsable: no
/// definition is missing and none is invented. A malformed edit is left out
/// because what a damaged file yields is the grammar's recovery, not a
/// definition reading.
#[test]
fn the_extraction_matches_the_oracle_in_every_well_formed_scenario() {
    let corpus = corpus_root();
    let mut mismatches = Vec::new();
    for scenario in scenarios(&corpus)
        .into_iter()
        .filter(|s| s.class != EditClass::Malformed)
    {
        let produced = produce(&materialize(
            &corpus,
            &scenario,
            Projection::WithoutHelmTemplates,
        ));
        let score = oracle::score(
            &oracle::expected(&corpus, &scenario),
            &oracle::observed(&produced.head),
        );
        if !score.false_positives.is_empty() || !score.false_negatives.is_empty() {
            mismatches.push(format!(
                "{}: invented {:?}, missing {:?}",
                scenario.name(Projection::WithoutHelmTemplates),
                score.false_positives,
                score.false_negatives
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "the extraction disagrees with the oracle:\n{}",
        mismatches.join("\n")
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

#[test]
fn the_benchmark_reports_every_phase_and_agrees_across_modes() {
    let report = bench::report(&corpus_root(), 2, Some("rust"));
    let scenarios = report["scenarios"].as_array().expect("scenarios");
    assert_eq!(scenarios.len(), 3, "simple, semantic and malformed");
    for scenario in scenarios {
        assert_eq!(
            scenario["modes_agree_on_verdict_and_findings"], true,
            "{scenario}"
        );
        for pipeline in scenario["pipelines"]
            .as_object()
            .expect("pipelines")
            .values()
        {
            for phase in ["facts", "plan", "detect", "snapshot", "total"] {
                assert!(pipeline["phases"][phase]["p50_ms"].is_number(), "{phase}");
            }
            assert!(pipeline["time_to_first_feedback"]["p95_ms"].is_number());
        }
        let warm = &scenario["pipelines"]["incremental_complete.warm"]["cache"];
        assert!(
            warm["fact_files_reused"].as_u64() > Some(0),
            "a warm store must serve unchanged files: {warm}"
        );
    }
    assert!(report["not_evaluated"]["compilation_time"].is_string());
}

/// Writes the benchmark report. Run on demand:
/// `MODE_BENCH_RUNS=30 MODE_BENCH_OUT=report.json cargo test -p cognicode-core
/// --test it mode_equivalence::bench_report -- --ignored --exact --nocapture`.
#[test]
#[ignore = "benchmark: run on demand"]
fn bench_report() {
    let runs = std::env::var("MODE_BENCH_RUNS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(20);
    let report = bench::report(&corpus_root(), runs, None);
    let text = serde_json::to_string_pretty(&report).expect("serialize report");
    if let Ok(path) = std::env::var("MODE_BENCH_OUT") {
        std::fs::write(&path, &text).expect("write report");
    }
    println!("{text}");
}
