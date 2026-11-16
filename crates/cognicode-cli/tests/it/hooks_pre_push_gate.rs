//! CAA-10 (CGH-028) — the pre-push gate is fail-closed over real git.
//!
//! git pipes `<local_ref> <local_oid> <remote_ref> <remote_oid>` per
//! pushed ref into the shim. This drives the published binary against a
//! throwaway repository and pins which pushes may pass:
//! a fast-forward, whose recorded range is enumerable;
//! a new remote branch, which still has to push a recorded tip;
//! and which must never pass: an unrecorded rewind and an
//! unresolvable tip — both are inconclusive analyses, and the gate
//! refuses to invent a PASS (AC #3).

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::common;

fn run_git(repo: &Path, args: &[&str]) -> String {
    let sealed = repo.join("no-git-configuration");
    let output = Command::new("git")
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_GLOBAL", &sealed)
        .env("GIT_CONFIG_SYSTEM", &sealed)
        .args(args)
        .current_dir(repo)
        .output()
        .expect("git is available");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf-8")
}

/// Pushes `line` through `cognicode hooks run pre-push` and returns
/// the exit code together with what the gate said.
fn gate(repo: &Path, line: &str) -> (i32, String) {
    let binary = common::binary_path("cognicode");
    let sealed = repo.join("no-git-configuration");
    let mut child = Command::new(&binary)
        .args(["hooks", "run", "pre-push"])
        .current_dir(repo)
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_GLOBAL", &sealed)
        .env("GIT_CONFIG_SYSTEM", &sealed)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary spawns");
    child
        .stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(line.as_bytes())
        .expect("stdin accepts the git line");
    let output = child.wait_with_output().expect("the gate terminates");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (output.status.code().expect("exit code"), said)
}

#[test]
fn a_recorded_fast_forward_is_pushable() {
    let (_root, repo_dir, first, second) = repository();
    let line = format!("refs/heads/main {second} refs/heads/main {first}\n");
    let (code, said) = gate(&repo_dir, &line);
    assert_eq!(code, 0, "the gate blocked a fast-forward: {said}");
}

#[test]
fn a_new_remote_branch_still_has_to_push_a_recorded_tip() {
    let (_root, repo_dir, _first, second) = repository();
    let zeros = "0".repeat(40);
    let line = format!("refs/heads/new {second} refs/heads/new {zeros}\n");
    let (code, said) = gate(&repo_dir, &line);
    assert_eq!(code, 0, "a recorded tip may open a branch: {said}");

    let ghost = "d".repeat(40);
    let line = format!("refs/heads/new {ghost} refs/heads/new {zeros}\n");
    let (code, said) = gate(&repo_dir, &line);
    assert_eq!(code, 1, "an unresolvable tip must never pass: {said}");
}

#[test]
fn an_unrecorded_rewind_fails_closed() {
    let (_root, repo_dir, first, second) = repository();
    // Pushing the older commit over the newer one is not a recorded
    // descendant step, so the range cannot be enumerated.
    let line = format!("refs/heads/main {first} refs/heads/main {second}\n");
    let (code, said) = gate(&repo_dir, &line);
    assert_eq!(code, 1, "a rewind must not fabricate a PASS: {said}");
    assert!(
        said.contains("cognicode pre-push"),
        "the refusal says which gate spoke: {said}"
    );
}

/// (root, repo, first, second) — keeps the temporary directory alive.
fn repository() -> (tempfile::TempDir, std::path::PathBuf, String, String) {
    let root = tempfile::tempdir().expect("temporary root");
    let repo = root.path().join("repo");
    std::fs::create_dir_all(&repo).expect("repo dir");
    run_git(&repo, &["init", "-b", "main", "."]);
    run_git(&repo, &["config", "user.email", "t@example.com"]);
    run_git(&repo, &["config", "user.name", "Tester"]);
    std::fs::write(repo.join("a.rs"), "one\n").expect("first content");
    run_git(&repo, &["add", "a.rs"]);
    run_git(&repo, &["commit", "-m", "first"]);
    let first = run_git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    std::fs::write(repo.join("a.rs"), "two\n").expect("second content");
    run_git(&repo, &["commit", "-qam", "second"]);
    let second = run_git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    (root, repo, first, second)
}
