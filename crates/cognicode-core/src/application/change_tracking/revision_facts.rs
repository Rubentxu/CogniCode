//! The production source of [`RevisionFacts`]: the facts of one
//! revision, read through the SCM ports and projected by the existing
//! fact bridge, plus the honest statements a comparison may make from
//! two such revisions.
//!
//! ## What this module is, and is not
//!
//! It is the missing producer between the neutral SCM ports
//! ([`RevisionReader`]) and the one affected-work planner
//! ([`plan_between`]). It does not plan, rank or schedule: the planner
//! and the bridge in [`super::scm_delta`] keep every verdict. It does
//! not parse differently from the rest of the product either: every
//! file goes through `extract_file` and [`FactBatchBuilder`].
//!
//! ## Why the facts are assembled per file
//!
//! The fact of one file depends on that file's bytes and path alone:
//! every object of a bridge fact is text and every subject is named by
//! [`stable_entity_id`](crate::application::fact_bridge::batch_builder::stable_entity_id), so a batch of two files is the union of the two
//! batches of one. Per-file facts are therefore content-addressed
//! objects ([`FactObjectStore`]): a revision that shares bytes with an
//! earlier one, on any branch, reuses them instead of parsing again, and
//! the assembled facts are identical to a cold rebuild. A test pins that
//! equivalence.
//!
//! ## What incomplete means
//!
//! A file the producer could not read, decode or extract is a
//! [`FactGap`], never an empty file. Gaps travel with the facts
//! ([`FactsCoverage`]) and every statement derived from them degrades:
//! an impact becomes [`ImpactStatus::Partial`], and no piece of work is
//! reported as reusable on facts that could be missing — a fact missing
//! on both sides looks unchanged, which is exactly the false negative
//! the planner exists to prevent.
//!
//! A Helm chart template is the one analyzable file that is not read as
//! its extension says: it is Go-template text until rendered, so it is
//! decided by a render check of its chart instead
//! (see [`super::helm_templates`]). Without a faithful render it is a
//! [`GapReason::TemplateNotRendered`] gap: never a syntax error, never
//! facts, never clean.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::application::fact_bridge::FactBatchBuilder;
use crate::application::ingest::extractor::{HandlerOutcome, SyntaxFault, extract_file_checked};
use crate::application::revision_snapshots::{
    ExtractionObject, ProducerVersions, RevisionSnapshot,
};
use crate::domain::evidence_kernel::fact::Fact;
use crate::domain::evidence_kernel::ids::{FactId, SnapshotId};
use crate::domain::findings::detector_reuse::DetectorReuseIndex;
use crate::domain::kernel_ids::ExecutionId;
use crate::domain::naming::NamespacedName;
use crate::domain::readset::{ReadSet, ReadSetConfig};
use crate::domain::scm::{
    BlobRead, ChangeSet, InventoryEntry, PathKey, RevisionReader, ScmError, SourceInventory,
    SourceRevision,
};

use crate::application::ingest::extractor::{SourceLanguage, language_for_extension};

use super::helm_templates::{
    ChartFiles, ChartRender, ChartRenderCheck, NoChartRender, below_root, chart_roots,
    is_chart_file, template_chart_root,
};
use super::planner::{AffectedWorkPlan, ExecutionDependency, WorkDisposition, WorkId};
use super::scm_delta::{
    RevisionFacts, ScmDeltaError, delta_from_change_set, plan_between, snapshot_id_for,
};

/// The version of the fact assembly itself. It is part of every object
/// key and of the producer identity of every persisted snapshot: a change
/// to how facts are assembled must bump it, and nothing written before
/// is served afterwards. v3: chart templates are decided by a render, no
/// longer extracted as YAML.
pub const FACTS_PRODUCER_VERSION: &str = "cognicode.revision-facts/v3";

/// Paths read per batched content request: bounds peak memory (about
/// this many files of at most `max_file_bytes` each) while keeping the
/// per-request process cost amortised.
const READ_CHUNK: usize = 256;

const OBJECT_SCHEMA: &str = "cognicode.revision-facts-object/v1";

/// The files a build system reads. A change to any of them can move
/// what an analysis means without moving a single extracted fact, so the
/// planner is told about them explicitly.
const MANIFEST_NAMES: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "tsconfig.json",
    "pyproject.toml",
    "requirements.txt",
    "go.mod",
    "go.sum",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
    "Gemfile",
    "Gemfile.lock",
    "composer.json",
    "mix.exs",
];

/// How much one production run may read. A bound that is hit is a gap,
/// not a silent omission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FactsLimits {
    /// Analyzable files read per revision.
    pub max_files: usize,
    /// Bytes of one file the extractor is given.
    pub max_file_bytes: usize,
}

impl Default for FactsLimits {
    fn default() -> Self {
        Self {
            max_files: 20_000,
            max_file_bytes: 1024 * 1024,
        }
    }
}

/// Why one analyzable file contributes no facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GapReason {
    /// The provider could not hand over the bytes.
    Unreadable(String),
    /// The bytes are not UTF-8 text.
    NotText,
    /// The file is over the byte bound.
    TooLarge { bytes: usize, limit: usize },
    /// The extractor failed on this file.
    ExtractionFailed(String),
    /// The file does not parse cleanly: the grammar recovered around the
    /// fault, so the declarations read from it are an unknown share of what
    /// the file declares, and no consumer may take them for the whole.
    SyntaxError(SyntaxFault),
    /// The file is a Helm chart template, which is Go-template text and not
    /// YAML until rendered, and its chart was not (completely) rendered, so
    /// nothing is known about the document it stands for. Not a syntax
    /// error: the file is fine; the render is what is missing.
    TemplateNotRendered {
        /// The chart root, `.` for the repository root.
        chart: String,
        /// Why the chart was not rendered.
        cause: String,
    },
    /// The revision has more analyzable files than the run may read.
    FileLimitReached { limit: usize },
}

impl GapReason {
    /// A stable, one-line rendering for answers.
    pub fn describe(&self) -> String {
        match self {
            Self::Unreadable(why) => format!("content unreadable: {why}"),
            Self::NotText => "content is not UTF-8 text".to_string(),
            Self::TooLarge { bytes, limit } => {
                format!("file of {bytes} bytes exceeds the {limit} byte bound")
            }
            Self::ExtractionFailed(why) => format!("extraction failed: {why}"),
            Self::SyntaxError(fault) => fault.describe(),
            Self::TemplateNotRendered { chart, cause } => format!(
                "helm template is not YAML until rendered, and chart `{chart}` was not rendered: {cause}"
            ),
            Self::FileLimitReached { limit } => {
                format!("revision exceeds the {limit} analyzable file bound")
            }
        }
    }
}

/// One analyzable path whose facts are missing, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactGap {
    pub path: PathKey,
    pub reason: GapReason,
}

/// What the facts of one revision cover.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FactsCoverage {
    /// Tracked paths of the revision.
    pub tracked_files: usize,
    /// Paths a language reader claims.
    pub analyzable_files: usize,
    /// Analyzable files whose facts were parsed in this run.
    pub parsed_files: usize,
    /// Analyzable files whose facts came from a stored object.
    pub reused_files: usize,
    /// Analyzable chart templates covered by a faithful render of their
    /// chart instead of by facts of their own.
    pub rendered_files: usize,
    /// Tracked paths no reader claims: their content is invisible to the
    /// facts, which is a limit of the facts, not a gap in them.
    pub unanalyzed_files: usize,
    /// Analyzable paths with no facts, and why.
    pub gaps: Vec<FactGap>,
}

impl FactsCoverage {
    /// Whether every analyzable file contributed its facts.
    pub fn is_complete(&self) -> bool {
        self.gaps.is_empty()
    }
}

/// The facts of one revision, how complete they are, and the snapshot
/// record that makes them reusable.
#[derive(Debug, Clone)]
pub struct ProducedRevision {
    pub revision: SourceRevision,
    pub facts: RevisionFacts,
    pub coverage: FactsCoverage,
    /// The tracked manifest paths of this revision.
    pub manifests: BTreeSet<PathKey>,
    /// Tracked paths no reader claims, for statements about changes the
    /// facts cannot see.
    pub unanalyzed: BTreeSet<PathKey>,
    /// The record to persist: per-file objects plus the coverage verdict.
    pub snapshot: RevisionSnapshot,
}

// ----- the object store ---------------------------------------------

/// Where the per-file fact objects live: a directory of
/// content-addressed files, one per `(producer, path, bytes)`.
///
/// Reading is never trusted blindly: an object that cannot be parsed, or
/// whose inside names another path, content or artifact digest than the
/// one asked for, is simply absent. Writing is atomic.
#[derive(Debug, Clone)]
pub struct FactObjectStore {
    dir: PathBuf,
    writable: bool,
}

#[derive(Serialize, Deserialize)]
struct ObjectFile {
    schema: String,
    path: String,
    content: String,
    artifact: String,
    facts: Vec<Fact>,
}

impl FactObjectStore {
    /// An object store rooted at `dir` (created on first write).
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            writable: true,
        }
    }

    /// The same store, serving reads and refusing every write: a run over it
    /// leaves the directory exactly as it found it.
    pub fn read_only(mut self) -> Self {
        self.writable = false;
        self
    }

    /// Whether a run may add objects to this store.
    pub fn is_writable(&self) -> bool {
        self.writable
    }

    /// The store that lives beside the revision snapshots of `store`.
    pub fn beside(
        snapshots: &crate::application::revision_snapshots::RevisionSnapshotStore,
    ) -> Self {
        Self::new(snapshots.root().join("facts"))
    }

    fn file_for(&self, path: &PathKey, content: &str) -> PathBuf {
        let mut hasher = Sha256::new();
        hasher.update(FACTS_PRODUCER_VERSION.as_bytes());
        hasher.update([0]);
        hasher.update(path.as_str().as_bytes());
        hasher.update([0]);
        hasher.update(content.as_bytes());
        self.dir
            .join(format!("{}.facts.json", hex(&hasher.finalize())))
    }

    fn read(&self, path: &PathKey, content: &str) -> Option<(Vec<Fact>, String)> {
        let text = std::fs::read_to_string(self.file_for(path, content)).ok()?;
        let object: ObjectFile = serde_json::from_str(&text).ok()?;
        let agrees = object.schema == OBJECT_SCHEMA
            && object.path == path.as_str()
            && object.content == content
            && artifact_of(&object.facts).as_deref() == Some(object.artifact.as_str());
        agrees.then_some((object.facts, object.artifact))
    }

    fn write(
        &self,
        path: &PathKey,
        content: &str,
        facts: &[Fact],
        artifact: &str,
    ) -> io::Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let object = ObjectFile {
            schema: OBJECT_SCHEMA.to_string(),
            path: path.as_str().to_string(),
            content: content.to_string(),
            artifact: artifact.to_string(),
            facts: facts.to_vec(),
        };
        let bytes = serde_json::to_vec(&object)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let target = self.file_for(path, content);
        let mut writer =
            crate::application::services::atomic_file_writer::AtomicFileWriter::create(&target)?;
        writer.write(&bytes)?;
        writer.commit()
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The digest of the derived object: the facts, serialized.
fn artifact_of(facts: &[Fact]) -> Option<String> {
    serde_json::to_vec(facts)
        .ok()
        .map(|bytes| hex(&Sha256::digest(bytes)))
}

// ----- the producer ---------------------------------------------------

/// The producer identity written into every snapshot this module saves.
pub fn producer_versions() -> ProducerVersions {
    ProducerVersions::new(
        format!(
            "tree-sitter-facts/{FACTS_PRODUCER_VERSION}/{}",
            env!("CARGO_PKG_VERSION")
        ),
        "none",
        FACTS_PRODUCER_VERSION,
        "none",
        "none",
    )
}

fn is_manifest(path: &PathKey) -> bool {
    let name = path.as_str().rsplit('/').next().unwrap_or_default();
    MANIFEST_NAMES.contains(&name)
}

fn language_of(path: &PathKey) -> Option<&'static SourceLanguage> {
    let extension = Path::new(path.as_str()).extension()?.to_str()?;
    language_for_extension(extension)
}

/// Assembles the facts of `revision`.
///
/// `objects`, when given, serves and receives per-file fact objects; with
/// `None` every file is parsed (a cold rebuild). The two produce the same
/// facts.
///
/// # Errors
/// The inventory of the revision could not be read: with no inventory
/// there is no honest partial answer. Per-file failures are
/// [`FactGap`]s, not errors.
pub fn produce_revision_facts(
    reader: &dyn RevisionReader,
    revision: &SourceRevision,
    objects: Option<&FactObjectStore>,
    limits: &FactsLimits,
) -> Result<ProducedRevision, ScmError> {
    produce_revision_facts_rendering(reader, revision, objects, limits, &NoChartRender)
}

/// [`produce_revision_facts`] with a render check for Helm charts.
///
/// A chart template is not YAML until rendered, so it never reaches the
/// YAML grammar (see [`super::helm_templates`]): `charts` decides, once per
/// chart, whether the chart was faithfully rendered. Only then are its
/// templates covered; otherwise each one is a
/// [`GapReason::TemplateNotRendered`] gap.
///
/// # Errors
/// As [`produce_revision_facts`].
pub fn produce_revision_facts_rendering(
    reader: &dyn RevisionReader,
    revision: &SourceRevision,
    objects: Option<&FactObjectStore>,
    limits: &FactsLimits,
    charts: &dyn ChartRenderCheck,
) -> Result<ProducedRevision, ScmError> {
    let inventory = reader.inventory(revision)?;
    let snapshot_id = snapshot_id_for(revision);
    let mut coverage = FactsCoverage {
        tracked_files: inventory.entries().len(),
        ..FactsCoverage::default()
    };
    let mut manifests = BTreeSet::new();
    let mut unanalyzed = BTreeSet::new();
    let mut per_file: BTreeMap<PathKey, Vec<Fact>> = BTreeMap::new();
    let mut files: BTreeMap<PathKey, ExtractionObject> = BTreeMap::new();
    let mut pending: Vec<(&InventoryEntry, &SourceLanguage)> = Vec::new();
    let roots = chart_roots(inventory.entries().iter().map(|entry| entry.path.as_str()));
    let mut templates: BTreeMap<String, Vec<PathKey>> = BTreeMap::new();

    for entry in inventory.entries() {
        if is_manifest(&entry.path) {
            manifests.insert(entry.path.clone());
        }
        let Some(config) = language_of(&entry.path) else {
            coverage.unanalyzed_files += 1;
            unanalyzed.insert(entry.path.clone());
            continue;
        };
        coverage.analyzable_files += 1;
        if coverage.analyzable_files > limits.max_files {
            coverage.gaps.push(FactGap {
                path: entry.path.clone(),
                reason: GapReason::FileLimitReached {
                    limit: limits.max_files,
                },
            });
            continue;
        }
        // Go-template text is not YAML: no grammar and no stored object may
        // speak for it. Only a render of its chart can.
        if let Some(root) = template_chart_root(entry.path.as_str(), &roots) {
            templates.entry(root).or_default().push(entry.path.clone());
            continue;
        }
        let content = entry.content.as_hex();
        if let Some((facts, artifact)) = objects.and_then(|store| store.read(&entry.path, content))
        {
            coverage.reused_files += 1;
            files.insert(
                entry.path.clone(),
                ExtractionObject {
                    content: entry.content.clone(),
                    artifact,
                },
            );
            per_file.insert(entry.path.clone(), facts);
            continue;
        }
        pending.push((entry, config));
    }

    for chunk in pending.chunks(READ_CHUNK) {
        let paths: Vec<PathKey> = chunk.iter().map(|(entry, _)| entry.path.clone()).collect();
        let blobs = reader.contents(revision, &paths, limits.max_file_bytes as u64);
        for (index, (entry, config)) in chunk.iter().enumerate() {
            let content = entry.content.as_hex();
            let blob = blobs.get(index).cloned().unwrap_or_else(|| {
                Err(ScmError::ContentUnavailable {
                    path: entry.path.as_str().to_string(),
                })
            });
            match extract_one(blob, &entry.path, content, config, limits) {
                Ok(facts) => {
                    coverage.parsed_files += 1;
                    let artifact = artifact_of(&facts).unwrap_or_default();
                    if let Some(store) = objects.filter(|store| store.is_writable()) {
                        // A store that cannot be written costs reuse, never
                        // correctness: the facts are already in hand.
                        let _ = store.write(&entry.path, content, &facts, &artifact);
                    }
                    files.insert(
                        entry.path.clone(),
                        ExtractionObject {
                            content: entry.content.clone(),
                            artifact,
                        },
                    );
                    per_file.insert(entry.path.clone(), facts);
                }
                Err(reason) => coverage.gaps.push(FactGap {
                    path: entry.path.clone(),
                    reason,
                }),
            }
        }
    }

    for (root, template_paths) in &templates {
        let outcome = match read_chart_files(reader, revision, &inventory, root, limits) {
            Ok(chart) => charts.check(&chart),
            Err(cause) => ChartRender::NotRendered { cause },
        };
        match outcome {
            ChartRender::Rendered => {
                coverage.rendered_files += template_paths.len();
                unanalyzed.extend(template_paths.iter().cloned());
            }
            ChartRender::NotRendered { cause } => {
                let chart = if root.is_empty() { "." } else { root.as_str() };
                coverage
                    .gaps
                    .extend(template_paths.iter().map(|path| FactGap {
                        path: path.clone(),
                        reason: GapReason::TemplateNotRendered {
                            chart: chart.to_string(),
                            cause: cause.clone(),
                        },
                    }));
            }
        }
    }

    // Gaps are found in several passes (limit/skip first, reads after): a
    // path order keeps the report independent of that.
    coverage.gaps.sort_by(|a, b| a.path.cmp(&b.path));

    let facts = assemble(snapshot_id, per_file);
    let verdict = if coverage.is_complete() {
        "full".to_string()
    } else {
        format!("partial: {} file(s) without facts", coverage.gaps.len())
    };
    let snapshot = RevisionSnapshot {
        lineage: revision.handle().clone(),
        source_content: revision.content().clone(),
        coverage: verdict,
        producer: producer_versions(),
        files,
        resolution: None,
    };
    Ok(ProducedRevision {
        revision: revision.clone(),
        facts,
        coverage,
        manifests,
        unanalyzed,
        snapshot,
    })
}

/// The files of the chart rooted at `root`, read from the revision.
///
/// # Errors
/// A chart file that cannot be read, is over the byte bound, or whose
/// path is not inside the chart: a chart that cannot be handed over whole
/// cannot be rendered faithfully, and the cause says why.
fn read_chart_files(
    reader: &dyn RevisionReader,
    revision: &SourceRevision,
    inventory: &SourceInventory,
    root: &str,
    limits: &FactsLimits,
) -> Result<ChartFiles, String> {
    let wanted: Vec<(PathKey, String)> = inventory
        .entries()
        .iter()
        .filter_map(|entry| {
            let relative = below_root(entry.path.as_str(), root)?;
            is_chart_file(relative).then(|| (entry.path.clone(), relative.to_string()))
        })
        .collect();
    let mut files = BTreeMap::new();
    for chunk in wanted.chunks(READ_CHUNK) {
        let paths: Vec<PathKey> = chunk.iter().map(|(path, _)| path.clone()).collect();
        let blobs = reader.contents(revision, &paths, limits.max_file_bytes as u64);
        for (index, (path, relative)) in chunk.iter().enumerate() {
            let blob = blobs.get(index).cloned().unwrap_or_else(|| {
                Err(ScmError::ContentUnavailable {
                    path: path.as_str().to_string(),
                })
            });
            match blob {
                Ok(BlobRead::Bytes(bytes)) if bytes.len() <= limits.max_file_bytes => {
                    files.insert(relative.clone(), bytes);
                }
                Ok(_) => {
                    return Err(format!(
                        "chart file `{}` exceeds the {} byte bound",
                        path.as_str(),
                        limits.max_file_bytes
                    ));
                }
                Err(error) => {
                    return Err(format!(
                        "chart file `{}` is unreadable: {error}",
                        path.as_str()
                    ));
                }
            }
        }
    }
    Ok(ChartFiles {
        root: root.to_string(),
        files,
    })
}

/// Parses one file into its facts, with stable subject ids.
fn extract_one(
    blob: Result<BlobRead, ScmError>,
    path: &PathKey,
    content_hex: &str,
    config: &SourceLanguage,
    limits: &FactsLimits,
) -> Result<Vec<Fact>, GapReason> {
    let bytes = match blob.map_err(|error| GapReason::Unreadable(error.to_string()))? {
        BlobRead::Bytes(bytes) if bytes.len() <= limits.max_file_bytes => bytes,
        BlobRead::Bytes(bytes) => {
            return Err(GapReason::TooLarge {
                bytes: bytes.len(),
                limit: limits.max_file_bytes,
            });
        }
        BlobRead::TooLarge { bytes } => {
            return Err(GapReason::TooLarge {
                bytes: usize::try_from(bytes).unwrap_or(usize::MAX),
                limit: limits.max_file_bytes,
            });
        }
    };
    let source = String::from_utf8(bytes).map_err(|_| GapReason::NotText)?;
    let (result, outcome, fault) =
        extract_file_checked(config, Path::new(path.as_str()), &source, content_hex);
    if let Some(error) = result.error {
        return Err(GapReason::ExtractionFailed(error));
    }
    if let HandlerOutcome::Failed(cause) = outcome {
        return Err(GapReason::ExtractionFailed(cause));
    }
    if let Some(fault) = fault {
        return Err(GapReason::SyntaxError(fault));
    }
    // The snapshot id of a per-file batch is a placeholder: the facts are
    // re-pinned to the revision when they are assembled.
    let mut builder = FactBatchBuilder::new(SnapshotId::new(1));
    builder.add_extraction(&result);
    Ok(builder.finish_stable())
}

/// Merges per-file facts into the facts of one snapshot: pinned to the
/// snapshot, one assertion per triple, ids sequential in path order.
fn assemble(snapshot: SnapshotId, per_file: BTreeMap<PathKey, Vec<Fact>>) -> RevisionFacts {
    let mut seen = BTreeSet::new();
    let mut facts = Vec::new();
    let mut path_facts: BTreeMap<PathKey, Vec<FactId>> = BTreeMap::new();
    for (path, file_facts) in per_file {
        for mut fact in file_facts {
            let key = (
                fact.subject,
                fact.predicate.as_str().to_string(),
                format!("{:?}", fact.object),
            );
            if !seen.insert(key) {
                continue;
            }
            fact.snapshot = snapshot;
            fact.id = FactId::new(facts.len() as u64 + 1);
            path_facts.entry(path.clone()).or_default().push(fact.id);
            facts.push(fact);
        }
    }
    RevisionFacts {
        snapshot,
        facts,
        path_facts,
    }
}

// ----- what a comparison may say ------------------------------------

/// How much of the semantic change the facts can vouch for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImpactStatus {
    /// Every changed path is covered by complete facts.
    Evaluated,
    /// The counts are a lower bound: some change is invisible to the
    /// facts, or some facts are missing.
    Partial,
    /// There is nothing to compare.
    NotEvaluated,
}

impl ImpactStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Evaluated => "evaluated",
            Self::Partial => "partial",
            Self::NotEvaluated => "not_evaluated",
        }
    }
}

/// The semantic impact of a transition, as far as the facts see it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactAssessment {
    pub status: ImpactStatus,
    pub reason: String,
    pub facts_added: usize,
    pub facts_removed: usize,
    /// Changed paths no language reader claims.
    pub unanalyzed_changed: Vec<PathKey>,
    /// Changed paths the SCM could not classify.
    pub unclassified: Vec<PathKey>,
    /// Paths whose facts are missing on either side.
    pub gap_paths: Vec<PathKey>,
}

/// What a comparison can say about the reuse of prior work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReuseStatus {
    /// Every piece of prior work has a proven verdict.
    Evaluated,
    /// Some verdicts are unknown, or no verdict can be trusted as
    /// reusable because facts are missing.
    Partial,
    /// There is no prior execution to reuse.
    NotEvaluated,
}

impl ReuseStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Evaluated => "evaluated",
            Self::Partial => "partial",
            Self::NotEvaluated => "not_evaluated",
        }
    }
}

/// The verdict for one piece of prior work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkVerdict {
    pub work: String,
    pub disposition: WorkDisposition,
}

/// Reuse of prior work over a transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReuseAssessment {
    pub status: ReuseStatus,
    pub reason: String,
    pub works: Vec<WorkVerdict>,
}

impl ReuseAssessment {
    fn not_evaluated(reason: impl Into<String>) -> Self {
        Self {
            status: ReuseStatus::NotEvaluated,
            reason: reason.into(),
            works: Vec::new(),
        }
    }

    pub fn count(&self, disposition: WorkDisposition) -> usize {
        self.works
            .iter()
            .filter(|work| work.disposition == disposition)
            .count()
    }
}

fn paths_of(changes: &ChangeSet, keep: impl Fn(&PathKey) -> bool) -> Vec<PathKey> {
    let mut paths: Vec<PathKey> = changes
        .changes()
        .iter()
        .map(|change| change.path().clone())
        .filter(|path| keep(path))
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// The impact the two revisions' facts support.
///
/// # Errors
/// The delta could not be computed (the two revisions share a snapshot
/// identity); callers state that as not evaluated.
pub fn assess_impact(
    base: &ProducedRevision,
    head: &ProducedRevision,
    changes: &ChangeSet,
) -> Result<ImpactAssessment, ScmDeltaError> {
    let scm = delta_from_change_set(&base.facts, &head.facts, changes)?;
    let unanalyzed_changed = paths_of(changes, |path| {
        base.unanalyzed.contains(path) || head.unanalyzed.contains(path)
    });
    let gap_paths: Vec<PathKey> = base
        .coverage
        .gaps
        .iter()
        .chain(head.coverage.gaps.iter())
        .map(|gap| gap.path.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let unclassified = scm.unclassified;
    let mut limits = Vec::new();
    if !gap_paths.is_empty() {
        limits.push(format!(
            "facts are missing for {} analyzable path(s)",
            gap_paths.len()
        ));
    }
    if !unanalyzed_changed.is_empty() {
        limits.push(format!(
            "{} changed path(s) have no language reader",
            unanalyzed_changed.len()
        ));
    }
    if !unclassified.is_empty() {
        limits.push(format!(
            "{} change(s) the SCM could not classify",
            unclassified.len()
        ));
    }
    let (status, reason) = if limits.is_empty() {
        (
            ImpactStatus::Evaluated,
            "the fact delta covers every changed path".to_string(),
        )
    } else {
        (
            ImpactStatus::Partial,
            format!("the counts are a lower bound: {}", limits.join("; ")),
        )
    };
    Ok(ImpactAssessment {
        status,
        reason,
        facts_added: scm.delta.added.len(),
        facts_removed: scm.delta.removed.len(),
        unanalyzed_changed,
        unclassified,
        gap_paths,
    })
}

/// The prior executions recorded against `base`: every detector run
/// whose evidence snapshot is `base`, as the planner's dependency. Records
/// kept for other bases are not consumed.
///
/// A record whose read-set was truncated stays truncated, so the planner
/// answers `Unknown` for it. A record whose detector id is not a valid
/// work name is skipped.
pub fn dependencies_on(index: &DetectorReuseIndex, base: SnapshotId) -> Vec<ExecutionDependency> {
    index
        .records
        .iter()
        .filter(|record| record.produced_snapshot == base)
        .filter_map(|record| {
            let id = record.identity.detector_id.as_str().to_string();
            let work = WorkId::new(NamespacedName::new(id).ok()?);
            let mut read_set: ReadSet = record.observed_facts.iter().copied().collect();
            if record.observed_truncated {
                read_set = truncated(&record.observed_facts);
            }
            Some(ExecutionDependency {
                work,
                execution: ExecutionId(0),
                read_set,
            })
        })
        .collect()
}

/// The read-set of a run whose recording was cut short. Every observed
/// fact stays tracked for invalidation, and one extra fact past the
/// bound marks the set truncated, so the planner can only answer
/// `Unknown` for it.
fn truncated(observed: &BTreeSet<FactId>) -> ReadSet {
    use crate::domain::readset::InMemoryReadSetRecorder;
    let mut recorder = InMemoryReadSetRecorder::new(ReadSetConfig {
        max_records: std::num::NonZeroUsize::new(observed.len().max(1)),
    });
    for fact in observed {
        let _ = recorder.record(*fact);
    }
    let past_the_bound = observed.iter().next_back().map_or(1, |last| last.0 + 1);
    let _ = recorder.record(FactId::new(past_the_bound));
    let _ = recorder.record(FactId::new(past_the_bound + 1));
    recorder.finalize()
}

/// The reuse the prior executions support over a transition, from the one
/// existing planner.
///
/// With no prior execution recorded against the base snapshot the answer
/// is `NotEvaluated`. With facts missing on either side no work is
/// reported reusable: an `Unaffected` verdict over facts that could be
/// absent is downgraded to `Unknown`.
///
/// # Errors
/// The delta could not be computed.
pub fn assess_reuse(
    base: &ProducedRevision,
    head: &ProducedRevision,
    changes: &ChangeSet,
    index: Option<&DetectorReuseIndex>,
) -> Result<ReuseAssessment, ScmDeltaError> {
    let Some(index) = index else {
        return Ok(ReuseAssessment::not_evaluated(
            "no prior execution read-sets are stored for this workspace",
        ));
    };
    let dependencies = dependencies_on(index, base.facts.snapshot);
    if dependencies.is_empty() {
        return Ok(ReuseAssessment::not_evaluated(
            "no prior execution read-set is recorded against the base revision",
        ));
    }
    let manifests: BTreeSet<PathKey> = base.manifests.union(&head.manifests).cloned().collect();
    let plan: AffectedWorkPlan =
        plan_between(&base.facts, &head.facts, changes, &manifests, &dependencies)?;
    let incomplete = !base.coverage.is_complete() || !head.coverage.is_complete();
    let works: Vec<WorkVerdict> = plan
        .decisions
        .iter()
        .map(|decision| WorkVerdict {
            work: decision.work.as_namespaced_name().to_string(),
            disposition: if incomplete && decision.disposition == WorkDisposition::Unaffected {
                WorkDisposition::Unknown
            } else {
                decision.disposition
            },
        })
        .collect();
    let all_proven = works
        .iter()
        .all(|work| work.disposition != WorkDisposition::Unknown);
    let (status, reason) = if all_proven {
        (
            ReuseStatus::Evaluated,
            "every prior execution has a proven verdict".to_string(),
        )
    } else if incomplete {
        (
            ReuseStatus::Partial,
            "facts are missing for some analyzable paths, so unchanged work is not proven reusable"
                .to_string(),
        )
    } else {
        (
            ReuseStatus::Partial,
            "some prior executions cannot be proven unaffected and must run again".to_string(),
        )
    };
    Ok(ReuseAssessment {
        status,
        reason,
        works,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::revision_snapshots::RevisionSnapshotStore;
    use crate::domain::findings::detector_reuse::{DetectorRunIdentity, DetectorRunRecord};
    use crate::domain::findings::digest::DetectorDigest;
    use crate::domain::findings::{
        AnalysisCapability, DetectorAuthority, DetectorDigests, DetectorId,
    };
    use crate::domain::scm::fake::FakeSource;
    use crate::domain::scm::{ChangeReader, ProviderCapabilities, SourceRef};

    const ALPHA_V1: &[u8] = b"pub fn alpha() { first(); }\n";
    const ALPHA_V2: &[u8] = b"pub fn alpha() { second(); }\n";
    const BETA: &[u8] = b"pub fn beta() { helper(); }\n";
    const GAMMA: &[u8] = b"pub fn gamma() {}\n";

    fn source() -> SourceRef {
        SourceRef::new("fake", "demo").expect("source")
    }

    /// A revision name, its parents and its files.
    type RevisionSpec<'a> = (&'a str, &'a [&'a str], &'a [(&'a str, &'a [u8])]);

    fn provider(revisions: &[RevisionSpec]) -> FakeSource {
        revisions.iter().fold(
            FakeSource::new(
                "fake",
                ProviderCapabilities {
                    ancestor_queries: true,
                    ..ProviderCapabilities::NONE
                },
            ),
            |fake, (name, parents, files)| fake.with_revision(name, parents, files),
        )
    }

    fn revision(fake: &FakeSource, name: &str) -> SourceRevision {
        fake.resolve(&source(), name).expect("revision")
    }

    fn produce(
        fake: &FakeSource,
        name: &str,
        objects: Option<&FactObjectStore>,
    ) -> ProducedRevision {
        produce_revision_facts(
            fake,
            &revision(fake, name),
            objects,
            &FactsLimits::default(),
        )
        .expect("facts")
    }

    fn path(text: &str) -> PathKey {
        PathKey::new(text).expect("path")
    }

    fn triples(facts: &RevisionFacts) -> BTreeSet<(u64, String, String)> {
        facts
            .facts
            .iter()
            .map(|f| {
                (
                    f.subject.0,
                    f.predicate.as_str().to_string(),
                    format!("{:?}", f.object),
                )
            })
            .collect()
    }

    fn identity(key: &str) -> DetectorRunIdentity {
        let digest = |s: &str| DetectorDigest::from_content(s);
        DetectorRunIdentity {
            detector_id: DetectorId::new(key).expect("detector id"),
            version: "1".to_string(),
            authority: DetectorAuthority::Gated,
            digests: DetectorDigests {
                logic: digest("logic"),
                policy: digest("policy"),
                semantic: digest(key),
                instance: digest("instance"),
            },
            effective_capabilities: [AnalysisCapability::AstPattern].into_iter().collect(),
            parser: digest("parser"),
            sanitizer: digest("sanitizer"),
        }
    }

    fn index_reading(
        base: &ProducedRevision,
        reads: &[(&str, &str)],
        truncated: bool,
    ) -> DetectorReuseIndex {
        let mut index = DetectorReuseIndex::empty();
        for (detector, file) in reads {
            let observed = base.facts.path_facts[&path(file)].iter().copied().collect();
            index.remember(DetectorRunRecord {
                identity: identity(detector),
                observed_facts: observed,
                observed_truncated: truncated,
                produced_snapshot: base.facts.snapshot,
                findings: Vec::new(),
            });
        }
        index
    }

    fn two_branches() -> FakeSource {
        provider(&[
            (
                "main",
                &[],
                &[
                    ("src/a.rs", ALPHA_V1),
                    ("src/b.rs", BETA),
                    ("README.md", b"docs"),
                ],
            ),
            (
                "feature",
                &["main"],
                &[
                    ("src/a.rs", ALPHA_V2),
                    ("src/b.rs", BETA),
                    ("README.md", b"docs"),
                ],
            ),
        ])
    }

    #[test]
    fn a_revision_yields_pinned_deterministic_facts_with_a_path_index() {
        let fake = two_branches();
        let first = produce(&fake, "main", None);
        let second = produce(&fake, "main", None);

        assert!(!first.facts.facts.is_empty());
        assert!(
            first
                .facts
                .facts
                .iter()
                .all(|f| f.snapshot == first.facts.snapshot)
        );
        assert_eq!(first.facts.facts, second.facts.facts);
        assert_eq!(first.facts.path_facts, second.facts.path_facts);
        assert!(first.coverage.is_complete());
        assert_eq!(first.coverage.analyzable_files, 2);
        assert_eq!(first.coverage.unanalyzed_files, 1);
        let ids: Vec<u64> = first.facts.facts.iter().map(|f| f.id.0).collect();
        assert_eq!(ids, (1..=ids.len() as u64).collect::<Vec<_>>());
    }

    #[test]
    fn incremental_assembly_equals_a_full_rebuild() {
        let fake = two_branches();
        let dir = tempfile::tempdir().expect("dir");
        let objects = FactObjectStore::new(dir.path());
        let _warm = produce(&fake, "main", Some(&objects));

        let incremental = produce(&fake, "feature", Some(&objects));
        let rebuilt = produce(&fake, "feature", None);

        assert_eq!(incremental.coverage.reused_files, 1, "b.rs is shared");
        assert_eq!(incremental.coverage.parsed_files, 1, "a.rs changed");
        assert_eq!(rebuilt.coverage.reused_files, 0);
        assert_eq!(incremental.facts.facts, rebuilt.facts.facts);
        assert_eq!(incremental.facts.path_facts, rebuilt.facts.path_facts);
        assert_eq!(incremental.snapshot, rebuilt.snapshot);
    }

    /// Three histories of one tree: `main` has three files, `deleted` drops
    /// one of them, `renamed` moves another to a new path with the same bytes.
    fn deletion_and_rename() -> FakeSource {
        provider(&[
            (
                "main",
                &[],
                &[
                    ("src/a.rs", ALPHA_V1),
                    ("src/b.rs", BETA),
                    ("src/c.rs", GAMMA),
                ],
            ),
            (
                "deleted",
                &["main"],
                &[("src/a.rs", ALPHA_V1), ("src/c.rs", GAMMA)],
            ),
            (
                "renamed",
                &["main"],
                &[
                    ("src/alpha.rs", ALPHA_V1),
                    ("src/b.rs", BETA),
                    ("src/c.rs", GAMMA),
                ],
            ),
        ])
    }

    fn paths_of(produced: &ProducedRevision) -> Vec<&str> {
        produced
            .facts
            .path_facts
            .keys()
            .map(PathKey::as_str)
            .collect()
    }

    /// What was parsed and what was reused, as `(parsed, reused)`.
    fn work_of(produced: &ProducedRevision) -> (usize, usize) {
        (
            produced.coverage.parsed_files,
            produced.coverage.reused_files,
        )
    }

    #[test]
    fn a_deleted_file_leaves_no_facts_and_costs_no_parsing() {
        let fake = deletion_and_rename();
        let dir = tempfile::tempdir().expect("dir");
        let objects = FactObjectStore::new(dir.path());
        let _warm = produce(&fake, "main", Some(&objects));

        let incremental = produce(&fake, "deleted", Some(&objects));
        let rebuilt = produce(&fake, "deleted", None);

        assert_eq!(paths_of(&incremental), ["src/a.rs", "src/c.rs"]);
        assert_eq!(work_of(&incremental), (0, 2), "every survivor is reused");
        assert_eq!(incremental.coverage.analyzable_files, 2);
        assert_eq!(incremental.facts.facts, rebuilt.facts.facts);
        assert_eq!(incremental.facts.path_facts, rebuilt.facts.path_facts);
        assert_eq!(incremental.snapshot, rebuilt.snapshot);
        let kept: BTreeSet<_> = incremental
            .snapshot
            .files
            .keys()
            .map(PathKey::as_str)
            .collect();
        assert_eq!(kept, BTreeSet::from(["src/a.rs", "src/c.rs"]));
    }

    #[test]
    fn a_renamed_file_is_parsed_again_under_its_new_path() {
        let fake = deletion_and_rename();
        let dir = tempfile::tempdir().expect("dir");
        let objects = FactObjectStore::new(dir.path());
        let _warm = produce(&fake, "main", Some(&objects));

        let incremental = produce(&fake, "renamed", Some(&objects));
        let rebuilt = produce(&fake, "renamed", None);

        // The object is addressed by path and bytes: the same bytes under
        // another path are another object, never the old path's facts.
        assert_eq!(
            paths_of(&incremental),
            ["src/alpha.rs", "src/b.rs", "src/c.rs"]
        );
        assert_eq!(
            work_of(&incremental),
            (1, 2),
            "only the moved file is parsed"
        );
        assert_eq!(incremental.facts.facts, rebuilt.facts.facts);
        assert_eq!(incremental.facts.path_facts, rebuilt.facts.path_facts);
        assert_eq!(incremental.snapshot, rebuilt.snapshot);
    }

    #[test]
    fn the_order_revisions_are_assembled_in_changes_the_work_and_never_the_facts() {
        let fake = deletion_and_rename();
        let cold: BTreeMap<&str, ProducedRevision> = ["main", "deleted", "renamed"]
            .into_iter()
            .map(|name| (name, produce(&fake, name, None)))
            .collect();

        // (revision, parsed, reused) in the order each history is assembled.
        let forward = [("main", 3, 0), ("deleted", 0, 2), ("renamed", 1, 2)];
        let backward = [("renamed", 3, 0), ("deleted", 1, 1), ("main", 0, 3)];

        for (order, expected) in [("forward", forward), ("backward", backward)] {
            let dir = tempfile::tempdir().expect("dir");
            let objects = FactObjectStore::new(dir.path());
            for (name, parsed, reused) in expected {
                let incremental = produce(&fake, name, Some(&objects));
                let rebuilt = &cold[name];
                assert_eq!(
                    work_of(&incremental),
                    (parsed, reused),
                    "{order}: {name} parsed and reused"
                );
                assert_eq!(
                    incremental.facts.facts, rebuilt.facts.facts,
                    "{order}: {name}"
                );
                assert_eq!(
                    incremental.facts.path_facts, rebuilt.facts.path_facts,
                    "{order}: {name}"
                );
                assert_eq!(incremental.snapshot, rebuilt.snapshot, "{order}: {name}");
            }
        }
    }

    #[test]
    fn the_assembly_is_the_union_of_a_one_shot_batch() {
        use crate::application::ingest::extractor::extract_file;
        let fake = two_branches();
        let produced = produce(&fake, "main", None);

        let mut batch = FactBatchBuilder::new(produced.facts.snapshot);
        for (file, bytes) in [("src/a.rs", ALPHA_V1), ("src/b.rs", BETA)] {
            let text = std::str::from_utf8(bytes).expect("utf8");
            let hash = crate::domain::scm::ContentDigest::from_bytes(bytes);
            let result = extract_file(
                language_for_extension("rs").expect("Rust is supported"),
                Path::new(file),
                text,
                hash.as_hex(),
            );
            batch.add_extraction(&result);
        }
        let one_shot = RevisionFacts {
            snapshot: produced.facts.snapshot,
            facts: batch.finish_stable(),
            path_facts: Default::default(),
        };

        assert_eq!(triples(&one_shot), triples(&produced.facts));
    }

    #[test]
    fn a_saved_snapshot_is_reloaded_for_the_same_producer_only() {
        let fake = two_branches();
        let cache = tempfile::tempdir().expect("cache");
        let store = RevisionSnapshotStore::for_workspace(cache.path(), Path::new("/ws"));
        let produced = produce(&fake, "main", Some(&FactObjectStore::beside(&store)));

        store.save(&produced.snapshot).expect("save");

        let loaded = store
            .load(produced.revision.handle(), &producer_versions())
            .expect("the snapshot is reusable");
        assert_eq!(loaded, produced.snapshot);
        assert_eq!(
            loaded.files.len(),
            2,
            "one extraction object per analyzable file"
        );
        assert_eq!(loaded.coverage, "full");
        let foreign = ProducerVersions::new("other", "none", "other", "none", "none");
        assert!(store.load(produced.revision.handle(), &foreign).is_none());
    }

    #[test]
    fn two_branches_share_the_objects_of_identical_bytes() {
        let fake = provider(&[
            ("main", &[], &[("src/a.rs", ALPHA_V1), ("src/c.rs", GAMMA)]),
            ("other", &[], &[("src/a.rs", ALPHA_V2), ("src/c.rs", GAMMA)]),
        ]);
        let dir = tempfile::tempdir().expect("dir");
        let objects = FactObjectStore::new(dir.path());
        let _ = produce(&fake, "main", Some(&objects));

        let other = produce(&fake, "other", Some(&objects));

        assert_eq!(other.coverage.reused_files, 1);
        assert_eq!(other.coverage.parsed_files, 1);
    }

    #[test]
    fn a_read_only_store_serves_stored_objects_and_never_writes() {
        let fake = two_branches();
        let dir = tempfile::tempdir().expect("dir");
        let writable = FactObjectStore::new(dir.path().join("objects"));
        let _ = produce(&fake, "main", Some(&writable));
        let stored = std::fs::read_dir(dir.path().join("objects"))
            .expect("objects")
            .count();
        let read_only = writable.clone().read_only();

        let other = produce(&fake, "feature", Some(&read_only));

        assert_eq!(other.coverage.reused_files, 1, "stored bytes are served");
        assert_eq!(other.coverage.parsed_files, 1, "changed bytes are parsed");
        assert_eq!(
            std::fs::read_dir(dir.path().join("objects"))
                .expect("objects")
                .count(),
            stored,
            "the parsed object of the feature branch was not written"
        );
    }

    #[test]
    fn a_read_only_store_over_a_missing_directory_creates_nothing() {
        let fake = two_branches();
        let dir = tempfile::tempdir().expect("dir");
        let read_only = FactObjectStore::new(dir.path().join("objects")).read_only();

        let _ = produce(&fake, "main", Some(&read_only));

        assert!(!dir.path().join("objects").exists());
    }

    #[test]
    fn a_corrupt_object_is_a_miss_not_evidence() {
        let fake = two_branches();
        let dir = tempfile::tempdir().expect("dir");
        let objects = FactObjectStore::new(dir.path());
        let _ = produce(&fake, "main", Some(&objects));
        for entry in std::fs::read_dir(dir.path()).expect("objects") {
            std::fs::write(entry.expect("entry").path(), b"{ not json").expect("corrupt");
        }

        let again = produce(&fake, "main", Some(&objects));

        assert_eq!(again.coverage.reused_files, 0);
        assert_eq!(again.coverage.parsed_files, 2);
        assert_eq!(again.facts.facts, produce(&fake, "main", None).facts.facts);
    }

    #[test]
    fn an_object_whose_facts_were_edited_in_place_is_a_miss_not_evidence() {
        let fake = two_branches();
        let dir = tempfile::tempdir().expect("dir");
        let objects = FactObjectStore::new(dir.path());
        let _ = produce(&fake, "main", Some(&objects));
        for entry in std::fs::read_dir(dir.path()).expect("objects") {
            let file = entry.expect("entry").path();
            let mut object: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&file).expect("read"))
                    .expect("a stored object is JSON");
            let facts = object["facts"].as_array_mut().expect("facts array");
            assert!(!facts.is_empty(), "the fixture has facts to tamper with");
            facts.pop();
            std::fs::write(&file, serde_json::to_vec(&object).expect("json")).expect("tamper");
        }

        let again = produce(&fake, "main", Some(&objects));

        assert_eq!(
            (again.coverage.parsed_files, again.coverage.reused_files),
            (2, 0)
        );
        assert_eq!(again.facts.facts, produce(&fake, "main", None).facts.facts);
    }

    #[test]
    fn an_unreadable_file_is_a_gap_and_the_impact_is_partial() {
        let fake = provider(&[
            (
                "main",
                &[],
                &[("src/a.rs", ALPHA_V1), ("src/bad.rs", &[0xff, 0xfe, 0x00])],
            ),
            (
                "feature",
                &["main"],
                &[("src/a.rs", ALPHA_V2), ("src/bad.rs", &[0xff, 0xfe, 0x00])],
            ),
        ]);
        let base = produce(&fake, "main", None);
        let head = produce(&fake, "feature", None);
        let changes = fake
            .changes_between(&base.revision, &head.revision)
            .expect("changes");

        assert!(!head.coverage.is_complete());
        assert_eq!(head.coverage.gaps[0].path, path("src/bad.rs"));
        assert_eq!(head.coverage.gaps[0].reason, GapReason::NotText);
        assert_eq!(head.snapshot.coverage, "partial: 1 file(s) without facts");
        let impact = assess_impact(&base, &head, &changes).expect("impact");
        assert_eq!(impact.status, ImpactStatus::Partial);
        assert_eq!(impact.gap_paths, vec![path("src/bad.rs")]);
        assert!(impact.facts_added > 0 && impact.facts_removed > 0);
    }

    /// Source that tree-sitter recovers from silently: each file declares
    /// something the broken syntax then hides from the extractor.
    const BROKEN_SOURCES: [(&str, &[u8]); 4] = [
        ("src/broken.rs", b"pub fn kept() {}\npub fn lost( {{{\n"),
        (
            "src/Broken.java",
            b"class Broken { void lost( { }\nclass Other {\n",
        ),
        ("src/broken.kt", b"fun lost( { \nclass Other {\n"),
        ("src/broken.go", b"package demo\nfunc Lost( {{{\n"),
    ];

    #[test]
    fn a_syntax_error_is_a_gap_in_every_language_and_never_partial_facts() {
        for (file, source) in BROKEN_SOURCES {
            let fake = provider(&[("main", &[], &[("src/ok.rs", GAMMA), (file, source)])]);

            let cold = produce(&fake, "main", None);

            assert!(!cold.coverage.is_complete(), "{file} must be a gap");
            let [gap] = cold.coverage.gaps.as_slice() else {
                panic!("{file}: exactly one gap expected: {:?}", cold.coverage.gaps);
            };
            assert_eq!(gap.path, path(file));
            assert!(
                gap.reason.describe().contains("syntax error at line"),
                "{file}: {}",
                gap.reason.describe()
            );
            assert_eq!(paths_of(&cold), ["src/ok.rs"], "{file} publishes no facts");
            assert_eq!(cold.snapshot.coverage, "partial: 1 file(s) without facts");
        }
    }

    #[test]
    fn a_syntax_error_is_a_gap_when_the_store_is_warm_too() {
        let (file, source) = BROKEN_SOURCES[0];
        let fake = provider(&[("main", &[], &[("src/ok.rs", GAMMA), (file, source)])]);
        let dir = tempfile::tempdir().expect("dir");
        let objects = FactObjectStore::new(dir.path());
        let _warm = produce(&fake, "main", Some(&objects));

        let incremental = produce(&fake, "main", Some(&objects));
        let rebuilt = produce(&fake, "main", None);

        assert_eq!(incremental.coverage.gaps, rebuilt.coverage.gaps);
        assert_eq!(incremental.coverage.gaps.len(), 1);
        assert_eq!(
            work_of(&incremental),
            (0, 1),
            "the clean file is reused; the broken one is never stored"
        );
        assert_eq!(incremental.snapshot, rebuilt.snapshot);
    }

    #[test]
    fn well_formed_files_stay_gap_free() {
        let fake = provider(&[(
            "main",
            &[],
            &[
                ("src/a.rs", ALPHA_V1),
                (
                    "src/Good.java",
                    b"package demo;\npublic class Good { void run() { } }\n",
                ),
                (
                    "src/good.kt",
                    b"package demo\nclass Good { fun run() {} }\n",
                ),
                ("src/good.go", b"package demo\nfunc Run() {}\n"),
            ],
        )]);

        let produced = produce(&fake, "main", None);

        assert!(produced.coverage.is_complete(), "{:?}", produced.coverage);
        assert_eq!(produced.coverage.analyzable_files, 4);
    }

    /// Plants, under the key `producer` would have used, an object that
    /// claims `file` has no facts at all.
    fn plant_object_of(producer: &str, dir: &std::path::Path, file: &str, source: &[u8]) {
        let fake = provider(&[("main", &[], &[(file, source)])]);
        let content = fake
            .inventory(&revision(&fake, "main"))
            .expect("inventory")
            .entries()
            .iter()
            .find(|entry| entry.path == path(file))
            .map(|entry| entry.content.as_hex().to_string())
            .expect("the file is tracked");
        let mut hasher = Sha256::new();
        hasher.update(producer.as_bytes());
        hasher.update([0]);
        hasher.update(file.as_bytes());
        hasher.update([0]);
        hasher.update(content.as_bytes());
        let stale = ObjectFile {
            schema: OBJECT_SCHEMA.to_string(),
            path: file.to_string(),
            content,
            artifact: artifact_of(&[]).expect("artifact"),
            facts: Vec::new(),
        };
        std::fs::write(
            dir.join(format!("{}.facts.json", hex(&hasher.finalize()))),
            serde_json::to_vec(&stale).expect("json"),
        )
        .expect("plant the stale object");
    }

    #[test]
    fn an_object_stored_before_syntax_errors_were_gaps_is_never_served() {
        // A broken file used to be stored with the facts of its damaged
        // tree. Those objects sit under an earlier producer's key and must
        // stay unreachable, or a warm store keeps vouching for them.
        let (file, source) = BROKEN_SOURCES[0];
        let fake = provider(&[("main", &[], &[(file, source)])]);
        let dir = tempfile::tempdir().expect("dir");
        plant_object_of("cognicode.revision-facts/v1", dir.path(), file, source);

        let produced = produce(&fake, "main", Some(&FactObjectStore::new(dir.path())));

        assert_eq!(
            produced.coverage.reused_files, 0,
            "the stale object is a miss"
        );
        assert_eq!(produced.coverage.gaps.len(), 1);
    }

    #[test]
    fn an_object_stored_before_go_types_and_bodiless_signatures_is_never_served() {
        // Facts extracted before Go types were reported and bodiless
        // signatures were told apart from functions describe the same bytes
        // differently. They sit under the previous producer's key.
        let fake = provider(&[("main", &[], &[("src/ok.rs", GAMMA)])]);
        let dir = tempfile::tempdir().expect("dir");
        plant_object_of(
            "cognicode.revision-facts/v2",
            dir.path(),
            "src/ok.rs",
            GAMMA,
        );

        let produced = produce(&fake, "main", Some(&FactObjectStore::new(dir.path())));

        assert_eq!(
            produced.coverage.reused_files, 0,
            "the stale object is a miss"
        );
        assert_eq!(produced.coverage.parsed_files, 1);
    }

    #[test]
    fn oversized_files_and_the_file_bound_are_gaps_never_silence() {
        let fake = two_branches();
        let tiny = FactsLimits {
            max_files: 1,
            max_file_bytes: 8,
        };
        let produced =
            produce_revision_facts(&fake, &revision(&fake, "main"), None, &tiny).expect("facts");

        let reasons: Vec<&GapReason> = produced.coverage.gaps.iter().map(|g| &g.reason).collect();
        assert!(
            reasons
                .iter()
                .any(|reason| matches!(reason, GapReason::TooLarge { .. }))
        );
        assert!(
            reasons
                .iter()
                .any(|reason| matches!(reason, GapReason::FileLimitReached { limit: 1 }))
        );
        assert!(produced.facts.facts.is_empty());
        assert!(!produced.coverage.is_complete());
    }

    #[test]
    fn complete_facts_give_an_evaluated_impact_over_analyzed_changes() {
        let fake = two_branches();
        let base = produce(&fake, "main", None);
        let head = produce(&fake, "feature", None);
        let changes = fake
            .changes_between(&base.revision, &head.revision)
            .expect("changes");

        let impact = assess_impact(&base, &head, &changes).expect("impact");

        assert_eq!(impact.status, ImpactStatus::Evaluated);
        assert!(impact.facts_added > 0 && impact.facts_removed > 0);
        assert!(impact.unanalyzed_changed.is_empty());
    }

    #[test]
    fn a_changed_path_no_reader_claims_makes_the_impact_partial() {
        let fake = provider(&[
            (
                "main",
                &[],
                &[("src/a.rs", ALPHA_V1), ("notes.txt", b"one")],
            ),
            (
                "feature",
                &["main"],
                &[("src/a.rs", ALPHA_V1), ("notes.txt", b"two")],
            ),
        ]);
        let base = produce(&fake, "main", None);
        let head = produce(&fake, "feature", None);
        let changes = fake
            .changes_between(&base.revision, &head.revision)
            .expect("changes");

        let impact = assess_impact(&base, &head, &changes).expect("impact");

        assert_eq!(impact.status, ImpactStatus::Partial);
        assert_eq!(impact.unanalyzed_changed, vec![path("notes.txt")]);
        assert_eq!((impact.facts_added, impact.facts_removed), (0, 0));
    }

    #[test]
    fn without_prior_read_sets_reuse_is_not_evaluated_with_a_reason() {
        let fake = two_branches();
        let base = produce(&fake, "main", None);
        let head = produce(&fake, "feature", None);
        let changes = fake
            .changes_between(&base.revision, &head.revision)
            .expect("changes");

        let none = assess_reuse(&base, &head, &changes, None).expect("reuse");
        let empty = assess_reuse(&base, &head, &changes, Some(&DetectorReuseIndex::empty()))
            .expect("reuse");

        for answer in [none, empty] {
            assert_eq!(answer.status, ReuseStatus::NotEvaluated);
            assert!(answer.reason.contains("read-set"), "{}", answer.reason);
            assert!(answer.works.is_empty());
        }
    }

    #[test]
    fn prior_read_sets_decide_which_work_is_reusable() {
        let fake = two_branches();
        let base = produce(&fake, "main", None);
        let head = produce(&fake, "feature", None);
        let changes = fake
            .changes_between(&base.revision, &head.revision)
            .expect("changes");
        let index = index_reading(
            &base,
            &[
                ("detector.reads_a", "src/a.rs"),
                ("detector.reads_b", "src/b.rs"),
            ],
            false,
        );

        let answer = assess_reuse(&base, &head, &changes, Some(&index)).expect("reuse");

        let verdict = |name: &str| {
            answer
                .works
                .iter()
                .find(|w| w.work == name)
                .unwrap_or_else(|| panic!("no verdict for {name}: {answer:?}"))
                .disposition
        };
        assert_eq!(verdict("detector.reads_a"), WorkDisposition::Affected);
        assert_eq!(verdict("detector.reads_b"), WorkDisposition::Unaffected);
        assert_eq!(answer.status, ReuseStatus::Evaluated);
    }

    #[test]
    fn a_truncated_read_set_is_never_reusable() {
        let fake = two_branches();
        let base = produce(&fake, "main", None);
        let head = produce(&fake, "feature", None);
        let changes = fake
            .changes_between(&base.revision, &head.revision)
            .expect("changes");
        let index = index_reading(&base, &[("detector.reads_b", "src/b.rs")], true);

        let answer = assess_reuse(&base, &head, &changes, Some(&index)).expect("reuse");

        assert_eq!(answer.works[0].disposition, WorkDisposition::Unknown);
        assert_eq!(answer.status, ReuseStatus::Partial);
    }

    #[test]
    fn incomplete_facts_never_report_work_as_reusable() {
        let fake = provider(&[
            (
                "main",
                &[],
                &[("src/b.rs", BETA), ("src/bad.rs", &[0xff, 0x00])],
            ),
            (
                "feature",
                &["main"],
                &[("src/b.rs", BETA), ("src/bad.rs", &[0xfe, 0x00])],
            ),
        ]);
        let base = produce(&fake, "main", None);
        let head = produce(&fake, "feature", None);
        let changes = fake
            .changes_between(&base.revision, &head.revision)
            .expect("changes");
        let index = index_reading(&base, &[("detector.reads_b", "src/b.rs")], false);

        let answer = assess_reuse(&base, &head, &changes, Some(&index)).expect("reuse");

        assert_ne!(answer.works[0].disposition, WorkDisposition::Unaffected);
        assert_eq!(answer.status, ReuseStatus::Partial);
        assert!(answer.reason.contains("missing"));
    }

    #[test]
    fn a_manifest_change_leaves_no_work_reusable() {
        let fake = provider(&[
            (
                "main",
                &[],
                &[("src/b.rs", BETA), ("Cargo.toml", b"[package]\n")],
            ),
            (
                "feature",
                &["main"],
                &[
                    ("src/b.rs", BETA),
                    ("Cargo.toml", b"[package]\nname=\"x\"\n"),
                ],
            ),
        ]);
        let base = produce(&fake, "main", None);
        let head = produce(&fake, "feature", None);
        let changes = fake
            .changes_between(&base.revision, &head.revision)
            .expect("changes");
        let index = index_reading(&base, &[("detector.reads_b", "src/b.rs")], false);

        assert!(head.manifests.contains(&path("Cargo.toml")));
        let answer = assess_reuse(&base, &head, &changes, Some(&index)).expect("reuse");

        assert_eq!(answer.works[0].disposition, WorkDisposition::Unknown);
    }

    // ----- Helm chart templates -----------------------------------------

    use super::super::helm_templates::{ChartFiles, ChartRender};
    use std::cell::RefCell;

    /// A labelled test double: it answers what it was told, whatever the
    /// chart, and records every chart it was asked about. It renders
    /// nothing; it stands in for an engine whose verdict is the input of
    /// the producer under test.
    struct ScriptedRender {
        answer: ChartRender,
        asked: RefCell<Vec<ChartFiles>>,
    }

    impl ScriptedRender {
        fn answering(answer: ChartRender) -> Self {
            Self {
                answer,
                asked: RefCell::new(Vec::new()),
            }
        }

        fn rendering() -> Self {
            Self::answering(ChartRender::Rendered)
        }

        fn refusing(cause: &str) -> Self {
            Self::answering(ChartRender::NotRendered {
                cause: cause.to_string(),
            })
        }
    }

    impl ChartRenderCheck for ScriptedRender {
        fn check(&self, chart: &ChartFiles) -> ChartRender {
            self.asked.borrow_mut().push(chart.clone());
            self.answer.clone()
        }
    }

    const CHART_YAML: &[u8] = b"apiVersion: v2\nname: web\nversion: 0.1.0\n";
    const VALUES: &[u8] = b"replicas: 2\n";
    const DEPLOYMENT: &[u8] = b"apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: {{ .Release.Name }}\nspec:\n  replicas: {{ .Values.replicas }}\n";
    const SERVICE: &[u8] = b"{{- if .Values.service.enabled }}\nkind: Service\n{{- end }}\n";

    fn chart_revision() -> FakeSource {
        provider(&[(
            "main",
            &[],
            &[
                ("src/ok.rs", GAMMA),
                ("README.md", b"docs"),
                ("deploy/web/Chart.yaml", CHART_YAML),
                ("deploy/web/values.yaml", VALUES),
                ("deploy/web/templates/deployment.yaml", DEPLOYMENT),
                ("deploy/web/templates/service.yaml", SERVICE),
            ],
        )])
    }

    fn produce_with(
        fake: &FakeSource,
        objects: Option<&FactObjectStore>,
        charts: &dyn ChartRenderCheck,
    ) -> ProducedRevision {
        produce_revision_facts_rendering(
            fake,
            &revision(fake, "main"),
            objects,
            &FactsLimits::default(),
            charts,
        )
        .expect("facts")
    }

    #[test]
    fn go_template_text_is_never_a_syntax_error_and_never_facts() {
        let fake = chart_revision();

        let produced = produce(&fake, "main", None);

        assert_eq!(produced.coverage.gaps.len(), 2, "{:?}", produced.coverage);
        for gap in &produced.coverage.gaps {
            let GapReason::TemplateNotRendered { chart, cause } = &gap.reason else {
                panic!("{:?}: not a template gap: {:?}", gap.path, gap.reason);
            };
            assert_eq!(chart, "deploy/web");
            assert!(cause.contains("no chart renderer"), "{cause}");
        }
        assert_eq!(paths_of(&produced), ["src/ok.rs"]);
        assert!(!produced.coverage.is_complete());
        assert_eq!(
            produced.snapshot.coverage,
            "partial: 2 file(s) without facts"
        );
    }

    #[test]
    fn a_chart_that_was_not_rendered_is_a_gap_per_template_with_the_cause() {
        let fake = chart_revision();
        let engine = ScriptedRender::refusing("engine failed: boom");

        let produced = produce_with(&fake, None, &engine);

        let causes: Vec<String> = produced
            .coverage
            .gaps
            .iter()
            .map(|gap| gap.reason.describe())
            .collect();
        assert_eq!(causes.len(), 2);
        for cause in causes {
            assert!(cause.contains("engine failed: boom"), "{cause}");
            assert!(cause.contains("not rendered"), "{cause}");
        }
        assert_ne!(produced.snapshot.coverage, "full");
    }

    #[test]
    fn a_faithfully_rendered_chart_leaves_no_gap_so_the_revision_can_be_complete() {
        let fake = chart_revision();
        let engine = ScriptedRender::rendering();

        let produced = produce_with(&fake, None, &engine);

        assert!(produced.coverage.is_complete(), "{:?}", produced.coverage);
        assert_eq!(produced.snapshot.coverage, "full");
        assert_eq!(produced.coverage.rendered_files, 2);
        assert_eq!(
            produced.coverage.parsed_files, 3,
            "the plain YAML of the chart and src/ok.rs are parsed; the templates are not"
        );
        assert!(
            paths_of(&produced)
                .iter()
                .all(|path| !path.contains("/templates/")),
            "{:?}",
            paths_of(&produced)
        );
        assert!(
            produced
                .unanalyzed
                .contains(&path("deploy/web/templates/service.yaml")),
            "a template's content is invisible to the facts, and a change to it must say so"
        );
    }

    #[test]
    fn a_chart_is_rendered_once_from_exactly_its_own_files() {
        let fake = chart_revision();
        let engine = ScriptedRender::rendering();

        let _ = produce_with(&fake, None, &engine);

        let asked = engine.asked.borrow();
        let [chart] = asked.as_slice() else {
            panic!("one check per chart, got {}", asked.len());
        };
        assert_eq!(chart.root, "deploy/web");
        assert_eq!(
            chart.files.keys().map(String::as_str).collect::<Vec<_>>(),
            [
                "Chart.yaml",
                "templates/deployment.yaml",
                "templates/service.yaml",
                "values.yaml"
            ]
        );
        assert_eq!(chart.files["templates/deployment.yaml"], DEPLOYMENT);
    }

    #[test]
    fn a_templates_directory_outside_a_chart_is_plain_yaml() {
        let fake = provider(&[(
            "main",
            &[],
            &[("docs/templates/a.yaml", b"title: report\n")],
        )]);
        let engine = ScriptedRender::refusing("must not be asked");

        let produced = produce_with(&fake, None, &engine);

        assert!(produced.coverage.is_complete(), "{:?}", produced.coverage);
        assert_eq!(produced.coverage.parsed_files, 1);
        assert!(engine.asked.borrow().is_empty());
    }

    #[test]
    fn a_warm_store_cannot_serve_or_hide_a_template() {
        let fake = chart_revision();
        let dir = tempfile::tempdir().expect("dir");
        let objects = FactObjectStore::new(dir.path());
        // An object for the template under the current producer, as a
        // store written by anything else could hold.
        let template = path("deploy/web/templates/service.yaml");
        let content = fake
            .inventory(&revision(&fake, "main"))
            .expect("inventory")
            .entries()
            .iter()
            .find(|entry| entry.path == template)
            .map(|entry| entry.content.as_hex().to_string())
            .expect("tracked");
        objects
            .write(
                &template,
                &content,
                &[],
                &artifact_of(&[]).expect("artifact"),
            )
            .expect("plant");

        let _prime = produce(&fake, "main", Some(&objects));
        let warm = produce(&fake, "main", Some(&objects));
        let cold = produce(&fake, "main", None);

        assert_eq!(warm.coverage.gaps, cold.coverage.gaps);
        assert_eq!(warm.coverage.gaps.len(), 2);
        assert_eq!(
            warm.coverage.reused_files, 3,
            "src/ok.rs and the chart's plain YAML are reused, never a template"
        );
        assert_eq!(warm.snapshot, cold.snapshot);
    }

    #[test]
    fn the_extraction_contract_change_is_versioned() {
        // Template files stopped being extracted: nothing stored under the
        // previous contract may be served for them.
        assert_eq!(FACTS_PRODUCER_VERSION, "cognicode.revision-facts/v3");
        assert_eq!(producer_versions().facts, FACTS_PRODUCER_VERSION);
        assert!(producer_versions().extractor.contains("revision-facts/v3"));
    }
}
