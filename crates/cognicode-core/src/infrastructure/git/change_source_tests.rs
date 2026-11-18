//! The change source over real throwaway repositories: each scope reads its
//! own bytes and never another scope's.

use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

use super::GitChangeSource;
use crate::application::context_pack::change::input::{
    ChangeInput, ChangeScope, ChangeSource, Content, FileChange,
};

fn git(repo: &Path, args: &[&str]) -> String {
    let none = repo.join("no-git-configuration");
    let output = Command::new("git")
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_GLOBAL", &none)
        .env("GIT_CONFIG_SYSTEM", &none)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(["-c", "commit.gpgsign=false"])
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

fn write(repo: &Path, relative: &str, text: &str) {
    let target = repo.join(relative);
    fs::create_dir_all(target.parent().expect("a file has a parent")).expect("directory");
    fs::write(target, text).expect("file");
}

fn repo_with(files: &[(&str, &str)]) -> TempDir {
    let directory = tempfile::tempdir().expect("temporary repository");
    let repo = directory.path();
    git(repo, &["init", "-q", "-b", "main"]);
    for (path, text) in files {
        write(repo, path, text);
    }
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "base"]);
    directory
}

fn read(repo: &Path, scope: ChangeScope) -> Result<ChangeInput, String> {
    GitChangeSource::new(repo).expect("source").read(&scope)
}

fn paths(input: &ChangeInput) -> Vec<&str> {
    input.files.iter().map(|file| file.path.as_str()).collect()
}

fn bytes(text: &str) -> Content {
    Content::Bytes(text.as_bytes().to_vec())
}

#[test]
fn staged_reads_the_index_and_not_the_working_tree() {
    let directory = repo_with(&[("a.rs", "one\n"), ("b.rs", "one\n")]);
    let repo = directory.path();
    write(repo, "a.rs", "staged\n");
    git(repo, &["add", "a.rs"]);
    write(repo, "a.rs", "staged\nunstaged\n");
    write(repo, "b.rs", "edited\n");

    let staged = read(repo, ChangeScope::Staged).expect("staged");
    assert_eq!(paths(&staged), ["a.rs"]);
    assert_eq!(staged.head, "index");
    assert_eq!(staged.files[0].change, FileChange::Modified);
    assert_eq!(staged.files[0].before, bytes("one\n"));
    assert_eq!(staged.files[0].after, bytes("staged\n"));
}

#[test]
fn the_working_tree_reads_staged_unstaged_and_untracked_from_disk() {
    let directory = repo_with(&[("a.rs", "one\n"), ("b.rs", "one\n"), ("gone.rs", "x\n")]);
    let repo = directory.path();
    write(repo, "a.rs", "staged\n");
    git(repo, &["add", "a.rs"]);
    write(repo, "a.rs", "staged\nunstaged\n");
    write(repo, "b.rs", "edited\n");
    write(repo, "new.rs", "fresh\n");
    fs::remove_file(repo.join("gone.rs")).expect("delete");

    let tree = read(repo, ChangeScope::WorkingTree).expect("working tree");
    assert_eq!(paths(&tree), ["a.rs", "b.rs", "gone.rs", "new.rs"]);
    assert_eq!(tree.head, "working_tree");
    assert_eq!(tree.files[0].after, bytes("staged\nunstaged\n"));
    assert_eq!(tree.files[1].before, bytes("one\n"));
    assert_eq!(tree.files[2].change, FileChange::Deleted);
    assert_eq!(tree.files[3].change, FileChange::Added);
    assert_eq!(tree.files[3].before, Content::Absent);
}

#[test]
fn a_staged_rename_is_one_change_in_the_working_tree() {
    let directory = repo_with(&[("old.rs", "same\n"), ("keep.rs", "k\n")]);
    let repo = directory.path();
    git(repo, &["mv", "old.rs", "new.rs"]);

    let tree = read(repo, ChangeScope::WorkingTree).expect("working tree");
    assert_eq!(paths(&tree), ["new.rs"]);
    assert_eq!(
        tree.files[0].change,
        FileChange::Renamed {
            from: "old.rs".to_string()
        }
    );
    assert_eq!(tree.files[0].before, bytes("same\n"));
}

#[test]
fn a_commit_does_not_inherit_what_is_on_disk() {
    let directory = repo_with(&[("a.rs", "one\n"), ("b.rs", "one\n")]);
    let repo = directory.path();
    write(repo, "a.rs", "two\n");
    git(repo, &["commit", "-q", "-am", "change a"]);
    write(repo, "b.rs", "dirty\n");
    write(repo, "untracked.rs", "u\n");

    let commit = read(
        repo,
        ChangeScope::Commit {
            commit: "HEAD".to_string(),
        },
    )
    .expect("commit");
    assert_eq!(paths(&commit), ["a.rs"]);
    assert_eq!(commit.files[0].before, bytes("one\n"));
    assert_eq!(commit.files[0].after, bytes("two\n"));
    let earlier = read(
        repo,
        ChangeScope::Commit {
            commit: "HEAD~1".to_string(),
        },
    );
    assert!(
        earlier.is_err(),
        "a root commit has no first parent: {earlier:?}"
    );
}

#[test]
fn a_range_reads_the_changes_since_the_merge_base() {
    let directory = repo_with(&[("a.rs", "one\n"), ("b.rs", "one\n")]);
    let repo = directory.path();
    git(repo, &["checkout", "-q", "-b", "topic"]);
    write(repo, "a.rs", "topic\n");
    git(repo, &["commit", "-q", "-am", "topic"]);
    git(repo, &["checkout", "-q", "main"]);
    write(repo, "b.rs", "main\n");
    git(repo, &["commit", "-q", "-am", "main moves on"]);

    let range = read(
        repo,
        ChangeScope::Range {
            base: "main".to_string(),
            head: "topic".to_string(),
        },
    )
    .expect("range");
    assert_eq!(paths(&range), ["a.rs"]);
    assert_eq!(range.files[0].after, bytes("topic\n"));
}

#[test]
fn a_merge_candidate_names_its_conflicts_as_unknown() {
    let directory = repo_with(&[("a.rs", "one\n"), ("b.rs", "one\n"), ("c.rs", "one\n")]);
    let repo = directory.path();
    git(repo, &["checkout", "-q", "-b", "topic"]);
    write(repo, "a.rs", "topic\n");
    write(repo, "c.rs", "topic only\n");
    git(repo, &["commit", "-q", "-am", "topic"]);
    git(repo, &["checkout", "-q", "main"]);
    write(repo, "a.rs", "main\n");
    git(repo, &["commit", "-q", "-am", "main"]);

    let candidate = read(
        repo,
        ChangeScope::MergeCandidate {
            ours: "main".to_string(),
            theirs: "topic".to_string(),
        },
    )
    .expect("candidate");
    assert_eq!(paths(&candidate), ["a.rs", "c.rs"]);
    assert!(matches!(candidate.files[0].change, FileChange::Unknown(_)));
    assert!(matches!(candidate.files[0].after, Content::Unreadable(_)));
    assert_eq!(candidate.files[1].change, FileChange::Modified);
    assert_eq!(candidate.files[1].after, bytes("topic only\n"));
}

#[test]
fn an_unborn_head_still_has_a_staged_change() {
    let directory = tempfile::tempdir().expect("temporary repository");
    let repo = directory.path();
    git(repo, &["init", "-q", "-b", "main"]);
    write(repo, "a.rs", "first\n");
    git(repo, &["add", "a.rs"]);

    let staged = read(repo, ChangeScope::Staged).expect("staged");
    assert_eq!(paths(&staged), ["a.rs"]);
    assert_eq!(staged.files[0].change, FileChange::Added);
    assert_eq!(staged.files[0].after, bytes("first\n"));
}

#[test]
fn a_reference_that_does_not_exist_is_unavailable_not_empty() {
    let directory = repo_with(&[("a.rs", "one\n")]);
    let error = read(
        directory.path(),
        ChangeScope::Commit {
            commit: "no-such-ref".to_string(),
        },
    )
    .expect_err("unknown reference");
    assert!(error.contains("not available"), "{error}");
}

#[test]
fn a_clean_tree_has_no_changed_file() {
    let directory = repo_with(&[("a.rs", "one\n")]);
    let tree = read(directory.path(), ChangeScope::WorkingTree).expect("working tree");
    assert!(tree.files.is_empty());
    assert_eq!(tree.files_not_read, 0);
}
