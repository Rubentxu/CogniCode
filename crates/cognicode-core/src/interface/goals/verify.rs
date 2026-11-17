//! The goal `verify`: does the stored snapshot of a revision still equal a
//! full rebuild of that revision?
//!
//! The goal rebuilds the facts of the revision from its bytes, without
//! consulting any stored object, and compares the result with the snapshot
//! persisted for it. It never writes. Whatever it cannot establish is said
//! as such: no cache, no stored snapshot, or facts missing for some file
//! never read as `verified`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;

use crate::application::change_tracking::revision_facts::producer_versions;
use crate::interface::goals::GoalEnvironment;

use super::compare::RevisionFactsCoverage;
use super::revision_support::{produce_for_ref, snapshot_store};

/// Arguments for the `verify` goal.
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct VerifyInput {
    /// The Git revision whose stored snapshot is verified.
    pub revision: String,
}

/// What was found when the stored snapshot was compared with the rebuild.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifySnapshot {
    /// Whether a snapshot of this revision, from this producer, is stored.
    pub found: bool,
    /// Files the stored snapshot holds objects for.
    pub files_in_snapshot: usize,
    /// Files the rebuild produced facts for.
    pub files_rebuilt: usize,
    /// Files present on both sides whose object differs.
    pub mismatched_paths: Vec<String>,
    /// Rebuilt files the stored snapshot lacks.
    pub missing_in_snapshot: Vec<String>,
    /// Stored files the rebuild does not have.
    pub extra_in_snapshot: Vec<String>,
}

/// The answer of the goal `verify`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalVerify {
    /// One line that states the result.
    pub summary: String,
    /// Version of this response contract.
    pub schema_version: String,
    /// The goal that answers.
    pub goal: String,
    /// The reference the caller supplied.
    pub requested_ref: String,
    /// The resolved immutable revision handle.
    pub revision: String,
    /// The digest of all tracked content of the revision.
    pub content_digest: String,
    /// `verified`, `mismatch`, `partial` or `not_evaluated`.
    pub status: String,
    /// Why the status is what it is.
    pub reason: String,
    /// What the rebuilt facts cover, gaps included.
    pub coverage: RevisionFactsCoverage,
    /// The comparison with the stored snapshot.
    pub snapshot: VerifySnapshot,
}

/// Verifies the stored snapshot of `reference` against a full rebuild.
pub fn verify(
    workspace: &Path,
    cache_root: Option<&Path>,
    reference: &str,
) -> Result<GoalVerify, String> {
    // A full rebuild: no stored object is consulted and nothing is written.
    let rebuilt = produce_for_ref(workspace, cache_root, reference, false)?;
    let rebuilt_files = &rebuilt.produced.snapshot.files;
    let canonical = workspace
        .canonicalize()
        .map_err(|error| format!("cannot resolve repository root: {error}"))?;
    let stored = snapshot_store(&canonical, cache_root)
        .and_then(|store| store.load(rebuilt.revision.handle(), &producer_versions()));

    let mut snapshot = VerifySnapshot {
        found: stored.is_some(),
        files_in_snapshot: 0,
        files_rebuilt: rebuilt_files.len(),
        mismatched_paths: Vec::new(),
        missing_in_snapshot: Vec::new(),
        extra_in_snapshot: Vec::new(),
    };
    let (status, reason) = match (&stored, cache_root) {
        (_, None) => (
            "not_evaluated",
            "the environment names no cache, so there is no stored snapshot to verify".to_string(),
        ),
        (None, Some(_)) => (
            "not_evaluated",
            "no snapshot of this revision from this producer is stored; run `compare` or \
             `inspect --revision` to produce one"
                .to_string(),
        ),
        (Some(stored), Some(_)) => {
            snapshot.files_in_snapshot = stored.files.len();
            let paths: BTreeSet<_> = rebuilt_files.keys().chain(stored.files.keys()).collect();
            for path in paths {
                match (rebuilt_files.get(path), stored.files.get(path)) {
                    (Some(a), Some(b)) if a != b => {
                        snapshot.mismatched_paths.push(path.as_str().to_string());
                    }
                    (Some(_), None) => snapshot.missing_in_snapshot.push(path.as_str().to_string()),
                    (None, Some(_)) => snapshot.extra_in_snapshot.push(path.as_str().to_string()),
                    _ => {}
                }
            }
            let identical = snapshot.mismatched_paths.is_empty()
                && snapshot.missing_in_snapshot.is_empty()
                && snapshot.extra_in_snapshot.is_empty()
                && stored.source_content == *rebuilt.revision.content();
            if !identical {
                (
                    "mismatch",
                    "the stored snapshot differs from a full rebuild of the revision".to_string(),
                )
            } else if !rebuilt.produced.coverage.is_complete() {
                (
                    "partial",
                    format!(
                        "the stored snapshot equals the rebuild, but facts are missing for {} \
                         analyzable file(s): completeness is not vouched for",
                        rebuilt.produced.coverage.gaps.len()
                    ),
                )
            } else {
                (
                    "verified",
                    "the stored snapshot equals a full rebuild and covers every analyzable file"
                        .to_string(),
                )
            }
        }
    };
    let summary = format!(
        "Revision {} is {status}: {reason}.",
        rebuilt.revision.handle().as_str()
    );
    Ok(GoalVerify {
        summary,
        schema_version: "cognicode.goal.verify/v1".to_string(),
        goal: "verify".to_string(),
        requested_ref: reference.to_string(),
        revision: rebuilt.revision.handle().as_str().to_string(),
        content_digest: rebuilt.revision.content().as_hex().to_string(),
        status: status.to_string(),
        reason,
        coverage: rebuilt.coverage(),
        snapshot,
    })
}

/// Runs the goal from a shared CLI/MCP context.
pub fn run(ctx: &GoalEnvironment, arguments: &Value) -> Result<Value, String> {
    let input: VerifyInput = serde_json::from_value(arguments.clone())
        .map_err(|error| format!("the arguments of the goal `verify` are not valid: {error}"))?;
    let answer = verify(ctx.workspace, ctx.cache_root, &input.revision)?;
    serde_json::to_value(answer)
        .map_err(|error| format!("the answer of the goal `verify` is not serialisable: {error}"))
}

/// The answer as text for a terminal.
pub fn render(answer: &Value) -> String {
    let verified: GoalVerify = serde_json::from_value(answer.clone())
        .expect("the answer of the goal `verify` is the answer its own type describes");
    let mut text = format!("{}\n", verified.summary);
    for (label, paths) in [
        ("mismatched", &verified.snapshot.mismatched_paths),
        (
            "missing in snapshot",
            &verified.snapshot.missing_in_snapshot,
        ),
        ("extra in snapshot", &verified.snapshot.extra_in_snapshot),
    ] {
        for path in paths {
            text.push_str(&format!("  {label}: {path}\n"));
        }
    }
    for gap in &verified.coverage.gaps {
        text.push_str(&format!("  no facts: {} ({})\n", gap.path, gap.reason));
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

    fn repository(files: &[(&str, &[u8])]) -> tempfile::TempDir {
        let directory = tempfile::tempdir().expect("repository");
        let root = directory.path();
        git(root, &["init", "--quiet"]);
        git(root, &["config", "user.name", "CogniCode tests"]);
        git(root, &["config", "user.email", "tests@example.invalid"]);
        for (name, body) in files {
            std::fs::write(root.join(name), body).expect("write file");
        }
        git(root, &["add", "-A"]);
        git(root, &["commit", "--quiet", "-m", "base"]);
        directory
    }

    #[test]
    fn a_stored_snapshot_that_equals_the_rebuild_is_verified() {
        let repo = repository(&[("a.rs", b"fn a() {}\n"), ("b.rs", b"fn b() {}\n")]);
        let cache = tempfile::tempdir().expect("cache");
        produce_for_ref(repo.path(), Some(cache.path()), "HEAD", true).expect("persist");

        let answer = verify(repo.path(), Some(cache.path()), "HEAD").expect("verify");

        assert_eq!(answer.status, "verified", "{}", answer.reason);
        assert!(answer.snapshot.found);
        assert_eq!(answer.snapshot.files_in_snapshot, 2);
    }

    #[test]
    fn a_missing_snapshot_or_cache_is_not_evaluated_never_verified() {
        let repo = repository(&[("a.rs", b"fn a() {}\n")]);
        let cache = tempfile::tempdir().expect("cache");

        let no_cache = verify(repo.path(), None, "HEAD").expect("verify");
        let nothing_stored = verify(repo.path(), Some(cache.path()), "HEAD").expect("verify");

        assert_eq!(no_cache.status, "not_evaluated");
        assert!(no_cache.reason.contains("cache"));
        assert_eq!(nothing_stored.status, "not_evaluated");
        assert!(!nothing_stored.snapshot.found);
    }

    #[test]
    fn incomplete_facts_are_partial_never_verified() {
        let repo = repository(&[("a.rs", b"fn a() {}\n"), ("bad.rs", &[0xff, 0xfe, 0x9f])]);
        let cache = tempfile::tempdir().expect("cache");
        produce_for_ref(repo.path(), Some(cache.path()), "HEAD", true).expect("persist");

        let answer = verify(repo.path(), Some(cache.path()), "HEAD").expect("verify");

        assert_eq!(answer.status, "partial", "{}", answer.reason);
        assert_eq!(answer.coverage.gaps[0].path, "bad.rs");
    }

    #[test]
    fn a_tampered_snapshot_is_a_mismatch() {
        use crate::application::revision_snapshots::ExtractionObject;
        let repo = repository(&[("a.rs", b"fn a() {}\n")]);
        let cache = tempfile::tempdir().expect("cache");
        produce_for_ref(repo.path(), Some(cache.path()), "HEAD", true).expect("persist");
        let canonical = repo.path().canonicalize().expect("canonical");
        let store = snapshot_store(&canonical, Some(cache.path())).expect("store");
        let answer = verify(repo.path(), Some(cache.path()), "HEAD").expect("verify");
        let mut stored = store
            .load(
                &crate::domain::scm::RevisionHandle::new(&answer.revision).expect("handle"),
                &producer_versions(),
            )
            .expect("stored snapshot");
        let key = stored.files.keys().next().expect("a file").clone();
        let ExtractionObject { content, .. } = stored.files[&key].clone();
        stored.files.insert(
            key,
            ExtractionObject {
                content,
                artifact: "forged".to_string(),
            },
        );
        store.save(&stored).expect("save forged snapshot");

        let answer = verify(repo.path(), Some(cache.path()), "HEAD").expect("verify");

        assert_eq!(answer.status, "mismatch");
        assert_eq!(answer.snapshot.mismatched_paths, vec!["a.rs".to_string()]);
    }

    #[test]
    fn an_unknown_revision_is_an_error_not_a_verdict() {
        let repo = repository(&[("a.rs", b"fn a() {}\n")]);

        let answer = verify(repo.path(), None, "no-such-ref");

        assert!(answer.is_err());
    }
}
