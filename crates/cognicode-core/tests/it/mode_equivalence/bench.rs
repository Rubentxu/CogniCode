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
//! is a high-water mark of the whole test process, not of one pipeline.

use std::collections::BTreeMap;
use std::fs;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use cognicode_core::application::change_tracking::complete_snapshot::{
    AnalysisMode, CompleteSnapshotRequest, Verdict, build_complete_snapshot,
};
use cognicode_core::application::change_tracking::detector_runs::{
    Detection, ProductionDetector, Reuse, current_identity, detect_with_reuse,
};
use cognicode_core::application::change_tracking::revision_facts::{
    FactObjectStore, FactsLimits, ProducedRevision, dependencies_on, produce_revision_facts,
};
use cognicode_core::application::change_tracking::scm_delta::{
    delta_from_change_set, plan_between,
};
use cognicode_core::domain::findings::detector_reuse::{DetectorReuseIndex, DetectorRunRecord};
use cognicode_core::domain::scm::{ChangeReader, RevisionReader, SourceRevision};
use cognicode_core::infrastructure::findings::in_memory_evidence::InMemoryEvidenceStore;

use super::harness::{
    Fixture, Produced, Projection, Scenario, fresh_record, gaps_of, identities, materialize,
    produce, workspace,
};
use super::oracle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    IncrementalComplete,
    FullRebuild,
}

impl Mode {
    pub const ALL: [Self; 2] = [Self::IncrementalComplete, Self::FullRebuild];

    pub fn name(self) -> &'static str {
        match self {
            Self::IncrementalComplete => "incremental_complete",
            Self::FullRebuild => "full_rebuild",
        }
    }

    fn analysis(self) -> AnalysisMode {
        match self {
            Self::IncrementalComplete => AnalysisMode::IncrementalComplete,
            Self::FullRebuild => AnalysisMode::FullRebuild,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cache {
    Cold,
    Warm,
}

impl Cache {
    pub const ALL: [Self; 2] = [Self::Cold, Self::Warm];

    pub fn name(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Warm => "warm",
        }
    }
}

/// One repetition of one pipeline.
#[derive(Debug, Clone)]
pub struct Sample {
    pub facts: Duration,
    pub plan: Duration,
    pub detect: Duration,
    pub snapshot: Duration,
    pub first_feedback: Duration,
    pub total: Duration,
    pub parsed_files: usize,
    pub reused_files: usize,
    pub detectors_reused: usize,
    pub detectors_run: usize,
    pub verdict: Verdict,
    pub findings: usize,
}

/// What one scenario needs that is not part of what is timed.
pub struct Context {
    fixture: Fixture,
    base_revision: SourceRevision,
    head_revision: SourceRevision,
    base: ProducedRevision,
    prior: DetectorReuseIndex,
}

pub fn context(fixture: Fixture, produced: &Produced) -> Context {
    let resolve = |sha: &str| {
        fixture
            .adapter
            .resolve(&fixture.source, sha)
            .expect("resolve revision")
    };
    let mut prior = DetectorReuseIndex::empty();
    for detector in ProductionDetector::ALL {
        prior.remember(fresh_record(detector, &produced.base));
    }
    Context {
        base_revision: resolve(&fixture.base_sha),
        head_revision: resolve(&fixture.head_sha),
        base: produced.base.clone(),
        prior,
        fixture,
    }
}

/// A store holding the objects of the base revision, as a previous run leaves it.
fn primed_store(ctx: &Context) -> (tempfile::TempDir, FactObjectStore) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = FactObjectStore::new(dir.path());
    produce_revision_facts(
        &ctx.fixture.adapter,
        &ctx.base_revision,
        Some(&store),
        &FactsLimits::default(),
    )
    .expect("prime the store");
    (dir, store)
}

pub fn run_once(ctx: &Context, mode: Mode, cache: Cache) -> Sample {
    let primed = (cache == Cache::Warm).then(|| primed_store(ctx));
    let limits = FactsLimits::default();
    let started = Instant::now();

    let head = produce_revision_facts(
        &ctx.fixture.adapter,
        &ctx.head_revision,
        primed.as_ref().map(|(_, store)| store),
        &limits,
    )
    .expect("head facts");
    let facts = started.elapsed();

    let planning = Instant::now();
    let changes = ctx
        .fixture
        .adapter
        .changes_between(&ctx.base_revision, &ctx.head_revision)
        .expect("changes between");
    let manifests = ctx.base.manifests.union(&head.manifests).cloned().collect();
    let dependencies = dependencies_on(&ctx.prior, ctx.base.facts.snapshot);
    let plan = plan_between(
        &ctx.base.facts,
        &head.facts,
        &changes,
        &manifests,
        &dependencies,
    )
    .expect("plan between");
    let delta = delta_from_change_set(&ctx.base.facts, &head.facts, &changes)
        .expect("delta")
        .delta;
    let plan_time = planning.elapsed();

    let detecting = Instant::now();
    let mut first_feedback = None;
    let mut reruns: BTreeMap<String, DetectorRunRecord> = BTreeMap::new();
    let mut detectors_reused = 0;
    for detector in ProductionDetector::ALL {
        let key = current_identity(detector)
            .expect("identity")
            .detector_id
            .as_str()
            .to_string();
        match mode {
            Mode::IncrementalComplete => {
                let prior = ctx.prior.record_for(&key, ctx.base.facts.snapshot);
                let Detection { reuse, fresh, .. } = detect_with_reuse(
                    detector,
                    prior,
                    &ctx.base,
                    &head,
                    &changes,
                    &workspace(),
                    &mut InMemoryEvidenceStore::new(),
                )
                .expect("detect with reuse");
                if matches!(reuse, Reuse::Reused { .. }) {
                    detectors_reused += 1;
                }
                if let Some(record) = fresh {
                    reruns.insert(key, record);
                }
            }
            Mode::FullRebuild => {
                reruns.insert(key, fresh_record(detector, &head));
            }
        }
        first_feedback.get_or_insert_with(|| started.elapsed());
    }
    let detect = detecting.elapsed();

    let snapshotting = Instant::now();
    let ids = identities();
    let gaps = gaps_of(&head);
    let snapshot = build_complete_snapshot(&CompleteSnapshotRequest {
        mode: mode.analysis(),
        plan: &plan,
        candidate_snapshot: head.facts.snapshot,
        delta: &delta,
        cache: &ctx.prior,
        current_identities: &ids,
        reruns: &reruns,
        gaps: &gaps,
    });
    let snapshot_time = snapshotting.elapsed();
    let total = started.elapsed();

    Sample {
        facts,
        plan: plan_time,
        detect,
        snapshot: snapshot_time,
        first_feedback: first_feedback.unwrap_or(total),
        total,
        parsed_files: head.coverage.parsed_files,
        reused_files: head.coverage.reused_files,
        detectors_reused,
        detectors_run: reruns.len(),
        verdict: snapshot.verdict(),
        findings: snapshot.findings.len(),
    }
}

/// Nearest-rank percentile of `values` (`p` in 1..=100); `None` when empty.
pub fn percentile(values: &[f64], p: usize) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let rank = (p * sorted.len()).div_ceil(100).max(1);
    Some(sorted[rank - 1])
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn stats(values: &[f64]) -> Value {
    #[allow(clippy::cast_precision_loss)]
    let mean = values.iter().sum::<f64>() / values.len().max(1) as f64;
    json!({
        "n": values.len(),
        "p50_ms": percentile(values, 50),
        "p95_ms": percentile(values, 95),
        "min_ms": percentile(values, 1),
        "max_ms": percentile(values, 100),
        "mean_ms": mean,
    })
}

fn phase(samples: &[Sample], pick: impl Fn(&Sample) -> Duration) -> Value {
    stats(&samples.iter().map(|s| millis(pick(s))).collect::<Vec<_>>())
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

fn pipeline_report(samples: &[Sample]) -> Value {
    let last = samples.last().expect("at least one sample");
    json!({
        "runs": samples.len(),
        "phases": {
            "facts": phase(samples, |s| s.facts),
            "plan": phase(samples, |s| s.plan),
            "detect": phase(samples, |s| s.detect),
            "snapshot": phase(samples, |s| s.snapshot),
            "total": phase(samples, |s| s.total),
        },
        "time_to_first_feedback": phase(samples, |s| s.first_feedback),
        "cache": {
            "fact_files_parsed": last.parsed_files,
            "fact_files_reused": last.reused_files,
            "detectors_reused": last.detectors_reused,
            "detectors_run": last.detectors_run,
        },
        "verdict": format!("{:?}", last.verdict),
        "findings": last.findings,
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

/// Everything measured for one scenario.
pub fn scenario_report(
    corpus: &std::path::Path,
    scenario: &Scenario,
    projection: Projection,
    runs: usize,
) -> Value {
    let fixture = materialize(corpus, scenario, projection);
    let produced = produce(&fixture);
    let accuracy = accuracy_report(corpus, scenario, &produced.head);
    let ctx = context(fixture, &produced);

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
            pipelines.insert(
                format!("{}.{}", mode.name(), cache.name()),
                pipeline_report(&samples),
            );
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
fn not_evaluated() -> Value {
    json!({
        "compilation_time": "no compilation happens in this pipeline; measuring cargo needs a separate, external benchmark",
        "check_publication_time": "publishing a check run belongs to the CI and gate tooling, not to this crate",
        "ci_cost_per_scenario": "needs the CI gates run per scenario; not derivable from analysis time",
        "branch_and_worktree_scenarios": "the pipelines compare two commits in one repository; branch switching and worktree creation are not exercised",
        "peak_memory_per_pipeline": "only the resident set of the whole test process is read, so it cannot be attributed to a pipeline",
        "speedup_claims": "no speedup is claimed: the numbers above are per pipeline, taken in a debug build on a shared host",
        "detection_precision_of_terraform_and_helm": "the production detectors read only function and type definitions, so nothing is detected in these parts",
    })
}

/// The whole report: machine, command, every measured scenario and the limits.
pub fn report(corpus: &std::path::Path, runs: usize, only_part: Option<&str>) -> Value {
    let load_before = load_average();
    let scenarios: Vec<Value> = super::harness::scenarios(corpus)
        .iter()
        .filter(|s| only_part.is_none_or(|part| s.part == part))
        .map(|s| scenario_report(corpus, s, Projection::WithoutHelmTemplates, runs))
        .collect();
    json!({
        "benchmark": "cgh-mode-equivalence",
        "command": "MODE_BENCH_RUNS=<n> MODE_BENCH_OUT=<file> CARGO_BUILD_JOBS=4 cargo test -p cognicode-core --test it mode_equivalence::bench_report -- --ignored --exact --nocapture",
        "runs_per_pipeline": runs,
        "percentiles": "nearest-rank over the runs; with fewer than 20 runs the p95 is the maximum",
        "projection": "corpus without helm/chart/templates, so a revision can reach Pass",
        "machine": machine(load_before),
        "peak_rss_kib_process_high_water_mark": proc_status_kib("VmHWM:"),
        "scenarios": scenarios,
        "not_evaluated": not_evaluated(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_percentiles() {
        let values: Vec<f64> = (1..=20).map(f64::from).collect();
        assert_eq!(percentile(&values, 50), Some(10.0));
        assert_eq!(percentile(&values, 95), Some(19.0));
        assert_eq!(percentile(&values, 100), Some(20.0));
        assert_eq!(percentile(&[7.0], 95), Some(7.0));
        assert_eq!(percentile(&[], 50), None);
    }

    #[test]
    fn percentiles_do_not_depend_on_input_order() {
        assert_eq!(percentile(&[3.0, 1.0, 2.0], 50), Some(2.0));
    }
}
