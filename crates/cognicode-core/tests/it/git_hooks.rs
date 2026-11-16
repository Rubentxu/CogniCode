//! CAA-10 / CGH-028 — git hooks as thin coordinator clients.
//!
//! Drives REAL throwaway repositories (the CAA-03 precedent: the
//! promise of these hooks is exactly that they read Git truth):
//!
//! 1. pre-commit gates the exact staged index, even when partial
//!    staging means the working tree holds different bytes (AC #1).
//! 2. Install/uninstall is opt-in, reversible and honours
//!    `core.hooksPath` and worktrees without overwriting third-party
//!    hooks (AC #2).
//! 3. Gate events fail closed on unmerged index entries; observe
//!    events never block (AC #3).
//! 4. Managed scripts execute only the installer-chosen binary and
//!    need no daemon (AC #4).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cognicode_core::application::change_tracking::hooks::{
    self, HookEvent, MANAGED_MARKER, SlotState,
};
use cognicode_core::domain::scm::{ChangeKind, ContentDigest, SourceRef, WorkingCopyReader};
use cognicode_core::infrastructure::git::GitScmAdapter;
use tempfile::TempDir;

fn source() -> SourceRef {
    SourceRef::new("git", "hooks-test").expect("non-empty source")
}

fn digest_of(bytes: &[u8]) -> ContentDigest {
    ContentDigest::from_bytes(bytes)
}

/// Runs git in `repo` hermetically: no user config, no hooks.
fn run_git(repo: &Path, args: &[&str]) -> String {
    let no_configuration = repo.join("no-git-configuration");
    let output = Command::new("git")
        .env("PATH", "/usr/bin:/bin")
        .args(args)
        .current_dir(repo)
        .env("GIT_CONFIG_GLOBAL", &no_configuration)
        .env("GIT_CONFIG_SYSTEM", &no_configuration)
        .output()
        .expect("git CLI is available for hook tests");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("git stdout is UTF-8")
}

fn init_repo(dir: &Path) -> PathBuf {
    let repo = dir.join("repo");
    fs::create_dir_all(&repo).expect("repo dir");
    run_git(&repo, &["init", "-b", "main", "."]);
    run_git(&repo, &["config", "user.email", "t@example.com"]);
    run_git(&repo, &["config", "user.name", "Tester"]);
    run_git(&repo, &["config", "commit.gpgsign", "false"]);
    repo
}

/// A test-local runner that shells out to real git with the same
/// hermetic environment as `run_git`, so `resolve_hooks_dir` is
/// exercised against genuine git output without touching process env.
struct HermeticGit {
    config_shield: PathBuf,
}

impl hooks::CommandRunner for HermeticGit {
    fn run(&self, args: &[&str]) -> Result<(i32, String), hooks::HookError> {
        let output = Command::new("git")
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_GLOBAL", &self.config_shield)
            .env("GIT_CONFIG_SYSTEM", &self.config_shield)
            .args(args)
            .output()
            .map_err(|error| hooks::HookError::Io {
                path: "git".to_string(),
                reason: error.to_string(),
            })?;
        Ok((
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        ))
    }
}

#[test]
fn ac1_pre_commit_gates_the_exact_index_under_partial_staging() {
    let root = TempDir::new().expect("tempdir");
    let repo = init_repo(root.path());
    fs::write(repo.join("a.rs"), "one\n").expect("write a");
    run_git(&repo, &["add", "a.rs"]);
    run_git(&repo, &["commit", "-m", "base"]);

    // Partial staging: the INDEX gets "1.5" while the working tree
    // keeps different bytes "two". A naive hook would read "two".
    let staged_content_file = root.path().join("staged-a");
    fs::write(&staged_content_file, b"1.5\n").expect("staged content");
    let staged_blob = run_git(
        &repo,
        &[
            "hash-object",
            "-w",
            &staged_content_file.display().to_string(),
        ],
    )
    .trim()
    .to_string();
    fs::write(repo.join("a.rs"), "two\n").expect("dirty worktree");
    fs::write(repo.join("b.rs"), "added\n").expect("write b");
    run_git(&repo, &["add", "b.rs"]);
    run_git(
        &repo,
        &[
            "update-index",
            "--cacheinfo",
            "100644",
            &staged_blob,
            "a.rs",
        ],
    );

    let adapter = GitScmAdapter::new(&repo);
    let staged = adapter.staged_changes(&source()).expect("staged changes");
    let kinds: Vec<(String, ChangeKind, Option<ContentDigest>)> = staged
        .changes()
        .iter()
        .map(|change| {
            (
                change.path().as_str().to_string(),
                change.kind().clone(),
                change.after().cloned(),
            )
        })
        .collect();
    let a = kinds
        .iter()
        .find(|(path, _, _)| path == "a.rs")
        .expect("a.rs staged");
    assert_eq!(a.1, ChangeKind::Modified);
    assert_eq!(
        a.2.as_ref(),
        Some(&digest_of(b"1.5\n")),
        "the gate sees the staged bytes, not the working tree"
    );
    let b = kinds
        .iter()
        .find(|(path, _, _)| path == "b.rs")
        .expect("b.rs staged");
    assert_eq!(b.1, ChangeKind::Added);
    assert_eq!(b.2.as_ref(), Some(&digest_of(b"added\n")));

    assert_eq!(hooks::verdict_from_staged(&staged), hooks::Verdict::Pass);
    assert_eq!(
        hooks::decide(HookEvent::PreCommit, &hooks::verdict_from_staged(&staged)),
        hooks::HookDecision::Proceed
    );
}

#[test]
fn ac3_unmerged_index_blocks_the_gate_while_observe_events_proceed() {
    let root = TempDir::new().expect("tempdir");
    let repo = init_repo(root.path());
    fs::write(repo.join("a.rs"), "base\n").expect("write");
    run_git(&repo, &["add", "a.rs"]);
    run_git(&repo, &["commit", "-m", "base"]);
    run_git(&repo, &["checkout", "-b", "side"]);
    fs::write(repo.join("a.rs"), "side\n").expect("side write");
    run_git(&repo, &["commit", "-am", "side"]);
    run_git(&repo, &["checkout", "main"]);
    fs::write(repo.join("a.rs"), "main\n").expect("main write");
    run_git(&repo, &["commit", "-am", "main"]);
    let merge = Command::new("git")
        .env("PATH", "/usr/bin:/bin")
        .args(["merge", "side"])
        .current_dir(&repo)
        .output()
        .expect("git merge");
    assert!(!merge.status.success(), "merge must conflict");

    let adapter = GitScmAdapter::new(&repo);
    let staged = adapter
        .staged_changes(&source())
        .expect("staged during conflict");
    let verdict = hooks::verdict_from_staged(&staged);
    match &verdict {
        hooks::Verdict::NotPass(reasons) => assert!(
            reasons.iter().any(|r| r == "unmerged path a.rs"),
            "conflict must name the unmerged path, got {reasons:?}"
        ),
        other => panic!("expected NotPass for unmerged index, got {other:?}"),
    }
    assert_eq!(
        hooks::decide(HookEvent::PreCommit, &verdict),
        hooks::HookDecision::Block {
            reasons: vec!["unmerged path a.rs".to_string()]
        }
    );
    assert_eq!(hooks::decide(HookEvent::PreCommit, &verdict).exit_code(), 1);
    // Observe events never block and never mint approvals.
    assert_eq!(
        hooks::decide(HookEvent::PostRewrite, &verdict),
        hooks::HookDecision::Proceed
    );
}

#[test]
fn ac2_install_round_trip_honours_hooks_path_worktrees_and_foreign_hooks() {
    let root = TempDir::new().expect("tempdir");
    let repo = init_repo(root.path());
    let runner = HermeticGit {
        config_shield: root.path().join("no-git-configuration"),
    };

    // Default layout resolves inside .git/hooks.
    let default_dir = hooks::resolve_hooks_dir(&runner, &repo).expect("resolve");
    assert!(default_dir.ends_with(".git/hooks") || default_dir.ends_with(".git\\hooks"));

    // Worktrees share the common hooks directory.
    fs::write(repo.join("a.rs"), "one\n").expect("write");
    run_git(&repo, &["add", "a.rs"]);
    run_git(&repo, &["commit", "-m", "base"]);
    let worktree = root.path().join("wt");
    run_git(
        &repo,
        &[
            "worktree",
            "add",
            &worktree.display().to_string(),
            "-b",
            "wt",
        ],
    );
    let from_worktree = hooks::resolve_hooks_dir(&runner, &worktree).expect("resolve wt");
    assert_eq!(
        from_worktree.canonicalize().ok(),
        default_dir.canonicalize().ok(),
        "a worktree must not get a private copy of the managed hooks"
    );

    // A custom core.hooksPath is honoured, never rewritten.
    let custom = root.path().join("team-hooks");
    fs::create_dir_all(&custom).expect("custom dir");
    run_git(
        &repo,
        &["config", "core.hooksPath", &custom.display().to_string()],
    );
    let resolved_custom = hooks::resolve_hooks_dir(&runner, &repo).expect("resolve custom");
    assert_eq!(resolved_custom, custom);
    let config_after = run_git(&repo, &["config", "--get", "core.hooksPath"]);
    assert_eq!(config_after.trim(), custom.display().to_string());

    // Foreign hook is preserved; managed slots round-trip.
    let foreign = custom.join("pre-commit");
    fs::write(&foreign, b"#!/bin/sh\nexit 0\n").expect("foreign");
    let report = hooks::install(&custom, Path::new("/opt/cognicode/cognicode")).expect("install");
    assert_eq!(report.installed.len(), 4);
    assert_eq!(report.foreign_kept, vec![HookEvent::PreCommit]);
    assert_eq!(
        fs::read(&foreign).expect("read foreign"),
        b"#!/bin/sh\nexit 0\n"
    );
    let entries = hooks::status(&custom).expect("status");
    assert!(
        entries
            .iter()
            .any(|(event, state)| *event == HookEvent::PrePush && *state == SlotState::Managed)
    );
    let report = hooks::uninstall(&custom).expect("uninstall");
    assert_eq!(report.removed.len(), 4);
    assert_eq!(report.foreign_kept, vec![HookEvent::PreCommit]);
    assert!(foreign.exists() && !custom.join("post-merge").exists());
    assert_eq!(config_after.trim(), custom.display().to_string());
}

#[test]
fn ac4_scripts_exec_only_the_installer_chosen_binary_and_need_no_daemon() {
    let root = TempDir::new().expect("tempdir");
    let repo = init_repo(root.path());
    let hooks_dir = repo.join(".git/hooks");
    fs::create_dir_all(&hooks_dir).expect("hooks dir");
    let program = Path::new("/opt/cognicode/cognicode");
    let report = hooks::install(&hooks_dir, program).expect("install");
    assert_eq!(report.installed.len(), 5);
    for event in HookEvent::ALL {
        let script = fs::read_to_string(hooks_dir.join(event.git_name())).expect("script");
        assert!(script.contains(MANAGED_MARKER));
        // The only executed command is the binary chosen by the person
        // who installed: no repository content, no shell eval, no
        // daemon socket dependency in the shim itself.
        assert!(script.contains(&format!(
            "exec '{}' hooks run {}",
            program.display(),
            event.git_name()
        )));
        assert!(!script.contains("eval"));
        assert!(!script.contains(".env"));
        assert_eq!(script.lines().count(), 3);
    }
}
