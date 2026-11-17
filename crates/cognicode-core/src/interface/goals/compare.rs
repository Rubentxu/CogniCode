//! Comparison of two ancestor Git revisions. It never touches the working
//! tree; when a cache is named it records the revision snapshots and the
//! detector read-set of the base revision there.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::application::change_tracking::detector_runs::{
    Reuse, detect_in_cache, detector_id, ensure_run_in_cache,
};
use crate::application::change_tracking::planner::WorkDisposition;
use crate::application::change_tracking::revision_facts::{
    FactObjectStore, FactsLimits, ProducedRevision, assess_impact, assess_reuse,
    produce_revision_facts, producer_versions,
};
use crate::application::revision_snapshots::RevisionSnapshotStore;
use crate::application::services::detector_reuse_store::DetectorReuseStore;
use crate::domain::scm::{
    ChangeKind, ChangeReader, ChangeSet, RevisionReader, SourceRef, WorkingCopyReader,
};
use crate::infrastructure::findings::in_memory_evidence::InMemoryEvidenceStore;
use crate::infrastructure::git::GitScmAdapter;
use crate::interface::goals::GoalEnvironment;

/// Arguments for the `compare` goal.
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct CompareInput {
    /// The ancestor revision to compare from.
    pub base_ref: String,
    /// The descendant revision to compare to.
    pub head_ref: String,
}

/// A tracked revision identified by both its SCM handle and content.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ComparedRevision {
    /// The reference supplied by the caller.
    pub requested_ref: String,
    /// The resolved immutable revision handle.
    pub revision: String,
    /// The digest of all tracked content at this revision.
    pub content_digest: String,
    /// Number of tracked entries read from this revision.
    pub tracked_files: usize,
}

/// One SCM-classified path change.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ComparedChange {
    /// The path at the candidate revision, or the deleted path.
    pub path: String,
    /// `added`, `deleted`, `modified`, `renamed`, `copied`, or `unknown`.
    pub kind: String,
    /// The source path when a rename or copy was evidenced.
    pub previous_path: Option<String>,
    /// Content identity at the base revision, when available.
    pub base_digest: Option<String>,
    /// Content identity at the candidate revision, when available.
    pub candidate_digest: Option<String>,
    /// Whether the change requires re-analysis of content.
    pub requires_reparse: bool,
}

/// Paths staged in the index and paths dirty in the working tree.
///
/// The Git port exposes exact index diffs and dirty path identities, but does
/// not provide working-tree file contents as a revision. Dirty paths are
/// therefore observations, not content classifications.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WorkingCopyComparison {
    /// Changes from HEAD to the staged index, including unmerged paths as unknown.
    pub staged_changes: Vec<ComparedChange>,
    /// Dirty tracked and untracked paths reported by Git status.
    pub dirty_paths: Vec<String>,
}

/// Explicitly scoped coverage for an SCM-only comparison.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CompareCoverage {
    /// Tracked files enumerated at the base revision.
    pub base_tracked_files: usize,
    /// Tracked files enumerated at the candidate revision.
    pub candidate_tracked_files: usize,
    /// Changes reported by the SCM provider.
    pub changed_paths: usize,
    /// Changes the SCM provider could not classify.
    pub unclassified_changes: usize,
    /// How far the semantic analysis reaches: `evaluated`, `partial` or
    /// `not_evaluated`, the same word as `impact_status`.
    pub analysis: String,
}

/// What one revision's facts cover.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RevisionFactsCoverage {
    /// Facts assembled for the revision.
    pub facts: usize,
    /// Tracked files at the revision.
    pub tracked_files: usize,
    /// Tracked files a language reader claims.
    pub analyzable_files: usize,
    /// Files parsed now.
    pub parsed_files: usize,
    /// Files whose facts were reused from a stored object.
    pub reused_files: usize,
    /// Tracked files no language reader claims.
    pub unanalyzed_files: usize,
    /// `full` when every analyzable file has facts, `partial` otherwise.
    pub coverage: String,
    /// Analyzable files left without facts, each with its reason.
    pub gaps: Vec<RevisionFactsGap>,
}

/// An analyzable file that has no facts, and why.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RevisionFactsGap {
    /// The tracked path.
    pub path: String,
    /// Why no facts exist for it.
    pub reason: String,
}

/// The revision facts the comparison was computed from.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CompareRevisionFacts {
    /// The producer that assembled the facts.
    pub producer: String,
    /// Facts of the base revision.
    pub base: RevisionFactsCoverage,
    /// Facts of the candidate revision.
    pub candidate: RevisionFactsCoverage,
    /// Whether both revision snapshots were persisted for reuse across
    /// branches. `false` when the environment names no cache.
    pub persisted: bool,
}

/// The change in facts between the two revisions.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CompareImpact {
    /// Facts asserted at the candidate and not at the base.
    pub facts_added: usize,
    /// Facts asserted at the base and not at the candidate.
    pub facts_removed: usize,
    /// Changed paths no language reader claims: their effect is invisible.
    pub unanalyzed_changed_paths: Vec<String>,
    /// Paths with a missing side of facts.
    pub gap_paths: Vec<String>,
}

/// The reuse verdict of one piece of prior work.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CompareWorkVerdict {
    /// The prior work, by the detector name it ran under.
    pub work: String,
    /// `affected`, `unaffected` or `unknown`.
    pub disposition: String,
}

/// What the prior read-sets say about reusing earlier work.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CompareWorkReuse {
    /// One verdict per prior work recorded against the base revision.
    pub works: Vec<CompareWorkVerdict>,
    /// Works that read a changed fact.
    pub affected: usize,
    /// Works proven independent of every change.
    pub unaffected: usize,
    /// Works that cannot be decided.
    pub unknown: usize,
}

/// One finding of the detector on the candidate revision.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CompareHeadFinding {
    /// The semantic fingerprint that identifies the finding across revisions.
    pub fingerprint: String,
    /// The file the finding is reported in.
    pub file: String,
    /// The line the finding is reported at.
    pub line: u32,
    /// The revision whose facts produced the evidence of this finding: the
    /// base revision for a reused finding, the candidate for a fresh one.
    pub evidence_revision: String,
}

/// The detector's findings on the candidate revision, and whether they were
/// reused from the base revision's run or produced again.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CompareHeadFindings {
    /// The detector that produced the findings.
    pub detector: String,
    /// `reused` only when the findings are the base run's, proven unaffected
    /// by every change; `rerun` when the detector ran on the candidate;
    /// `not_evaluated` when it could not run, with no findings.
    pub reuse: String,
    /// Why the findings were reused, run again, or not evaluated.
    pub reason: String,
    /// `full` when every analyzable file of the candidate has facts,
    /// `partial` otherwise: findings of a partial revision are a lower bound.
    pub coverage: String,
    /// The findings on the candidate revision.
    pub findings: Vec<CompareHeadFinding>,
}

/// The shared response contract for comparing two revisions.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GoalCompare {
    /// A human-readable statement of the result and its limits.
    pub summary: String,
    /// Version of this response contract.
    pub schema_version: String,
    /// The goal that answered.
    pub goal: String,
    /// SCM provider used to produce this comparison.
    pub producer: String,
    /// Base revision identity and coverage.
    pub base: ComparedRevision,
    /// Candidate revision identity and coverage.
    pub candidate: ComparedRevision,
    /// SCM-level coverage; this is not semantic-analysis coverage.
    pub coverage: CompareCoverage,
    /// The classified path changes.
    pub changes: Vec<ComparedChange>,
    /// Staged changes and dirty working-tree paths, observed separately.
    pub working_copy: WorkingCopyComparison,
    /// `evaluated`, `partial` or `not_evaluated`: how much of the semantic
    /// impact the revision facts support. `partial` counts are a lower bound.
    pub impact_status: String,
    /// Why the impact has that status.
    pub impact_reason: String,
    /// `evaluated`, `partial` or `not_evaluated`: what prior read-sets say
    /// about reusing earlier work.
    pub work_reuse_status: String,
    /// Why the reuse has that status.
    pub work_reuse_reason: String,
    /// The facts the comparison was computed from, with their gaps.
    pub revision_facts: Option<CompareRevisionFacts>,
    /// The fact delta, when facts exist for both revisions.
    pub impact: Option<CompareImpact>,
    /// The reuse verdicts, from the read-set the production detector run
    /// wrote for the base revision.
    pub work_reuse: Option<CompareWorkReuse>,
    /// The detector's findings on the candidate revision with their reuse
    /// state; absent when the revision facts could not be assembled.
    pub head_findings: Option<CompareHeadFindings>,
}

const FACTS_UNAVAILABLE: &str = "revision facts could not be assembled";

fn compared_changes(change_set: &ChangeSet) -> Vec<ComparedChange> {
    change_set
        .changes()
        .iter()
        .map(|change| {
            let (kind, previous_path) = match change.kind() {
                ChangeKind::Added => ("added", None),
                ChangeKind::Deleted => ("deleted", None),
                ChangeKind::Modified => ("modified", None),
                ChangeKind::Renamed { from } => ("renamed", Some(from.as_str().to_string())),
                ChangeKind::Copied { from } => ("copied", Some(from.as_str().to_string())),
                ChangeKind::Unknown => ("unknown", None),
            };
            ComparedChange {
                path: change.path().as_str().to_string(),
                kind: kind.to_string(),
                previous_path,
                base_digest: change.before().map(|digest| digest.as_hex().to_string()),
                candidate_digest: change.after().map(|digest| digest.as_hex().to_string()),
                requires_reparse: change.requires_reparse(),
            }
        })
        .collect()
}

pub(super) fn side_coverage(produced: &ProducedRevision) -> RevisionFactsCoverage {
    let coverage = &produced.coverage;
    RevisionFactsCoverage {
        facts: produced.facts.facts.len(),
        tracked_files: coverage.tracked_files,
        analyzable_files: coverage.analyzable_files,
        parsed_files: coverage.parsed_files,
        reused_files: coverage.reused_files,
        unanalyzed_files: coverage.unanalyzed_files,
        coverage: if coverage.is_complete() {
            "full"
        } else {
            "partial"
        }
        .to_string(),
        gaps: coverage
            .gaps
            .iter()
            .map(|gap| RevisionFactsGap {
                path: gap.path.as_str().to_string(),
                reason: gap.reason.describe(),
            })
            .collect(),
    }
}

fn disposition_word(disposition: WorkDisposition) -> &'static str {
    match disposition {
        WorkDisposition::Affected => "affected",
        WorkDisposition::Unaffected => "unaffected",
        WorkDisposition::Unknown => "unknown",
    }
}

/// The detector's findings on the candidate, with the reuse state exactly as
/// `detect_with_reuse` proved it. A detection that could not be made is
/// `not_evaluated` with its reason and no findings, never an empty clean list.
fn head_findings(
    workspace: &Path,
    cache_root: Option<&Path>,
    base: &ProducedRevision,
    candidate: &ProducedRevision,
    change_set: &ChangeSet,
) -> CompareHeadFindings {
    let coverage = side_coverage(candidate).coverage;
    let detector = detector_id()
        .map(|id| id.as_str().to_string())
        .unwrap_or_default();
    match detect_in_cache(cache_root, workspace, base, candidate, change_set) {
        Ok(detection) => {
            let (reuse, reason) = match &detection.reuse {
                Reuse::Reused { .. } => (
                    "reused",
                    "proven unaffected: no change touches a fact the detector read".to_string(),
                ),
                Reuse::Rerun { reason } => ("rerun", reason.clone()),
            };
            let findings = detection
                .findings
                .iter()
                .flat_map(|finding| {
                    let producer = if finding.produced_in == base.facts.snapshot {
                        base
                    } else {
                        candidate
                    };
                    finding
                        .output_locations
                        .iter()
                        .map(move |location| CompareHeadFinding {
                            fingerprint: finding.semantic_fingerprint.as_str().to_string(),
                            file: location.file.clone(),
                            line: location.line,
                            evidence_revision: producer.revision.handle().as_str().to_string(),
                        })
                })
                .collect();
            CompareHeadFindings {
                detector,
                reuse: reuse.to_string(),
                reason,
                coverage,
                findings,
            }
        }
        Err(error) => CompareHeadFindings {
            detector,
            reuse: "not_evaluated".to_string(),
            reason: format!("the detector could not run: {error}"),
            coverage,
            findings: Vec::new(),
        },
    }
}

/// What the revision facts and prior read-sets support, as the wire states it.
struct Semantics {
    impact_status: String,
    impact_reason: String,
    work_reuse_status: String,
    work_reuse_reason: String,
    revision_facts: Option<CompareRevisionFacts>,
    impact: Option<CompareImpact>,
    work_reuse: Option<CompareWorkReuse>,
    head_findings: Option<CompareHeadFindings>,
}

impl Semantics {
    fn not_evaluated(reason: String) -> Self {
        Self {
            impact_status: "not_evaluated".to_string(),
            impact_reason: reason.clone(),
            work_reuse_status: "not_evaluated".to_string(),
            work_reuse_reason: reason,
            revision_facts: None,
            impact: None,
            work_reuse: None,
            head_findings: None,
        }
    }
}

/// Assembles both revisions' facts over the SCM ports, persists their
/// snapshots when a cache is named, and asks the one planner what the
/// transition means. Whatever cannot be established stays `not_evaluated`.
fn semantics(
    scm: &GitScmAdapter,
    workspace: &Path,
    cache_root: Option<&Path>,
    revisions: (
        &crate::domain::scm::SourceRevision,
        &crate::domain::scm::SourceRevision,
    ),
    change_set: &ChangeSet,
) -> Semantics {
    let (base_revision, candidate_revision) = revisions;
    let snapshots = cache_root.map(|root| RevisionSnapshotStore::for_workspace(root, workspace));
    let objects = snapshots.as_ref().map(FactObjectStore::beside);
    let limits = FactsLimits::default();
    let produce = |revision| produce_revision_facts(scm, revision, objects.as_ref(), &limits);
    let (base, candidate) = match (produce(base_revision), produce(candidate_revision)) {
        (Ok(base), Ok(candidate)) => (base, candidate),
        (Err(error), _) | (_, Err(error)) => {
            return Semantics::not_evaluated(format!("{FACTS_UNAVAILABLE}: {error}"));
        }
    };
    let persisted = snapshots.as_ref().is_some_and(|store| {
        [&base, &candidate]
            .iter()
            .all(|produced| store.save(&produced.snapshot).is_ok())
    });
    let revision_facts = CompareRevisionFacts {
        producer: producer_versions().facts,
        base: side_coverage(&base),
        candidate: side_coverage(&candidate),
        persisted,
    };

    let impact = match assess_impact(&base, &candidate, change_set) {
        Ok(impact) => impact,
        Err(error) => {
            let mut answer =
                Semantics::not_evaluated(format!("the fact delta could not be computed: {error}"));
            answer.revision_facts = Some(revision_facts);
            return answer;
        }
    };
    // The base is the revision whose read-set a later comparison consumes;
    // writing it here is what turns `not_evaluated` into a real verdict. A
    // failed write only leaves the answer `not_evaluated`.
    let writer_note = cache_root.and_then(|root| {
        ensure_run_in_cache(root, workspace, &base, &mut InMemoryEvidenceStore::new())
            .err()
            .map(|error| error.to_string())
    });
    let index =
        cache_root.and_then(|root| DetectorReuseStore::for_workspace(root, workspace).load());
    let reuse = assess_reuse(&base, &candidate, change_set, index.as_ref());
    let (work_reuse_status, work_reuse_reason, work_reuse) = match reuse {
        Ok(reuse) => {
            let verdicts = (!reuse.works.is_empty()).then(|| CompareWorkReuse {
                works: reuse
                    .works
                    .iter()
                    .map(|work| CompareWorkVerdict {
                        work: work.work.clone(),
                        disposition: disposition_word(work.disposition).to_string(),
                    })
                    .collect(),
                affected: reuse.count(WorkDisposition::Affected),
                unaffected: reuse.count(WorkDisposition::Unaffected),
                unknown: reuse.count(WorkDisposition::Unknown),
            });
            let reason = match &writer_note {
                Some(note) => format!(
                    "{}; the read-set could not be written: {note}",
                    reuse.reason
                ),
                None => reuse.reason,
            };
            (reuse.status.as_str().to_string(), reason, verdicts)
        }
        Err(error) => (
            "not_evaluated".to_string(),
            format!("the reuse plan could not be computed: {error}"),
            None,
        ),
    };
    Semantics {
        impact_status: impact.status.as_str().to_string(),
        impact_reason: impact.reason,
        work_reuse_status,
        work_reuse_reason,
        revision_facts: Some(revision_facts),
        impact: Some(CompareImpact {
            facts_added: impact.facts_added,
            facts_removed: impact.facts_removed,
            unanalyzed_changed_paths: impact
                .unanalyzed_changed
                .iter()
                .map(|path| path.as_str().to_string())
                .collect(),
            gap_paths: impact
                .gap_paths
                .iter()
                .map(|path| path.as_str().to_string())
                .collect(),
        }),
        work_reuse,
        head_findings: Some(head_findings(
            workspace, cache_root, &base, &candidate, change_set,
        )),
    }
}

/// Compare two ancestor revisions without a cache: nothing is persisted and
/// no prior read-sets are available, so reuse is `not_evaluated`.
pub fn compare(workspace: &Path, base_ref: &str, head_ref: &str) -> Result<GoalCompare, String> {
    compare_with(workspace, None, base_ref, head_ref)
}

/// Compare two ancestor revisions: the SCM diff, plus the impact and reuse
/// the revision facts and stored read-sets support. A revision snapshot is
/// persisted under `cache_root` so another branch can reuse its files.
pub fn compare_with(
    workspace: &Path,
    cache_root: Option<&Path>,
    base_ref: &str,
    head_ref: &str,
) -> Result<GoalCompare, String> {
    let root = workspace
        .canonicalize()
        .map_err(|error| format!("cannot resolve repository root: {error}"))?;
    let project = root.to_string_lossy();
    let source = SourceRef::new("git", &project).map_err(|error| error.to_string())?;
    let scm = GitScmAdapter::new(root.clone());
    let base_revision = scm
        .resolve(&source, base_ref)
        .map_err(|error| format!("cannot resolve base revision: {error}"))?;
    let candidate_revision = scm
        .resolve(&source, head_ref)
        .map_err(|error| format!("cannot resolve candidate revision: {error}"))?;
    let change_set = scm
        .changes_between(&base_revision, &candidate_revision)
        .map_err(|error| format!("cannot compare revisions: {error}"))?;
    let base_inventory = scm
        .inventory(&base_revision)
        .map_err(|error| format!("cannot inventory base revision: {error}"))?;
    let candidate_inventory = scm
        .inventory(&candidate_revision)
        .map_err(|error| format!("cannot inventory candidate revision: {error}"))?;

    let changes = compared_changes(&change_set);
    let staged_changes = scm
        .staged_changes(&source)
        .map_err(|error| format!("cannot compare HEAD with the staged index: {error}"))?;
    let dirty_paths = scm
        .dirty_paths(&source)
        .map_err(|error| format!("cannot inspect working-tree status: {error}"))?
        .into_iter()
        .map(|path| path.as_str().to_string())
        .collect();
    let working_copy = WorkingCopyComparison {
        staged_changes: compared_changes(&staged_changes),
        dirty_paths,
    };
    let unclassified_changes = changes
        .iter()
        .filter(|change| change.kind == "unknown")
        .count();
    let base = ComparedRevision {
        requested_ref: base_ref.to_string(),
        revision: base_revision.handle().as_str().to_string(),
        content_digest: base_revision.content().as_hex().to_string(),
        tracked_files: base_inventory.entries().len(),
    };
    let candidate = ComparedRevision {
        requested_ref: head_ref.to_string(),
        revision: candidate_revision.handle().as_str().to_string(),
        content_digest: candidate_revision.content().as_hex().to_string(),
        tracked_files: candidate_inventory.entries().len(),
    };
    let semantic = semantics(
        &scm,
        &root,
        cache_root,
        (&base_revision, &candidate_revision),
        &change_set,
    );
    let summary = format!(
        "Git compared {} changed path(s) from {} to {}; semantic impact is {} and prior-work reuse is {}.",
        changes.len(),
        base.revision,
        candidate.revision,
        semantic.impact_status,
        semantic.work_reuse_status
    );

    Ok(GoalCompare {
        summary,
        schema_version: "cognicode.goal.compare/v1".to_string(),
        goal: "compare".to_string(),
        producer: "git".to_string(),
        coverage: CompareCoverage {
            base_tracked_files: base.tracked_files,
            candidate_tracked_files: candidate.tracked_files,
            changed_paths: changes.len(),
            unclassified_changes,
            analysis: semantic.impact_status.clone(),
        },
        base,
        candidate,
        changes,
        working_copy,
        impact_status: semantic.impact_status,
        impact_reason: semantic.impact_reason,
        work_reuse_status: semantic.work_reuse_status,
        work_reuse_reason: semantic.work_reuse_reason,
        revision_facts: semantic.revision_facts,
        impact: semantic.impact,
        work_reuse: semantic.work_reuse,
        head_findings: semantic.head_findings,
    })
}

/// Run the goal from a shared CLI/MCP context.
pub fn run(ctx: &GoalEnvironment, arguments: &Value) -> Result<Value, String> {
    let input: CompareInput = serde_json::from_value(arguments.clone())
        .map_err(|error| format!("the arguments of the goal `compare` are not valid: {error}"))?;
    let answer = compare_with(
        ctx.workspace,
        ctx.cache_root,
        &input.base_ref,
        &input.head_ref,
    )?;
    serde_json::to_value(answer)
        .map_err(|error| format!("the answer of the goal `compare` is not serialisable: {error}"))
}

/// Render the SCM diff for a human-readable CLI response.
pub fn render(answer: &Value) -> String {
    let compared: GoalCompare = serde_json::from_value(answer.clone())
        .expect("the answer of the goal `compare` is the answer its own type describes");
    let mut text = format!("{}\n", compared.summary);
    for change in compared.changes {
        text.push_str(&format!("  {} {}\n", change.kind, change.path));
    }
    if let Some(head) = &compared.head_findings {
        text.push_str(&format!(
            "Head findings of {}: {} ({}, {}): {}\n",
            head.detector,
            head.findings.len(),
            head.reuse,
            head.coverage,
            head.reason
        ));
    }
    if !compared.working_copy.staged_changes.is_empty() {
        text.push_str("Staged changes (HEAD → index):\n");
        for change in compared.working_copy.staged_changes {
            text.push_str(&format!("  {} {}\n", change.kind, change.path));
        }
    }
    if !compared.working_copy.dirty_paths.is_empty() {
        text.push_str("Dirty working-tree paths (content not classified):\n");
        for path in compared.working_copy.dirty_paths {
            text.push_str(&format!("  {path}\n"));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git(root: &Path, args: &[&str]) {
        let no_configuration = root.join("no-git-configuration");
        let output = Command::new("git")
            .env("PATH", "/usr/bin:/bin")
            .arg("-C")
            .arg(root)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", &no_configuration)
            .env("GIT_CONFIG_SYSTEM", &no_configuration)
            .output()
            .expect("git is installed");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn compare_reports_scm_changes_with_the_fact_impact() {
        let directory = tempfile::tempdir().expect("temporary repository");
        git(directory.path(), &["init", "--quiet"]);
        git(
            directory.path(),
            &["config", "user.name", "CogniCode tests"],
        );
        git(
            directory.path(),
            &["config", "user.email", "tests@example.invalid"],
        );
        std::fs::write(directory.path().join("src.rs"), "fn first() {}\n")
            .expect("write base source");
        git(directory.path(), &["add", "src.rs"]);
        git(directory.path(), &["commit", "--quiet", "-m", "base"]);
        let base = String::from_utf8(
            Command::new("git")
                .arg("-C")
                .arg(directory.path())
                .args(["rev-parse", "HEAD"])
                .output()
                .expect("read base revision")
                .stdout,
        )
        .expect("revision is UTF-8");
        std::fs::write(directory.path().join("src.rs"), "fn second() {}\n")
            .expect("write candidate source");
        git(directory.path(), &["add", "src.rs"]);
        git(directory.path(), &["commit", "--quiet", "-m", "candidate"]);
        std::fs::write(directory.path().join("src.rs"), "fn staged() {}\n")
            .expect("write staged content");
        git(directory.path(), &["add", "src.rs"]);
        std::fs::write(directory.path().join("loose.rs"), "fn loose() {}\n")
            .expect("write worktree content");
        std::fs::write(directory.path().join("untracked.rs"), "fn untracked() {}\n")
            .expect("write untracked content");

        let answer = compare(directory.path(), base.trim(), "HEAD").expect("compare revisions");

        assert_eq!(answer.producer, "git");
        let wire = serde_json::to_value(&answer).expect("serialize comparison");
        assert_eq!(wire["working_copy"]["staged_changes"][0]["path"], "src.rs");
        let dirty_paths = wire["working_copy"]["dirty_paths"]
            .as_array()
            .expect("dirty paths are published");
        assert!(dirty_paths.iter().any(|path| path == "loose.rs"));
        assert!(dirty_paths.iter().any(|path| path == "untracked.rs"));
        assert_eq!(answer.coverage.analysis, "evaluated");
        assert_eq!(answer.coverage.changed_paths, 1);
        assert_eq!(answer.changes[0].path, "src.rs");
        assert_eq!(answer.changes[0].kind, "modified");
        assert_eq!(answer.impact_status, "evaluated");
        let impact = answer.impact.as_ref().expect("a real fact delta");
        assert!(impact.facts_added > 0 && impact.facts_removed > 0);
        assert_eq!(answer.work_reuse_status, "not_evaluated");
        assert!(answer.work_reuse_reason.contains("read-set"));
        assert!(answer.work_reuse.is_none());
        assert_ne!(answer.base.content_digest, answer.candidate.content_digest);
    }

    fn commit_file(root: &Path, name: &str, body: &str, message: &str) -> String {
        std::fs::write(root.join(name), body).expect("write file");
        git(root, &["add", "-A"]);
        git(root, &["commit", "--quiet", "-m", message]);
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("rev-parse");
        String::from_utf8(output.stdout)
            .expect("utf8")
            .trim()
            .to_string()
    }

    fn repository() -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("temporary repository");
        git(directory.path(), &["init", "--quiet"]);
        git(
            directory.path(),
            &["config", "user.name", "CogniCode tests"],
        );
        git(
            directory.path(),
            &["config", "user.email", "tests@example.invalid"],
        );
        directory
    }

    #[test]
    fn a_body_edit_leaves_the_detector_unaffected_so_its_work_is_reusable() {
        let directory = repository();
        let cache = tempfile::tempdir().expect("cache");
        let root = directory.path();
        let base = commit_file(root, "a.rs", "pub fn alpha() { first(); }\n", "base");
        commit_file(root, "a.rs", "pub fn alpha() { second(); }\n", "head");

        let answer = compare_with(root, Some(cache.path()), &base, "HEAD").expect("compare");

        assert_eq!(
            answer.work_reuse_status, "evaluated",
            "{}",
            answer.work_reuse_reason
        );
        let reuse = answer.work_reuse.expect("a verdict per detector");
        assert_eq!((reuse.affected, reuse.unaffected, reuse.unknown), (0, 1, 0));
        assert_eq!(reuse.works[0].disposition, "unaffected");
    }

    #[test]
    fn a_new_definition_cannot_be_proven_unread_so_the_detector_must_run_again() {
        let directory = repository();
        let cache = tempfile::tempdir().expect("cache");
        let root = directory.path();
        let base = commit_file(root, "a.rs", "pub fn alpha() {}\n", "base");
        commit_file(root, "b.rs", "pub fn beta() {}\n", "head");

        let answer = compare_with(root, Some(cache.path()), &base, "HEAD").expect("compare");

        assert_eq!(answer.work_reuse_status, "partial");
        let reuse = answer.work_reuse.expect("a verdict per detector");
        assert_eq!(
            (reuse.unaffected, reuse.unknown),
            (0, 1),
            "an aggregate read over all functions cannot prove the new one unread"
        );
    }

    #[test]
    fn a_body_edit_reuses_the_head_findings_and_names_the_base_as_their_producer() {
        let directory = repository();
        let cache = tempfile::tempdir().expect("cache");
        let root = directory.path();
        let base = commit_file(root, "a.rs", "pub fn alpha() { first(); }\n", "base");
        commit_file(root, "a.rs", "pub fn alpha() { second(); }\n", "head");

        let answer = compare_with(root, Some(cache.path()), &base, "HEAD").expect("compare");

        let head = answer.head_findings.expect("head findings are published");
        assert_eq!(head.reuse, "reused", "{}", head.reason);
        assert_eq!(head.coverage, "full");
        assert_eq!(head.findings.len(), 1);
        assert_eq!(head.findings[0].file, "a.rs");
        assert_eq!(head.findings[0].line, 1);
        assert_eq!(head.findings[0].evidence_revision, answer.base.revision);
    }

    #[test]
    fn a_renamed_definition_reruns_the_detector_on_the_head_and_says_why() {
        let directory = repository();
        let cache = tempfile::tempdir().expect("cache");
        let root = directory.path();
        let base = commit_file(root, "a.rs", "pub fn alpha() {}\n", "base");
        commit_file(root, "a.rs", "pub fn alpha_two() {}\n", "head");

        let answer = compare_with(root, Some(cache.path()), &base, "HEAD").expect("compare");

        let head = answer.head_findings.expect("head findings are published");
        assert_eq!(head.reuse, "rerun");
        assert!(
            head.reason.contains("touches a fact the detector read"),
            "{}",
            head.reason
        );
        assert_eq!(head.findings.len(), 1);
        assert_eq!(
            head.findings[0].evidence_revision,
            answer.candidate.revision
        );
    }

    #[test]
    fn a_new_definition_is_found_on_the_head_and_never_reused() {
        let directory = repository();
        let cache = tempfile::tempdir().expect("cache");
        let root = directory.path();
        let base = commit_file(root, "a.rs", "pub fn alpha() {}\n", "base");
        commit_file(root, "b.rs", "pub fn beta() {}\n", "head");

        let answer = compare_with(root, Some(cache.path()), &base, "HEAD").expect("compare");

        let head = answer.head_findings.expect("head findings are published");
        assert_eq!(head.reuse, "rerun", "{}", head.reason);
        assert!(head.reason.contains("not proven"), "{}", head.reason);
        let files: Vec<&str> = head.findings.iter().map(|f| f.file.as_str()).collect();
        assert_eq!(files, ["a.rs", "b.rs"]);
    }

    #[test]
    fn without_a_cache_the_head_findings_are_a_rerun_because_no_read_set_exists() {
        let directory = repository();
        let root = directory.path();
        let base = commit_file(root, "a.rs", "pub fn alpha() { first(); }\n", "base");
        commit_file(root, "a.rs", "pub fn alpha() { second(); }\n", "head");

        let answer = compare(root, &base, "HEAD").expect("compare");

        let head = answer.head_findings.expect("head findings are published");
        assert_eq!(head.reuse, "rerun");
        assert!(head.reason.contains("no prior read-set"), "{}", head.reason);
        assert_eq!(head.findings.len(), 1);
    }

    #[test]
    fn a_partial_head_never_reuses_and_reports_its_findings_as_a_lower_bound() {
        let directory = repository();
        let cache = tempfile::tempdir().expect("cache");
        let root = directory.path();
        let base = commit_file(root, "a.rs", "pub fn alpha() { first(); }\n", "base");
        std::fs::write(root.join("blob.rs"), [0xff, 0xfe, 0x00, 0x9f]).expect("binary source");
        commit_file(root, "a.rs", "pub fn alpha() { second(); }\n", "head");

        let answer = compare_with(root, Some(cache.path()), &base, "HEAD").expect("compare");

        let head = answer.head_findings.expect("head findings are published");
        assert_eq!(head.coverage, "partial");
        assert_eq!(head.reuse, "rerun", "{}", head.reason);
    }

    #[test]
    fn the_text_rendering_states_whether_the_head_findings_were_reused() {
        let directory = repository();
        let cache = tempfile::tempdir().expect("cache");
        let root = directory.path();
        let base = commit_file(root, "a.rs", "pub fn alpha() { first(); }\n", "base");
        commit_file(root, "a.rs", "pub fn alpha() { second(); }\n", "head");
        let answer = compare_with(root, Some(cache.path()), &base, "HEAD").expect("compare");

        let text = render(&serde_json::to_value(answer).expect("serialise"));

        assert!(
            text.contains("Head findings of") && text.contains("1 (reused, full)"),
            "{text}"
        );
    }

    #[test]
    fn without_a_cache_there_is_no_read_set_and_the_answer_says_so() {
        let directory = repository();
        let root = directory.path();
        let base = commit_file(root, "a.rs", "pub fn alpha() {}\n", "base");
        commit_file(root, "a.rs", "pub fn alpha() { x(); }\n", "head");

        let answer = compare(root, &base, "HEAD").expect("compare");

        assert_eq!(answer.work_reuse_status, "not_evaluated");
        assert!(answer.work_reuse_reason.contains("read-set"));
        assert!(answer.work_reuse.is_none());
    }

    #[test]
    fn unreadable_files_make_the_impact_partial_never_evaluated() {
        let directory = repository();
        let root = directory.path();
        let base = commit_file(root, "src.rs", "fn first() {}\n", "base");
        std::fs::write(root.join("blob.rs"), [0xff, 0xfe, 0x00, 0x9f]).expect("binary source");
        commit_file(root, "src.rs", "fn second() {}\n", "head");

        let answer = compare(root, &base, "HEAD").expect("compare");

        assert_eq!(answer.impact_status, "partial", "{}", answer.impact_reason);
        assert_eq!(answer.coverage.analysis, "partial");
        let facts = answer.revision_facts.expect("facts are reported");
        assert_eq!(facts.candidate.coverage, "partial");
        assert_eq!(facts.candidate.gaps[0].path, "blob.rs");
        assert!(!facts.persisted, "no cache named, nothing persisted");
    }

    #[test]
    fn snapshots_persist_and_the_next_comparison_reuses_the_files() {
        let directory = repository();
        let cache = tempfile::tempdir().expect("cache");
        let root = directory.path();
        let base = commit_file(root, "a.rs", "fn a() {}\n", "base");
        commit_file(root, "b.rs", "fn b() {}\n", "head");

        let first = compare_with(root, Some(cache.path()), &base, "HEAD").expect("first");
        let facts = first.revision_facts.expect("facts");
        assert!(facts.persisted);
        assert_eq!(facts.base.reused_files, 0);

        let second = compare_with(root, Some(cache.path()), &base, "HEAD").expect("second");
        let facts = second.revision_facts.expect("facts");
        assert_eq!(facts.base.parsed_files, 0, "every file came from objects");
        assert_eq!(facts.candidate.parsed_files, 0);
        assert_eq!(facts.candidate.reused_files, 2);
        assert_eq!(
            first.impact.unwrap().facts_added,
            second.impact.unwrap().facts_added
        );
    }

    #[test]
    fn the_same_revision_on_both_sides_is_not_evaluated_with_a_reason() {
        let directory = repository();
        let root = directory.path();
        commit_file(root, "a.rs", "fn a() {}\n", "base");

        let answer = compare(root, "HEAD", "HEAD").expect("compare");

        assert_eq!(answer.impact_status, "not_evaluated");
        assert!(
            answer.impact_reason.contains("delta"),
            "{}",
            answer.impact_reason
        );
        assert!(answer.impact.is_none());
    }
}
