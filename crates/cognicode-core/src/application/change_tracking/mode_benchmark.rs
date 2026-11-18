//! Supported entry points of the mode-equivalence benchmark.
//!
//! The benchmark times, for one pair of revisions, four pipelines: the two
//! analysis modes (`incremental_complete` reuses the base revision's detector
//! records, `full_rebuild` reruns every detector) crossed with the two states
//! of the fact cache (`cold` parses every file, `warm` reads the objects a run
//! of the base revision left in a content-addressed store). A repetition is
//! timed in phases:
//!
//! - `facts`: producing the head revision's facts;
//! - `plan`: listing the changes, the fact delta and the affected-work plan;
//! - `detect`: running or reusing the production detectors;
//! - `snapshot`: composing the completeness verdict.
//!
//! `first_feedback` is the time from the start until the first detector has
//! reported. Whatever happens in [`BenchContext::prepare`] (resolving the
//! revisions, producing the base facts, running the base detectors) is not
//! timed.
//!
//! The pipelines run the production seams (`produce_revision_facts_rendering`,
//! `plan_between`, `detect_with_reuse`, `build_complete_snapshot`); nothing
//! here is a second analysis. The caller supplies the repository: any source
//! that reads revisions and changes can be measured, and a Git checkout is the
//! usual one.
//!
//! What this does not measure is not estimated: compilation, check
//! publication, CI, branches and worktrees are out of scope, and the resident
//! set of a process is a high-water mark of the whole process, so a per
//! pipeline peak needs a process of its own, which is the caller's to spawn.
//!
//! # Compatibility
//! [`PIPELINE_SUMMARY_SCHEMA`] versions the JSON of [`summarize_pipeline`].
//! Fields are only ever added within a schema version; a reader ignores the
//! fields it does not know.
//!
//! The corpus the benchmark of this repository runs over lives in the test
//! tree of the source checkout and is not part of the published crate.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::complete_snapshot::{
    AnalysisMode, CaptureGap, CompleteSnapshotRequest, ReasonCode, Verdict, build_complete_snapshot,
};
use super::detector_runs::{
    Detection, DetectorRunError, ProductionDetector, Reuse, current_identity, detect_with_reuse,
    run_detector,
};
use super::helm_templates::ChartRenderCheck;
use super::revision_facts::{
    FactObjectStore, FactsLimits, ProducedRevision, dependencies_on,
    produce_revision_facts_rendering,
};
use super::scm_delta::{ScmDeltaError, delta_from_change_set, plan_between};
use crate::domain::findings::detector_reuse::{
    DetectorReuseIndex, DetectorRunIdentity, DetectorRunRecord,
};
use crate::domain::findings::ports::EvidenceSink;
use crate::domain::scm::{ChangeReader, RevisionReader, ScmError, SourceRef, SourceRevision};
use crate::domain::value_objects::WorkspaceId;

/// Version of the JSON that [`summarize_pipeline`] produces.
pub const PIPELINE_SUMMARY_SCHEMA: &str = "cognicode.mode-benchmark/v1";

/// The analysis mode a pipeline runs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchMode {
    /// Reuses the base revision's detector records where the plan allows.
    IncrementalComplete,
    /// Reruns every detector.
    FullRebuild,
}

impl BenchMode {
    /// Every mode, in report order.
    pub const ALL: [Self; 2] = [Self::IncrementalComplete, Self::FullRebuild];

    /// The stable name of the mode in reports.
    #[must_use]
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

/// The state of the fact cache a pipeline starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchCache {
    /// Every file of the head revision is parsed.
    Cold,
    /// The objects of a run over the base revision are already stored.
    Warm,
}

impl BenchCache {
    /// Every cache state, in report order.
    pub const ALL: [Self; 2] = [Self::Cold, Self::Warm];

    /// The stable name of the cache state in reports.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Warm => "warm",
        }
    }
}

/// One repetition of one pipeline.
#[derive(Debug, Clone)]
pub struct BenchSample {
    /// Producing the head revision's facts.
    pub facts: Duration,
    /// Changes, fact delta and affected-work plan.
    pub plan: Duration,
    /// Running or reusing the production detectors.
    pub detect: Duration,
    /// Composing the completeness verdict.
    pub snapshot: Duration,
    /// From the start until the first detector reported.
    pub first_feedback: Duration,
    /// The whole repetition.
    pub total: Duration,
    /// Files of the head revision parsed in this repetition.
    pub parsed_files: usize,
    /// Files whose facts came from the object store.
    pub reused_files: usize,
    /// Detectors whose base record was reused.
    pub detectors_reused: usize,
    /// Detectors that ran.
    pub detectors_run: usize,
    /// The completeness verdict of the repetition.
    pub verdict: Verdict,
    /// Findings the verdict carries.
    pub findings: usize,
}

/// Why a pipeline could not be measured. A failed repetition is an error,
/// never a sample with zeroes.
#[derive(Debug, thiserror::Error)]
pub enum BenchError {
    /// A revision or its content could not be read.
    #[error("source read failed: {0}")]
    Source(#[from] ScmError),
    /// The fact delta between the revisions could not be computed.
    #[error("fact delta failed: {0}")]
    Delta(#[from] ScmDeltaError),
    /// A production detector could not be admitted or run.
    #[error("detector run failed: {0}")]
    Detector(#[from] DetectorRunError),
    /// The scratch directory of the warm cache could not be created.
    #[error("scratch directory unavailable: {0}")]
    Scratch(String),
}

/// Makes the sink a detector run writes its evidence to. The benchmark does
/// not keep that evidence, so the caller chooses the cheapest sink it has (an
/// in-memory store is enough); the choice is the caller's because adapters are
/// reached from the edge, not from the application layer.
pub type SinkFactory = fn() -> Box<dyn EvidenceSink>;

/// What a pair of revisions needs that is not part of what is timed.
pub struct BenchContext<'a, S> {
    source: &'a S,
    charts: &'a dyn ChartRenderCheck,
    workspace: WorkspaceId,
    new_sink: SinkFactory,
    base_revision: SourceRevision,
    head_revision: SourceRevision,
    base: ProducedRevision,
    prior: DetectorReuseIndex,
}

impl<'a, S: RevisionReader + ChangeReader> BenchContext<'a, S> {
    /// Resolves both references, produces the base revision's facts cold and
    /// runs every production detector over it, as a previous run would have
    /// left them.
    ///
    /// # Errors
    /// A reference does not resolve, the base facts cannot be produced, or a
    /// detector cannot run.
    pub fn prepare(
        source: &'a S,
        source_ref: &SourceRef,
        base_ref: &str,
        head_ref: &str,
        charts: &'a dyn ChartRenderCheck,
        workspace: WorkspaceId,
        new_sink: SinkFactory,
    ) -> Result<Self, BenchError> {
        let base_revision = source.resolve(source_ref, base_ref)?;
        let head_revision = source.resolve(source_ref, head_ref)?;
        let base = produce_revision_facts_rendering(
            source,
            &base_revision,
            None,
            &FactsLimits::default(),
            charts,
        )?;
        let mut prior = DetectorReuseIndex::empty();
        for detector in ProductionDetector::ALL {
            prior.remember(fresh_record(detector, &base, &workspace, &mut *new_sink())?);
        }
        Ok(Self {
            source,
            charts,
            workspace,
            new_sink,
            base_revision,
            head_revision,
            base,
            prior,
        })
    }

    /// The facts of the base revision, as produced by `prepare`.
    #[must_use]
    pub fn base(&self) -> &ProducedRevision {
        &self.base
    }
}

/// A store holding the objects of the base revision, as a previous run leaves it.
fn primed_store<S: RevisionReader + ChangeReader>(
    ctx: &BenchContext<'_, S>,
) -> Result<(tempfile::TempDir, FactObjectStore), BenchError> {
    let dir = tempfile::tempdir().map_err(|error| BenchError::Scratch(error.to_string()))?;
    let store = FactObjectStore::new(dir.path());
    produce_revision_facts_rendering(
        ctx.source,
        &ctx.base_revision,
        Some(&store),
        &FactsLimits::default(),
        ctx.charts,
    )?;
    Ok((dir, store))
}

/// Runs one repetition of the pipeline `mode` x `cache` and times its phases.
///
/// # Errors
/// Any step of the pipeline failed; no partial sample is returned.
pub fn run_pipeline_once<S: RevisionReader + ChangeReader>(
    ctx: &BenchContext<'_, S>,
    mode: BenchMode,
    cache: BenchCache,
) -> Result<BenchSample, BenchError> {
    let primed = (cache == BenchCache::Warm)
        .then(|| primed_store(ctx))
        .transpose()?;
    let started = Instant::now();

    let head = produce_revision_facts_rendering(
        ctx.source,
        &ctx.head_revision,
        primed.as_ref().map(|(_, store)| store),
        &FactsLimits::default(),
        ctx.charts,
    )?;
    let facts = started.elapsed();

    let planning = Instant::now();
    let changes = ctx
        .source
        .changes_between(&ctx.base_revision, &ctx.head_revision)?;
    let manifests = ctx.base.manifests.union(&head.manifests).cloned().collect();
    let dependencies = dependencies_on(&ctx.prior, ctx.base.facts.snapshot);
    let plan = plan_between(
        &ctx.base.facts,
        &head.facts,
        &changes,
        &manifests,
        &dependencies,
    )?;
    let delta = delta_from_change_set(&ctx.base.facts, &head.facts, &changes)?.delta;
    let plan_time = planning.elapsed();

    let detecting = Instant::now();
    let mut first_feedback = None;
    let mut reruns: BTreeMap<String, DetectorRunRecord> = BTreeMap::new();
    let mut detectors_reused = 0;
    for detector in ProductionDetector::ALL {
        let key = current_identity(detector)?.detector_id.as_str().to_string();
        match mode {
            BenchMode::IncrementalComplete => {
                let prior = ctx.prior.record_for(&key, ctx.base.facts.snapshot);
                let Detection { reuse, fresh, .. } = detect_with_reuse(
                    detector,
                    prior,
                    &ctx.base,
                    &head,
                    &changes,
                    &ctx.workspace,
                    &mut *(ctx.new_sink)(),
                )?;
                if matches!(reuse, Reuse::Reused { .. }) {
                    detectors_reused += 1;
                }
                if let Some(record) = fresh {
                    reruns.insert(key, record);
                }
            }
            BenchMode::FullRebuild => {
                reruns.insert(
                    key,
                    fresh_record(detector, &head, &ctx.workspace, &mut *(ctx.new_sink)())?,
                );
            }
        }
        first_feedback.get_or_insert_with(|| started.elapsed());
    }
    let detect = detecting.elapsed();

    let snapshotting = Instant::now();
    let identities = current_identities()?;
    let gaps = capture_gaps(&head);
    let snapshot = build_complete_snapshot(&CompleteSnapshotRequest {
        mode: mode.analysis(),
        plan: &plan,
        candidate_snapshot: head.facts.snapshot,
        delta: &delta,
        cache: &ctx.prior,
        current_identities: &identities,
        reruns: &reruns,
        gaps: &gaps,
    });
    let snapshot_time = snapshotting.elapsed();
    let total = started.elapsed();

    Ok(BenchSample {
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
    })
}

/// The identity every production detector would carry if it ran now, by id.
///
/// # Errors
/// A detector could not be admitted.
pub fn current_identities() -> Result<BTreeMap<String, DetectorRunIdentity>, DetectorRunError> {
    ProductionDetector::ALL
        .into_iter()
        .map(|detector| {
            let identity = current_identity(detector)?;
            Ok((identity.detector_id.as_str().to_string(), identity))
        })
        .collect()
}

/// The files of `head` that could not be read, as gaps of the completeness
/// claim.
#[must_use]
pub fn capture_gaps(head: &ProducedRevision) -> Vec<CaptureGap> {
    head.coverage
        .gaps
        .iter()
        .map(|gap| CaptureGap {
            code: ReasonCode::ParseError,
            detail: format!("{}: {}", gap.path.as_str(), gap.reason.describe()),
        })
        .collect()
}

/// A fresh run of `detector` over `revision`, whatever was cached, writing
/// its evidence to `sink`.
///
/// # Errors
/// The detector could not be admitted or run.
pub fn fresh_record(
    detector: ProductionDetector,
    revision: &ProducedRevision,
    workspace: &WorkspaceId,
    sink: &mut dyn EvidenceSink,
) -> Result<DetectorRunRecord, DetectorRunError> {
    run_detector(detector, revision, workspace, sink)
}

/// Nearest-rank percentile of `values` (`p` in 1..=100); `None` when empty.
///
/// With fewer than `100 / (100 - p)` values the high percentiles are the
/// maximum; a caller that reports p95 should measure at least 20 runs.
#[must_use]
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

fn phase(samples: &[BenchSample], pick: impl Fn(&BenchSample) -> Duration) -> Value {
    stats(&samples.iter().map(|s| millis(pick(s))).collect::<Vec<_>>())
}

/// Summarizes the repetitions of one pipeline: per-phase statistics, the
/// cache counters and the verdict of the last repetition. `None` for no
/// repetitions: a summary of nothing must not look like a measurement.
///
/// The JSON carries [`PIPELINE_SUMMARY_SCHEMA`] under `schema`.
#[must_use]
pub fn summarize_pipeline(samples: &[BenchSample]) -> Option<Value> {
    let last = samples.last()?;
    Some(json!({
        "schema": PIPELINE_SUMMARY_SCHEMA,
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
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(total_ms: u64, verdict: Verdict) -> BenchSample {
        let d = Duration::from_millis(total_ms);
        BenchSample {
            facts: d,
            plan: d,
            detect: d,
            snapshot: d,
            first_feedback: d,
            total: d,
            parsed_files: 3,
            reused_files: 1,
            detectors_reused: 2,
            detectors_run: 1,
            verdict,
            findings: 4,
        }
    }

    #[test]
    fn percentiles_do_not_depend_on_input_order_and_the_top_is_the_maximum() {
        let values: Vec<f64> = (1..=20).map(f64::from).collect();
        assert_eq!(percentile(&values, 50), Some(10.0));
        assert_eq!(percentile(&values, 95), Some(19.0));
        assert_eq!(percentile(&values, 100), Some(20.0));
        assert_eq!(percentile(&[7.0], 95), Some(7.0));
        assert_eq!(percentile(&[3.0, 1.0, 2.0], 50), Some(2.0));
    }

    #[test]
    fn percentiles_are_nearest_rank() {
        let values = [5.0, 1.0, 4.0, 2.0, 3.0];
        assert_eq!(percentile(&values, 50), Some(3.0));
        assert_eq!(percentile(&values, 95), Some(5.0));
        assert_eq!(percentile(&values, 1), Some(1.0));
        assert_eq!(percentile(&[], 50), None);
    }

    #[test]
    fn names_are_the_wire_names() {
        assert_eq!(
            BenchMode::IncrementalComplete.name(),
            "incremental_complete"
        );
        assert_eq!(BenchMode::FullRebuild.name(), "full_rebuild");
        assert_eq!(BenchCache::Cold.name(), "cold");
        assert_eq!(BenchCache::Warm.name(), "warm");
    }

    #[test]
    fn a_summary_of_nothing_is_not_a_measurement() {
        assert!(summarize_pipeline(&[]).is_none());
    }

    #[test]
    fn a_summary_is_versioned_and_reports_the_last_verdict() {
        let samples = [
            sample(10, Verdict::Pass),
            sample(30, Verdict::Pass),
            sample(20, Verdict::Pass),
        ];
        let summary = summarize_pipeline(&samples).expect("summary");
        assert_eq!(summary["schema"], PIPELINE_SUMMARY_SCHEMA);
        assert_eq!(summary["runs"], 3);
        assert_eq!(summary["phases"]["total"]["n"], 3);
        assert_eq!(summary["phases"]["total"]["p50_ms"], 20.0);
        assert_eq!(summary["phases"]["total"]["max_ms"], 30.0);
        assert_eq!(summary["cache"]["detectors_reused"], 2);
        assert_eq!(summary["findings"], 4);
    }
}
