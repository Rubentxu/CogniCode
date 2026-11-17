//! Read-only comparison of two ancestor Git revisions.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::domain::scm::{
    ChangeKind, ChangeReader, ChangeSet, RevisionReader, SourceRef, WorkingCopyReader,
};
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
    /// Semantic analysis is deliberately not claimed by this goal.
    pub analysis: String,
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
    /// Semantic impact and prior-result reuse are not inferred from an SCM diff.
    pub impact_status: String,
    /// Why semantic impact and reuse were not evaluated.
    pub impact_reason: String,
    /// Whether previous work was reused; this slice never claims reuse.
    pub work_reuse_status: String,
    /// Why previous work reuse was not evaluated.
    pub work_reuse_reason: String,
}

const NOT_EVALUATED_REASON: &str = "No production source currently assembles complete revision facts and prior execution read-sets for this request; an SCM diff alone cannot establish impact or reuse.";

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

/// Compare two ancestor revisions without claiming semantic impact or reused work.
pub fn compare(workspace: &Path, base_ref: &str, head_ref: &str) -> Result<GoalCompare, String> {
    let root = workspace
        .canonicalize()
        .map_err(|error| format!("cannot resolve repository root: {error}"))?;
    let project = root.to_string_lossy();
    let source = SourceRef::new("git", &project).map_err(|error| error.to_string())?;
    let scm = GitScmAdapter::new(root);
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
    let summary = format!(
        "Git compared {} changed path(s) from {} to {}; semantic impact and prior-work reuse were not evaluated.",
        changes.len(),
        base.revision,
        candidate.revision
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
            analysis: "not_evaluated".to_string(),
        },
        base,
        candidate,
        changes,
        working_copy,
        impact_status: "not_evaluated".to_string(),
        impact_reason: NOT_EVALUATED_REASON.to_string(),
        work_reuse_status: "not_evaluated".to_string(),
        work_reuse_reason: NOT_EVALUATED_REASON.to_string(),
    })
}

/// Run the goal from a shared CLI/MCP context.
pub fn run(ctx: &GoalEnvironment, arguments: &Value) -> Result<Value, String> {
    let input: CompareInput = serde_json::from_value(arguments.clone())
        .map_err(|error| format!("the arguments of the goal `compare` are not valid: {error}"))?;
    let answer = compare(ctx.workspace, &input.base_ref, &input.head_ref)?;
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
    fn compare_reports_scm_changes_without_claiming_semantic_impact() {
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
        assert_eq!(answer.coverage.analysis, "not_evaluated");
        assert_eq!(answer.coverage.changed_paths, 1);
        assert_eq!(answer.changes[0].path, "src.rs");
        assert_eq!(answer.changes[0].kind, "modified");
        assert_eq!(answer.impact_status, "not_evaluated");
        assert!(answer.impact_reason.contains("read-sets"));
        assert_ne!(answer.base.content_digest, answer.candidate.content_digest);
    }
}
