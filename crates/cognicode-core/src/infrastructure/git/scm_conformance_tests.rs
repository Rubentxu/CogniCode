//! The SCM conformance suite run over the Git adapter, against a real
//! throwaway repository built from the suite's own revision graph. The
//! same suite runs over the in-memory provider, so the two meet one
//! contract for inventory, reading, diff, ancestry and errors. Nothing
//! here touches the network, the user's Git configuration or a remote.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

use super::GitScmAdapter;
use crate::domain::scm::SourceRef;
use crate::domain::scm::conformance::{
    GraphRevision, ProvisionedSource, RevisionGraph, ScmHarness, WORKING_COPY_BASE,
    WorkingCopyDriver, run_scm_conformance,
};

/// Runs git in `repo` with no user configuration, no hooks and a
/// pinned `PATH`.
fn git(repo: &Path, args: &[&str]) -> String {
    let no_configuration = repo.join("no-git-configuration");
    let output = Command::new("git")
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_GLOBAL", &no_configuration)
        .env("GIT_CONFIG_SYSTEM", &no_configuration)
        .args(args)
        .current_dir(repo)
        .output()
        .expect("git is available for adapter tests");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("git prints UTF-8")
}

fn write_tracked(repo: &Path, relative: &str, bytes: &[u8]) {
    let target = repo.join(relative);
    fs::create_dir_all(target.parent().expect("a file has a parent")).expect("create directory");
    fs::write(target, bytes).expect("write file");
}

/// Replaces the working tree and index with exactly `revision`'s files.
fn commit_revision(repo: &Path, revision: &GraphRevision) {
    match revision.parents.as_slice() {
        [] => {
            git(repo, &["switch", "-q", "--orphan", revision.name]);
        }
        [parent] => {
            git(repo, &["checkout", "-q", "-B", revision.name, parent]);
        }
        _ => panic!("the fixture graph is linear per branch"),
    }
    git(
        repo,
        &["rm", "-r", "-q", "-f", "--ignore-unmatch", "--", "."],
    );
    for (path, bytes) in &revision.files {
        write_tracked(repo, path, bytes);
    }
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", revision.name]);
}

struct GitDriver {
    repo: PathBuf,
}

impl WorkingCopyDriver for GitDriver {
    fn stage(&self, path: &str, bytes: &[u8]) {
        write_tracked(&self.repo, path, bytes);
        git(&self.repo, &["add", "--", path]);
    }

    fn edit(&self, path: &str, bytes: &[u8]) {
        write_tracked(&self.repo, path, bytes);
    }
}

struct GitHarness;

impl ScmHarness for GitHarness {
    fn provision(&self, graph: &RevisionGraph) -> ProvisionedSource {
        let directory: TempDir = tempfile::tempdir().expect("temporary repository");
        let repo = directory.path();
        git(repo, &["init", "-q", "-b", "scratch"]);
        git(repo, &["config", "user.name", "Conformance"]);
        git(
            repo,
            &["config", "user.email", "conformance@example.invalid"],
        );
        git(repo, &["config", "commit.gpgsign", "false"]);
        for revision in graph.revisions() {
            commit_revision(repo, revision);
        }
        git(repo, &["checkout", "-q", WORKING_COPY_BASE]);

        let references = graph
            .revisions()
            .iter()
            .map(|revision| (revision.name, revision.name.to_string()))
            .collect();
        ProvisionedSource {
            provider: Box::new(GitScmAdapter::new(repo)),
            source: SourceRef::new("git", "conformance").expect("a fixed source"),
            references,
            working_copy: Some(Box::new(GitDriver {
                repo: repo.to_path_buf(),
            })),
            keep_alive: Some(Box::new(directory)),
        }
    }
}

#[test]
fn the_git_adapter_meets_the_same_contract_as_the_in_memory_provider() {
    run_scm_conformance(&GitHarness).assert_conforms("GitScmAdapter");
}
