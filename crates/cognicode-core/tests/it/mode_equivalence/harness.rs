//! The machinery that compares the two completeness-claiming analysis modes
//! over a real multi-language corpus.
//!
//! The corpus lives in `testdata/mode_equivalence`: `base/` is one project
//! with a Rust, a Java, a Kotlin, a Go, a Terraform and a Helm part, and
//! `edits/<part>/<class>/` is an overlay of files that replace (or add to)
//! the base, with an optional `DELETE` file naming paths to remove. Each
//! scenario commits the base and then the overlay into a throwaway Git
//! repository and analyzes the two commits through the real Git adapter.
//!
//! Both modes are driven by the production seams, not by synthetic records:
//! facts come from `produce_revision_facts`, runs from `run_detector`, plans
//! from `plan_between`, and the verdict from `build_complete_snapshot`.
//!
//! - `IncrementalComplete` reruns only the work the planner says must run
//!   and reuses the base records for the rest;
//! - `FullRebuild` reruns every detector.
//!
//! Over the same pair of commits the two must agree on the verdict, the
//! blocking reason codes and the findings. Anything else is a divergence
//! and the corpus is not certified.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cognicode_core::application::change_tracking::complete_snapshot::{
    AnalysisMode, CaptureGap, CompleteSnapshot, CompleteSnapshotRequest, Verdict,
    build_complete_snapshot,
};
use cognicode_core::application::change_tracking::detector_runs::{
    Detection, ProductionDetector, Reuse, current_identity, detect_with_reuse,
};
use cognicode_core::application::change_tracking::helm_templates::{
    ChartFiles, ChartRender, ChartRenderCheck, NoChartRender,
};
use cognicode_core::application::change_tracking::mode_benchmark::{
    capture_gaps, current_identities, fresh_record as bench_fresh_record,
};
use cognicode_core::application::change_tracking::planner::WorkDisposition;
use cognicode_core::application::change_tracking::revision_facts::{
    FactObjectStore, FactsLimits, ProducedRevision, dependencies_on,
    produce_revision_facts_rendering,
};
use cognicode_core::application::change_tracking::scm_delta::{
    delta_from_change_set, plan_between,
};
use cognicode_core::domain::findings::detector_reuse::{
    CachedObservation, DetectorReuseIndex, DetectorRunIdentity, DetectorRunRecord, ReboundFinding,
};
use cognicode_core::domain::scm::{ChangeReader, ChangeSet, RevisionReader, SourceRef};
use cognicode_core::domain::value_objects::WorkspaceId;
use cognicode_core::infrastructure::findings::in_memory_evidence::InMemoryEvidenceStore;
use cognicode_core::infrastructure::git::GitScmAdapter;
use cognicode_core::infrastructure::helm::HelmRevisionCharts;
use tempfile::TempDir;

/// The parts of the corpus, in report order.
pub const PARTS: [&str; 7] = ["rust", "java", "kotlin", "go", "terraform", "helm", "cross"];

/// What kind of change an overlay makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EditClass {
    /// Comments, whitespace or a body-only change: no definition appears,
    /// disappears or is renamed.
    Simple,
    /// A definition is added, removed, renamed or changes shape.
    Semantic,
    /// The edited file no longer parses cleanly.
    Malformed,
}

impl EditClass {
    pub const ALL: [Self; 3] = [Self::Simple, Self::Semantic, Self::Malformed];

    pub fn dir(self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Semantic => "semantic",
            Self::Malformed => "malformed",
        }
    }
}

/// How the Helm chart of the corpus is rendered while a scenario is judged.
///
/// A chart template is not YAML until rendered, so the verdict over a
/// chart depends on whether anything rendered it. The corpus is never
/// altered to make a verdict reachable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Projection {
    /// Nothing renders the chart, as in a run with no engine: its templates
    /// are gaps and `Pass` is unreachable, in both modes.
    Unrendered,
    /// A labelled double of the engine renders every chart whose templates
    /// are complete, so a revision can reach `Pass` and the comparison
    /// exercises the claim that matters. The double is deterministic and
    /// needs no `helm` binary; [`StrictTemplateRender`] says what it checks.
    Rendered,
    /// The real `helm` binary through the production adapter. Not part of
    /// [`Projection::ALL`]: it needs the engine on the machine.
    RealEngine,
}

impl Projection {
    pub const ALL: [Self; 2] = [Self::Unrendered, Self::Rendered];

    fn label(self) -> &'static str {
        match self {
            Self::Unrendered => "unrendered",
            Self::Rendered => "rendered",
            Self::RealEngine => "helm",
        }
    }

    /// The render check the production seam is given.
    fn charts(self) -> Box<dyn ChartRenderCheck> {
        match self {
            Self::Unrendered => Box::new(NoChartRender),
            Self::Rendered => Box::new(StrictTemplateRender),
            Self::RealEngine => Box::new(HelmRevisionCharts::from_path()),
        }
    }
}

/// A test double for the Helm engine, labelled as one: it renders nothing.
/// It answers `Rendered` for a chart that has a `Chart.yaml` and whose
/// every template action is closed, which is what an engine needs to
/// render it at all, and `NotRendered` otherwise. It does not evaluate
/// values, so it is more lenient than the engine where a template reads
/// an unset value.
pub struct StrictTemplateRender;

impl ChartRenderCheck for StrictTemplateRender {
    fn check(&self, chart: &ChartFiles) -> ChartRender {
        if !chart.files.contains_key("Chart.yaml") {
            return ChartRender::NotRendered {
                cause: "the chart has no Chart.yaml".to_string(),
            };
        }
        for (path, bytes) in &chart.files {
            let text = String::from_utf8_lossy(bytes);
            if path.starts_with("templates/") {
                if text.matches("{{").count() != text.matches("}}").count() {
                    return ChartRender::NotRendered {
                        cause: format!("template `{path}` has an unclosed action"),
                    };
                }
                if text.contains("required") {
                    let values = chart
                        .files
                        .get("values.yaml")
                        .and_then(|bytes| serde_norway::from_slice::<serde_json::Value>(bytes).ok())
                        .unwrap_or_else(|| serde_json::json!({}));
                    let required_path = text
                        .split(".Values.")
                        .nth(1)
                        .and_then(|tail| {
                            tail.split(|ch: char| {
                                !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
                            })
                            .next()
                        })
                        .unwrap_or_default();
                    if cognicode_core::domain::iac::helm::lookup(&values, required_path).is_none() {
                        return ChartRender::NotRendered {
                            cause: format!(
                                "template `{path}` requires unset `.Values.{required_path}`"
                            ),
                        };
                    }
                }
            }
        }
        ChartRender::Rendered
    }
}

/// A deliberate defect injected into the incremental side, to prove the
/// comparison can fail: an oracle that cannot disagree certifies nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    None,
    /// The incremental side skips every rerun the planner asked for.
    SkipPlannedReruns,
    /// The incremental side loses every finding a detector produced: the
    /// rerun results and the answer of the production entry point.
    DropDetectorFindings,
}

/// One edit of the base, named by the part it touches and its class.
#[derive(Debug, Clone)]
pub struct Scenario {
    pub part: String,
    pub class: EditClass,
    pub overlay: PathBuf,
}

impl Scenario {
    pub fn name(&self, projection: Projection) -> String {
        format!(
            "{}/{} [{}]",
            self.part,
            self.class.dir(),
            projection.label()
        )
    }
}

/// The root of the committed corpus.
pub fn corpus_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata/mode_equivalence")
}

/// Every scenario whose overlay directory exists, in a stable order.
pub fn scenarios(corpus: &Path) -> Vec<Scenario> {
    let mut found = Vec::new();
    for part in PARTS {
        for class in EditClass::ALL {
            let overlay = corpus.join("edits").join(part).join(class.dir());
            if overlay.is_dir() {
                found.push(Scenario {
                    part: part.to_string(),
                    class,
                    overlay,
                });
            }
        }
    }
    found
}

fn git(repo: &Path, args: &[&str]) -> String {
    let none = repo.join("no-git-configuration");
    let output = Command::new("git")
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_GLOBAL", &none)
        .env("GIT_CONFIG_SYSTEM", &none)
        .args(args)
        .current_dir(repo)
        .output()
        .expect("the git CLI is available");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("git prints UTF-8")
}

fn copy_tree(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).expect("read corpus directory") {
        let entry = entry.expect("corpus entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            fs::create_dir_all(&target).expect("create directory");
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy corpus file");
        }
    }
}

fn commit_all(repo: &Path, message: &str) -> String {
    git(repo, &["add", "-A"]);
    git(
        repo,
        &[
            "-c",
            "user.name=corpus",
            "-c",
            "user.email=corpus@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            message,
        ],
    );
    git(repo, &["rev-parse", "HEAD"]).trim().to_string()
}

/// A throwaway repository holding the base commit and the edited commit.
pub struct Fixture {
    _dir: TempDir,
    /// Decides the Helm charts of both revisions.
    pub charts: Box<dyn ChartRenderCheck>,
    pub adapter: GitScmAdapter,
    pub source: SourceRef,
    pub base_sha: String,
    pub head_sha: String,
}

/// Commits the base corpus and then the scenario's overlay.
pub fn materialize(corpus: &Path, scenario: &Scenario, projection: Projection) -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo = dir.path();
    git(repo, &["init", "-q", "-b", "main"]);
    copy_tree(&corpus.join("base"), repo);
    let base_sha = commit_all(repo, "base");
    let deletions = scenario.overlay.join("DELETE");
    if deletions.is_file() {
        for path in fs::read_to_string(&deletions).expect("read DELETE").lines() {
            if !path.trim().is_empty() {
                fs::remove_file(repo.join(path.trim())).expect("delete overlay path");
            }
        }
    }
    copy_overlay(&scenario.overlay, repo);
    let head_sha = commit_all(repo, "edit");
    Fixture {
        adapter: GitScmAdapter::new(repo),
        charts: projection.charts(),
        source: SourceRef::new("git", "mode-equivalence").expect("source"),
        _dir: dir,
        base_sha,
        head_sha,
    }
}

fn copy_overlay(overlay: &Path, repo: &Path) {
    for entry in fs::read_dir(overlay).expect("read overlay") {
        let entry = entry.expect("overlay entry");
        if entry.file_name() == "DELETE" || entry.file_name() == "ORACLE" {
            continue;
        }
        let target = repo.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            fs::create_dir_all(&target).expect("create directory");
            copy_overlay(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy overlay file");
        }
    }
}

/// Both revisions produced from a fixture, plus the changes between them.
pub struct Produced {
    pub base: ProducedRevision,
    pub head: ProducedRevision,
    pub changes: ChangeSet,
}

/// Cold facts for both revisions of the fixture.
pub fn produce(fixture: &Fixture) -> Produced {
    let limits = FactsLimits::default();
    let base_revision = fixture
        .adapter
        .resolve(&fixture.source, &fixture.base_sha)
        .expect("resolve base");
    let head_revision = fixture
        .adapter
        .resolve(&fixture.source, &fixture.head_sha)
        .expect("resolve head");
    let changes = fixture
        .adapter
        .changes_between(&base_revision, &head_revision)
        .expect("changes between");
    Produced {
        base: produce_revision_facts_rendering(
            &fixture.adapter,
            &base_revision,
            None,
            &limits,
            fixture.charts.as_ref(),
        )
        .expect("base facts"),
        head: produce_revision_facts_rendering(
            &fixture.adapter,
            &head_revision,
            None,
            &limits,
            fixture.charts.as_ref(),
        )
        .expect("head facts"),
        changes,
    }
}

pub fn workspace() -> WorkspaceId {
    WorkspaceId::try_new("mode-equivalence").expect("workspace")
}

/// The part of a finding that must be the same whichever mode produced it:
/// its semantic identity, where it points and what state it reports. The
/// snapshot that produced it and the evidence handles are deliberately not
/// compared: a reused finding keeps naming the snapshot of the run it came
/// from, and handle numbers belong to the sink of each run.
pub fn comparable(finding: &ReboundFinding) -> String {
    let state = match &finding.observation {
        CachedObservation::Verifiable { evidence, .. }
        | CachedObservation::Advisory { evidence, .. } => {
            format!("evidence={}", evidence.len())
        }
        other => format!("{other:?}"),
    };
    let locations: Vec<String> = finding
        .output_locations
        .iter()
        .map(|l| format!("{}:{}:{}", l.file, l.line, l.column))
        .collect();
    format!(
        "{} @ {} [{}]",
        finding.semantic_fingerprint.as_str(),
        locations.join(","),
        state
    )
}

fn comparable_set(findings: &[ReboundFinding]) -> BTreeSet<String> {
    findings.iter().map(comparable).collect()
}

/// How one detector was decided and whether its findings match a cold run.
#[derive(Debug, Clone)]
pub struct DetectorComparison {
    pub detector: String,
    pub planned: WorkDisposition,
    pub reuse: String,
    pub findings_full: usize,
    pub only_incremental: Vec<String>,
    pub only_full: Vec<String>,
}

impl DetectorComparison {
    pub fn equivalent(&self) -> bool {
        self.only_incremental.is_empty() && self.only_full.is_empty()
    }
}

/// What the harness found for one scenario.
#[derive(Debug, Clone)]
pub struct ScenarioResult {
    pub scenario: String,
    pub head_gaps: Vec<String>,
    pub warm_facts_equal_cold: bool,
    pub detectors: Vec<DetectorComparison>,
    pub incremental_verdict: Verdict,
    pub full_verdict: Verdict,
    pub incremental_codes: Vec<&'static str>,
    pub full_findings: usize,
    /// Findings located under the part this scenario edits. Zero means the
    /// production detectors see nothing of that part, so agreement on it
    /// proves nothing.
    pub part_findings: usize,
    /// Findings under the edited part in IncrementalComplete.
    pub incremental_part_findings: usize,
    /// What disagrees, stated as sentences. Empty when the modes agree.
    pub divergences: Vec<String>,
    /// Where an honest analysis would have refused and did not: a
    /// malformed edit that still receives `Pass`.
    pub false_passes: Vec<String>,
    /// The overlay changed nothing, so there is no edit to judge.
    pub no_change: bool,
}

impl ScenarioResult {
    pub fn vacuous(&self) -> bool {
        self.part_findings == 0
    }
}

pub fn identities() -> BTreeMap<String, DetectorRunIdentity> {
    current_identities().expect("identities")
}

pub fn gaps_of(head: &ProducedRevision) -> Vec<CaptureGap> {
    capture_gaps(head)
}

pub fn fresh_record(
    detector: ProductionDetector,
    revision: &ProducedRevision,
) -> DetectorRunRecord {
    bench_fresh_record(
        detector,
        revision,
        &workspace(),
        &mut InMemoryEvidenceStore::new(),
    )
    .expect("detector run")
}

/// Warm facts equal cold facts: the second revision is produced from an
/// object store primed by the first.
fn warm_equals_cold(fixture: &Fixture, produced: &Produced) -> bool {
    let store_dir = tempfile::tempdir().expect("tempdir");
    let store = FactObjectStore::new(store_dir.path());
    let limits = FactsLimits::default();
    let revision = |sha: &str| {
        fixture
            .adapter
            .resolve(&fixture.source, sha)
            .expect("resolve")
    };
    let _prime = produce_revision_facts_rendering(
        &fixture.adapter,
        &revision(&fixture.base_sha),
        Some(&store),
        &limits,
        fixture.charts.as_ref(),
    )
    .expect("prime");
    let warm = produce_revision_facts_rendering(
        &fixture.adapter,
        &revision(&fixture.head_sha),
        Some(&store),
        &limits,
        fixture.charts.as_ref(),
    )
    .expect("warm head");
    format!("{:?}", warm.facts.facts) == format!("{:?}", produced.head.facts.facts)
        && warm.coverage.gaps == produced.head.coverage.gaps
}

/// Both modes over the same pair of commits, compared.
pub fn evaluate(corpus: &Path, scenario: &Scenario, projection: Projection) -> ScenarioResult {
    evaluate_with_fault(corpus, scenario, projection, Fault::None)
}

pub fn evaluate_with_fault(
    corpus: &Path,
    scenario: &Scenario,
    projection: Projection,
    fault: Fault,
) -> ScenarioResult {
    let fixture = materialize(corpus, scenario, projection);
    let produced = produce(&fixture);
    evaluate_produced(scenario, projection, fault, &fixture, &produced)
}

pub fn evaluate_produced(
    scenario: &Scenario,
    projection: Projection,
    fault: Fault,
    fixture: &Fixture,
    produced: &Produced,
) -> ScenarioResult {
    let (base, head, changes) = (&produced.base, &produced.head, &produced.changes);
    let mut divergences = Vec::new();

    let mut index = DetectorReuseIndex::empty();
    for detector in ProductionDetector::ALL {
        index.remember(fresh_record(detector, base));
    }
    let dependencies = dependencies_on(&index, base.facts.snapshot);
    let manifests: BTreeSet<_> = base.manifests.union(&head.manifests).cloned().collect();
    let plan = plan_between(&base.facts, &head.facts, changes, &manifests, &dependencies)
        .expect("plan between");
    let delta = delta_from_change_set(&base.facts, &head.facts, changes)
        .expect("delta")
        .delta;
    let gaps = gaps_of(head);
    let ids = identities();

    let mut full_reruns = BTreeMap::new();
    let mut incremental_reruns = BTreeMap::new();
    let mut disposition_of = BTreeMap::new();
    for detector in ProductionDetector::ALL {
        let key = current_identity(detector)
            .expect("identity")
            .detector_id
            .as_str()
            .to_string();
        let record = fresh_record(detector, head);
        let planned = plan
            .decisions
            .iter()
            .find(|d| d.work.as_namespaced_name().as_str() == key)
            .map(|d| d.disposition);
        disposition_of.insert(key.clone(), planned);
        if planned.is_none_or(WorkDisposition::must_rerun) {
            incremental_reruns.insert(key.clone(), record.clone());
        }
        full_reruns.insert(key, record);
    }

    if fault == Fault::SkipPlannedReruns {
        incremental_reruns.clear();
    }
    if fault == Fault::DropDetectorFindings {
        for record in incremental_reruns.values_mut() {
            record.findings.clear();
        }
    }

    let build = |mode, reruns: &BTreeMap<String, DetectorRunRecord>| -> CompleteSnapshot {
        build_complete_snapshot(&CompleteSnapshotRequest {
            mode,
            plan: &plan,
            candidate_snapshot: head.facts.snapshot,
            delta: &delta,
            cache: &index,
            current_identities: &ids,
            reruns,
            gaps: &gaps,
        })
    };
    let incremental = build(AnalysisMode::IncrementalComplete, &incremental_reruns);
    let full = build(AnalysisMode::FullRebuild, &full_reruns);

    if incremental.verdict() != full.verdict() {
        divergences.push(format!(
            "verdict: incremental_complete={:?} full_rebuild={:?}",
            incremental.verdict(),
            full.verdict()
        ));
    }
    if incremental.reason_codes() != full.reason_codes() {
        divergences.push(format!(
            "blocking reason codes: incremental_complete={:?} full_rebuild={:?}",
            incremental.reason_codes(),
            full.reason_codes()
        ));
    }
    if incremental.semantic_coverage() != full.semantic_coverage() {
        divergences.push(format!(
            "semantic coverage differs: only incremental {:?}, only full {:?}",
            incremental
                .semantic_coverage()
                .difference(&full.semantic_coverage())
                .collect::<Vec<_>>(),
            full.semantic_coverage()
                .difference(&incremental.semantic_coverage())
                .collect::<Vec<_>>()
        ));
    }
    if comparable_set(&incremental.findings) != comparable_set(&full.findings) {
        divergences.push(format!(
            "findings differ beyond identity: only incremental {:?}, only full {:?}",
            comparable_set(&incremental.findings)
                .difference(&comparable_set(&full.findings))
                .collect::<Vec<_>>(),
            comparable_set(&full.findings)
                .difference(&comparable_set(&incremental.findings))
                .collect::<Vec<_>>()
        ));
    }

    // The production entry point, detector by detector, against a cold run.
    let mut detectors = Vec::new();
    for detector in ProductionDetector::ALL {
        let key = current_identity(detector)
            .expect("identity")
            .detector_id
            .as_str()
            .to_string();
        let prior = index.record_for(&key, base.facts.snapshot);
        let detection: Detection = detect_with_reuse(
            detector,
            prior,
            base,
            head,
            changes,
            &workspace(),
            &mut InMemoryEvidenceStore::new(),
        )
        .expect("detect with reuse");
        let cold = fresh_record(detector, head);
        let cold_findings: Vec<ReboundFinding> = cold
            .findings
            .iter()
            .map(|f| ReboundFinding {
                produced_in: head.facts.snapshot,
                reported_in: head.facts.snapshot,
                semantic_fingerprint: f.semantic_fingerprint.clone(),
                output_locations: f.output_locations.clone(),
                observation: f.observation.clone(),
            })
            .collect();
        let got = if fault == Fault::DropDetectorFindings {
            BTreeSet::new()
        } else {
            comparable_set(&detection.findings)
        };
        let want = comparable_set(&cold_findings);
        let comparison = DetectorComparison {
            detector: key.clone(),
            planned: disposition_of
                .get(&key)
                .copied()
                .flatten()
                .unwrap_or(WorkDisposition::Unknown),
            reuse: match &detection.reuse {
                Reuse::Reused { .. } => "reused".to_string(),
                Reuse::Rerun { reason } => format!("rerun ({reason})"),
            },
            findings_full: want.len(),
            only_incremental: got.difference(&want).cloned().collect(),
            only_full: want.difference(&got).cloned().collect(),
        };
        if !comparison.equivalent() {
            divergences.push(format!(
                "{key}: detect_with_reuse ({}) differs from a cold run: only reuse {:?}, only cold {:?}",
                comparison.reuse, comparison.only_incremental, comparison.only_full
            ));
        }
        detectors.push(comparison);
    }

    let warm_equal = warm_equals_cold(fixture, produced);
    if !warm_equal {
        divergences.push("facts from a warm object store differ from a cold parse".to_string());
    }

    // A completeness claim cannot hold over an analysis that skipped files.
    if !gaps.is_empty()
        && (incremental.verdict() == Verdict::Pass || full.verdict() == Verdict::Pass)
    {
        divergences.push("a Pass was claimed while the head has files without facts".to_string());
    }

    let part_prefix = format!("{}/", scenario.part);
    let is_in_part = |finding: &&ReboundFinding| {
        scenario.part == "cross"
            || finding
                .output_locations
                .iter()
                .any(|location| location.file.starts_with(&part_prefix))
    };
    let part_findings = full.findings.iter().filter(is_in_part).count();
    let incremental_part_findings = incremental.findings.iter().filter(is_in_part).count();
    let mut false_passes = Vec::new();
    let no_change = changes.changes().is_empty();
    if scenario.class == EditClass::Malformed
        && !no_change
        && (incremental.verdict() == Verdict::Pass || full.verdict() == Verdict::Pass)
    {
        false_passes.push(format!(
            "malformed edit received Pass: incremental_complete={:?} full_rebuild={:?}",
            incremental.verdict(),
            full.verdict()
        ));
    }
    ScenarioResult {
        scenario: scenario.name(projection),
        head_gaps: gaps.iter().map(|g| g.detail.clone()).collect(),
        warm_facts_equal_cold: warm_equal,
        detectors,
        incremental_verdict: incremental.verdict(),
        full_verdict: full.verdict(),
        incremental_codes: incremental.reason_codes(),
        full_findings: full.findings.len(),
        part_findings,
        incremental_part_findings,
        divergences,
        false_passes,
        no_change,
    }
}

/// The corpus-level statement: certified only when no scenario diverged.
#[derive(Debug, Clone)]
pub struct Certification {
    pub certified: bool,
    pub results: Vec<ScenarioResult>,
}

pub fn certify(results: Vec<ScenarioResult>) -> Certification {
    Certification {
        certified: results.iter().all(|r| r.divergences.is_empty()),
        results,
    }
}

impl Certification {
    /// A table a person can read, ending in the verdict on the corpus.
    pub fn render(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "{:<26} {:<12} {:<12} {:<9} {:<7} {:<8} detectors (planned/executed)",
            "scenario", "incremental", "full_rebuild", "findings", "in_part", "warm=cold"
        );
        for r in &self.results {
            let detectors: Vec<String> = r
                .detectors
                .iter()
                .map(|d| {
                    format!(
                        "{}={:?}/{}({})",
                        d.detector.rsplit('.').next().unwrap_or(&d.detector),
                        d.planned,
                        d.reuse.split(' ').next().unwrap_or(""),
                        d.findings_full
                    )
                })
                .collect();
            let _ = writeln!(
                out,
                "{:<26} {:<12} {:<12} {:<9} {:<7} {:<8} {}",
                r.scenario,
                format!("{:?}", r.incremental_verdict),
                format!("{:?}", r.full_verdict),
                r.full_findings,
                r.part_findings,
                r.warm_facts_equal_cold,
                detectors.join(" ")
            );
            if !r.incremental_codes.is_empty() {
                let _ = writeln!(out, "    blocking: {:?}", r.incremental_codes);
            }
            for gap in &r.head_gaps {
                let _ = writeln!(out, "    gap: {gap}");
            }
            for divergence in &r.divergences {
                let _ = writeln!(out, "    DIVERGENCE: {divergence}");
            }
            if r.no_change {
                let _ = writeln!(out, "    no change: the overlay changes nothing");
            }
            for false_pass in &r.false_passes {
                let _ = writeln!(out, "    FALSE PASS: {false_pass}");
            }
        }
        let vacuous: Vec<&str> = self
            .results
            .iter()
            .filter(|r| r.vacuous())
            .map(|r| r.scenario.as_str())
            .collect();
        let _ = writeln!(
            out,
            "\nscenarios where the detectors see nothing of the edited part (agreement proves nothing): {}",
            vacuous.len()
        );
        let _ = writeln!(
            out,
            "equivalence over {} scenario(s): {}",
            self.results.len(),
            if self.certified {
                "CERTIFIED (no divergence found)"
            } else {
                "NOT CERTIFIED (divergence found)"
            }
        );
        out
    }

    /// The scenarios where a malformed edit still received `Pass`.
    pub fn false_passes(&self) -> Vec<String> {
        self.results
            .iter()
            .filter(|r| !r.false_passes.is_empty())
            .map(|r| r.scenario.clone())
            .collect()
    }
}
