//! Do `IncrementalComplete` and `FullRebuild` agree over a real corpus?
//!
//! The corpus is `testdata/mode_equivalence`: a Rust, a Java, a Kotlin, a
//! Go, a Terraform and a Helm part, each edited three ways (simple,
//! semantic and malformed), plus edits that cross parts. The harness in
//! `mode_equivalence/harness.rs` commits each edit into a throwaway Git
//! repository and compares the two modes through the production seams.
//!
//! Every scenario is judged twice over the same, unaltered corpus. A Helm
//! template is not YAML until rendered, so the chart is judged once with
//! nothing rendering it (its templates are gaps and no verdict can be
//! `Pass`) and once with a labelled double of the engine that renders it,
//! so the comparison also covers the `Pass` path. A test with the real
//! `helm` binary covers the production adapter.
//!
//! `corpus_is_certified` is the loud one: any divergence between the modes
//! fails it and prints the table that says where.

mod bench;
mod harness;
mod isolated;
mod oracle;

use cognicode_core::application::change_tracking::complete_snapshot::Verdict;
use harness::{
    Certification, EditClass, Fault, PARTS, Projection, ScenarioResult, certify, corpus_root,
    evaluate, evaluate_with_fault, materialize, produce, scenarios,
};

fn helm_scenario(class: EditClass, projection: Projection) -> ScenarioResult {
    let corpus = corpus_root();
    let scenario = scenarios(&corpus)
        .into_iter()
        .find(|s| s.part == "helm" && s.class == class)
        .expect("the helm scenario exists");
    evaluate(&corpus, &scenario, projection)
}

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

/// A chart whose templates were rendered, with nothing else wrong, reaches
/// `Pass` in both modes. The corpus is unaltered: its templates are in the
/// revision and are judged by the render, not removed from the view.
#[test]
fn a_rendered_chart_with_nothing_else_wrong_reaches_pass() {
    let result = helm_scenario(EditClass::Simple, Projection::Rendered);

    assert!(result.divergences.is_empty(), "{:?}", result.divergences);
    assert_eq!(result.head_gaps, Vec::<String>::new());
    assert_eq!(result.incremental_verdict, Verdict::Pass);
    assert_eq!(result.full_verdict, Verdict::Pass);
}

/// With nothing rendering the chart, its templates are gaps and the same
/// edit is never `Pass`, in either mode. The cause is the render, not a
/// syntax error.
#[test]
fn an_unrendered_chart_is_never_pass_and_says_why() {
    for class in EditClass::ALL {
        let result = helm_scenario(class, Projection::Unrendered);

        assert_ne!(result.incremental_verdict, Verdict::Pass, "{class:?}");
        assert_ne!(result.full_verdict, Verdict::Pass, "{class:?}");
        assert!(
            result
                .head_gaps
                .iter()
                .any(|gap| gap.contains("not rendered")),
            "{class:?}: {:?}",
            result.head_gaps
        );
        assert!(
            result
                .head_gaps
                .iter()
                .all(|gap| !gap.contains("Syntax errors")),
            "a template is not a syntax error: {:?}",
            result.head_gaps
        );
    }
}

/// A template with an unclosed action is not rendered by anything, so the
/// edit is never `Pass` even when other templates are rendered.
#[test]
fn a_malformed_template_is_never_pass_even_when_the_engine_is_present() {
    let result = helm_scenario(EditClass::Malformed, Projection::Rendered);

    assert!(result.divergences.is_empty(), "{:?}", result.divergences);
    assert_ne!(result.incremental_verdict, Verdict::Pass);
    assert_ne!(result.full_verdict, Verdict::Pass);
    assert!(
        result
            .head_gaps
            .iter()
            .any(|gap| gap.contains("unclosed action")),
        "{:?}",
        result.head_gaps
    );
}

/// The production adapter over the real engine. Skipped loudly without
/// `helm`; the helm lane requires it. The simple edit changes a value the
/// chart sets, so the chart renders completely and reaches `Pass`.
#[test]
fn helm_chart_rendered_by_the_real_engine_reaches_pass() {
    if crate::helm_model::real_helm().is_none() {
        return;
    }

    let result = helm_scenario(EditClass::Simple, Projection::RealEngine);

    assert!(result.divergences.is_empty(), "{:?}", result.divergences);
    assert_eq!(result.head_gaps, Vec::<String>::new());
    assert_eq!(result.incremental_verdict, Verdict::Pass);
    assert_eq!(result.full_verdict, Verdict::Pass);

    let broken = helm_scenario(EditClass::Malformed, Projection::RealEngine);
    assert_ne!(broken.incremental_verdict, Verdict::Pass);
    assert_ne!(broken.full_verdict, Verdict::Pass);
    assert!(!broken.head_gaps.is_empty());
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
        let produced = produce(&materialize(&corpus, &scenario, Projection::Rendered));
        let score = oracle::score(
            &oracle::expected(&corpus, &scenario),
            &oracle::observed(&produced.head),
        );
        if !score.false_positives.is_empty() || !score.false_negatives.is_empty() {
            mismatches.push(format!(
                "{}: invented {:?}, missing {:?}",
                scenario.name(Projection::Rendered),
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
        Projection::Rendered,
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
    let report = bench::report(&corpus_root(), 2, Some("rust"), false);
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

/// What one isolated process runs: the pipeline `MODE_BENCH_ISOLATED` names,
/// reported on one marked line. Spawned by the benchmark, not run by hand.
#[test]
#[ignore = "entry point of the isolated-process memory measurement"]
fn bench_isolated_pipeline() {
    let selected = std::env::var(isolated::SELECTOR_ENV).expect("MODE_BENCH_ISOLATED is set");
    let runs = std::env::var("MODE_BENCH_RUNS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1);
    let report = isolated::measure_selected(&corpus_root(), &selected, runs);
    println!("{}", isolated::render(&report));
}

/// Every pipeline of a scenario reports a peak of its own, taken in a fresh
/// process, and a selector that names nothing is not reported as a number.
#[test]
fn each_pipeline_reports_the_peak_memory_of_a_process_of_its_own() {
    let report = bench::report(&corpus_root(), 1, Some("rust"), true);
    for scenario in report["scenarios"].as_array().expect("scenarios") {
        let pipelines = scenario["pipelines"].as_object().expect("pipelines");
        assert_eq!(pipelines.len(), 4, "two modes crossed with two caches");
        for (name, pipeline) in pipelines {
            let memory = &pipeline["memory"];
            assert!(memory["peak_kib"].as_u64() > Some(0), "{name}: {memory}");
            assert!(
                memory["peak_kib"].as_u64() >= memory["rss_after_setup_kib"].as_u64(),
                "{name}: {memory}"
            );
            assert_eq!(memory["pipeline"], name.as_str(), "{memory}");
        }
    }
    assert!(report["not_evaluated"]["peak_memory_per_pipeline"].is_null());

    let refused = isolated::spawn("no/such#full_rebuild.cold", 1);
    assert!(refused.is_err(), "{refused:?}");
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
    let isolate = std::env::var("MODE_BENCH_ISOLATE").is_ok_and(|value| value == "1");
    let report = bench::report(&corpus_root(), runs, None, isolate);
    let text = serde_json::to_string_pretty(&report).expect("serialize report");
    if let Ok(path) = std::env::var("MODE_BENCH_OUT") {
        std::fs::write(&path, &text).expect("write report");
    }
    println!("{text}");
}
