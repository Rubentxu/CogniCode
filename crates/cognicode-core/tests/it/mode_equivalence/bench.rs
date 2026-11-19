//! A benchmark of the two analysis modes that measures only what this code
//! can honestly measure.
//!
//! Each scenario is timed over `runs` repetitions of four pipelines: the two
//! modes (`incremental_complete` reuses the base revision's detector records,
//! `full_rebuild` reruns every detector) crossed with the two states of the
//! fact cache (`cold` parses every file, `warm` reads the objects a run of the
//! base revision left in a content-addressed store). Each repetition is
//! timed in phases:
//!
//! - `facts`: producing the head revision's facts;
//! - `plan`: listing the changes, the fact delta and the affected-work plan;
//! - `detect`: running or reusing the two production detectors;
//! - `snapshot`: composing the completeness verdict.
//!
//! `first_feedback` is the time from the start until the first detector has
//! reported. Everything outside the repetition (materializing the repository,
//! producing the base facts, priming the warm store) is not timed.
//!
//! Nothing here measures compilation, check publication, CI cost, branches or
//! worktrees: those are reported as `not_evaluated` with the reason, never
//! estimated. Memory is the process's resident set read from `/proc`, which
//! is a high-water mark of the whole test process, not of one pipeline; with
//! `isolate` every pipeline is also run in a process of its own (see
//! `isolated`) and its peak is reported under `memory`.

use std::collections::BTreeMap;
use std::fs;

use serde_json::{Value, json};

use cognicode_core::application::change_tracking::mode_benchmark::{
    BenchCache as Cache, BenchContext, BenchMode as Mode, BenchSample as Sample, run_pipeline_once,
    summarize_pipeline,
};
use cognicode_core::application::change_tracking::revision_facts::ProducedRevision;
use cognicode_core::infrastructure::findings::in_memory_evidence::InMemoryEvidenceStore;
use cognicode_core::infrastructure::git::GitScmAdapter;

use super::harness::{Fixture, Projection, Scenario, materialize, produce, workspace};
use super::{isolated, oracle};

/// The benchmark context of a fixture: its two commits, read through the Git
/// adapter the fixture holds.
pub fn context(fixture: &Fixture) -> BenchContext<'_, GitScmAdapter> {
    BenchContext::prepare(
        &fixture.adapter,
        &fixture.source,
        &fixture.base_sha,
        &fixture.head_sha,
        fixture.charts.as_ref(),
        workspace(),
        || Box::new(InMemoryEvidenceStore::new()),
    )
    .expect("prepare the benchmark context")
}

/// One repetition of a pipeline; a failed pipeline fails the benchmark.
pub fn run_once(ctx: &BenchContext<'_, GitScmAdapter>, mode: Mode, cache: Cache) -> Sample {
    run_pipeline_once(ctx, mode, cache).expect("run the pipeline")
}

fn proc_status_kib(field: &str) -> Option<u64> {
    fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix(field)?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
}

fn accuracy_report(
    corpus: &std::path::Path,
    scenario: &Scenario,
    head: &ProducedRevision,
) -> Value {
    let score = oracle::score(&oracle::expected(corpus, scenario), &oracle::observed(head));
    json!({
        "oracle": "hand-labelled function and type definitions (oracle/base.txt plus the overlay's ORACLE)",
        "true_positives": score.true_positives,
        "false_positives": score.false_positives,
        "false_negatives": score.false_negatives,
        "precision": score.precision(),
        "recall": score.recall(),
    })
}

/// The peak memory of one pipeline, measured in a process of its own. A
/// pipeline whose isolated run failed says so instead of reporting a number.
fn isolated_memory(scenario_name: &str, mode: Mode, cache: Cache, runs: usize) -> Value {
    isolated::spawn(&isolated::selector(scenario_name, mode, cache), runs)
        .unwrap_or_else(|reason| json!({ "unavailable": reason }))
}

/// Everything measured for one scenario.
pub fn scenario_report(
    corpus: &std::path::Path,
    scenario: &Scenario,
    projection: Projection,
    runs: usize,
    isolate: bool,
) -> Value {
    let fixture = materialize(corpus, scenario, projection);
    let produced = produce(&fixture);
    let accuracy = accuracy_report(corpus, scenario, &produced.head);
    let ctx = context(&fixture);

    let mut pipelines = serde_json::Map::new();
    let mut verdicts = BTreeMap::new();
    for mode in Mode::ALL {
        for cache in Cache::ALL {
            // One discarded repetition so the page cache and the allocator
            // are not what the first sample measures.
            let _ = run_once(&ctx, mode, cache);
            let samples: Vec<Sample> = (0..runs).map(|_| run_once(&ctx, mode, cache)).collect();
            let last = samples.last().expect("runs >= 1");
            verdicts.insert((mode.name(), cache.name()), (last.verdict, last.findings));
            let mut pipeline = summarize_pipeline(&samples).expect("runs >= 1");
            if isolate {
                pipeline["memory"] = isolated_memory(&scenario.name(projection), mode, cache, runs);
            }
            pipelines.insert(format!("{}.{}", mode.name(), cache.name()), pipeline);
        }
    }
    let reference = verdicts
        .get(&(Mode::FullRebuild.name(), Cache::Cold.name()))
        .copied();
    let modes_agree = verdicts.values().all(|v| Some(*v) == reference);

    json!({
        "scenario": scenario.name(projection),
        "modes_agree_on_verdict_and_findings": modes_agree,
        "pipelines": pipelines,
        "accuracy": accuracy,
        "rss_kib_after": proc_status_kib("VmRSS:"),
    })
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn meminfo_kib(field: &str) -> Option<u64> {
    fs::read_to_string("/proc/meminfo")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix(field)?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
}

fn load_average() -> Option<String> {
    let text = fs::read_to_string("/proc/loadavg").ok()?;
    Some(
        text.split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// The machine and build the numbers were taken on. The host is shared with
/// other builds, so the load average is part of the evidence.
fn machine(load_before: Option<String>) -> Value {
    json!({
        "logical_cpus": std::thread::available_parallelism().map(usize::from).ok(),
        "mem_total_kib": meminfo_kib("MemTotal:"),
        "mem_available_kib_at_end": meminfo_kib("MemAvailable:"),
        "load_average_before": load_before,
        "load_average_after": load_average(),
        "kernel": command_output("uname", &["-sr"]),
        "rustc": command_output("rustc", &["-V"]),
        "build_profile": if cfg!(debug_assertions) { "test/dev (debug assertions on, unoptimized)" } else { "optimized" },
        "shared_host": "the host is shared with other builds and agents; absolute times are noisy, only relative shape is meaningful",
    })
}

/// What this benchmark does not measure, and why. Never estimated.
fn not_evaluated(isolated_memory: bool) -> Value {
    let mut reasons = json!({
        "compilation_time": "no compilation happens in this pipeline; scripts/measure/scenario_cost.py corpus-gates times the cargo stages on the same corpus",
        "check_publication_time": "publishing a check run belongs to the CI and gate tooling, not to this crate; scripts/measure/scenario_cost.py pr-checks reads hosted check durations back from the API",
        "ci_cost_per_scenario": "not derivable from analysis time; scripts/measure/scenario_cost.py corpus-gates runs the gate stages per scenario, and the PipelineK lane is not run by it",
        "branch_and_worktree_scenarios": "the pipelines compare two commits in one repository and do not exercise git; scripts/measure/scenario_cost.py worktree times branch and worktree operations",
        "speedup_claims": "no speedup is claimed: the numbers above are per pipeline, taken in a debug build on a shared host",
        "detection_precision_of_terraform_and_helm": "the benchmark checks mode agreement, not precision against a hand-labelled infrastructure oracle",
    });
    if !isolated_memory {
        reasons["peak_memory_per_pipeline"] = json!(
            "only the resident set of the whole test process is read, so it cannot be attributed to a pipeline; rerun with MODE_BENCH_ISOLATE=1"
        );
    }
    reasons
}

/// The whole report: machine, command, every measured scenario and the limits.
pub fn report(
    corpus: &std::path::Path,
    runs: usize,
    only_part: Option<&str>,
    isolate: bool,
) -> Value {
    let load_before = load_average();
    let scenarios: Vec<Value> = super::harness::scenarios(corpus)
        .iter()
        .filter(|s| only_part.is_none_or(|part| s.part == part))
        .map(|s| scenario_report(corpus, s, Projection::Rendered, runs, isolate))
        .collect();
    json!({
        "benchmark": "cgh-mode-equivalence",
        "command": "MODE_BENCH_RUNS=<n> MODE_BENCH_ISOLATE=<1 for per-pipeline memory> MODE_BENCH_OUT=<file> CARGO_BUILD_JOBS=4 cargo test -p cognicode-core --test it mode_equivalence::bench_report -- --ignored --exact --nocapture",
        "runs_per_pipeline": runs,
        "percentiles": "nearest-rank over the runs; with fewer than 20 runs the p95 is the maximum",
        "projection": "the Helm chart is rendered by a labelled deterministic double of the engine (no helm binary needed), so a revision can reach Pass; the corpus itself is unaltered",
        "machine": machine(load_before),
        "peak_rss_kib_process_high_water_mark": proc_status_kib("VmHWM:"),
        "scenarios": scenarios,
        "memory_isolation": if isolate { "each pipeline ran in a fresh process; `memory` holds its own peak" } else { "off" },
        "not_evaluated": not_evaluated(isolate),
    })
}
