//! CAA-03 / CGH-021 — the Git adapter over the neutral SCM ports.
//!
//! Pins the four acceptance criteria against REAL throwaway repositories
//! (the `rename_evidence.rs` precedent: the adapter's promise is exactly
//! that it reads Git truth, so the tests drive the real binary):
//!
//! 1. HEAD, index, working tree and untracked files are FOUR different
//!    byte sets, and the adapter names the bytes it analyzed (AC #1).
//! 2. `B...F` (merge-base) and `D...merge(D,F)` (virtual candidate)
//!    produce different comparisons with explicit SHAs (AC #2).
//! 3. Rename, copy, staged rename, submodule, newline/unicode paths and
//!    shallow clones never collapse into a fictitiously empty result
//!    (AC #3).
//! 4. No shell, dash-prefixed references are data not options, and a
//!    broken provider fails with a named error (AC #4).

use std::fs;
use std::path::Path;
use std::process::Command;

use cognicode_core::domain::scm::{
    ChangeKind, ChangeReader, PathKey, RevisionReader, ScmError, SourceRef, WorkingCopyReader,
};
use cognicode_core::infrastructure::git::GitScmAdapter;
use tempfile::TempDir;

fn source() -> SourceRef {
    SourceRef::new("git", "adapter-test").expect("non-empty source")
}

fn digest_of(bytes: &[u8]) -> cognicode_core::domain::scm::ContentDigest {
    cognicode_core::domain::scm::ContentDigest::from_bytes(bytes)
}

fn key(p: &str) -> PathKey {
    PathKey::new(p).unwrap_or_else(|e| panic!("path {p:?} rejected: {e}"))
}

/// Runs git in `repo` with a hermetic environment (the precedent in
/// `rename_evidence.rs`: no user config, no hooks, pinned PATH).
fn run_git(repo: &Path, args: &[&str]) -> String {
    let no_configuration = repo.join("no-git-configuration");
    let output = Command::new("git")
        .env("PATH", "/usr/bin:/bin")
        .args(args)
        .current_dir(repo)
        .env("GIT_CONFIG_GLOBAL", &no_configuration)
        .env("GIT_CONFIG_SYSTEM", &no_configuration)
        .output()
        .expect("git CLI is available for adapter tests");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("git stdout is UTF-8")
}

fn init_repo() -> TempDir {
    let repo = tempfile::tempdir().expect("tempdir");
    run_git(repo.path(), &["init", "-q", "-b", "main"]);
    run_git(repo.path(), &["config", "user.name", "caa03 Test"]);
    run_git(
        repo.path(),
        &["config", "user.email", "caa03-test@example.invalid"],
    );
    run_git(repo.path(), &["config", "commit.gpgsign", "false"]);
    repo
}

fn commit_all(repo: &Path, message: &str) -> String {
    run_git(repo, &["add", "-A"]);
    run_git(repo, &["commit", "-q", "-m", message]);
    run_git(repo, &["rev-parse", "HEAD"]).trim().to_string()
}

/// One repo, one commit, with a.txt "v1" and b.txt "b1".
fn repo_with_baseline() -> (TempDir, String) {
    let repo = init_repo();
    fs::write(repo.path().join("a.txt"), "v1\n").expect("write a.txt");
    fs::write(repo.path().join("b.txt"), "b1\n").expect("write b.txt");
    let sha = commit_all(repo.path(), "baseline");
    (repo, sha)
}

// ----- AC #1: HEAD, index, working tree, untracked are four sets ----

#[test]
fn head_index_worktree_and_untracked_are_distinguished() {
    let (repo, head_sha) = repo_with_baseline();
    // index: a.txt "v2" staged; worktree: a.txt "v3" unstaged.
    fs::write(repo.path().join("a.txt"), "v2\n").expect("stage v2");
    run_git(repo.path(), &["add", "a.txt"]);
    fs::write(repo.path().join("a.txt"), "v3\n").expect("worktree v3");
    fs::write(repo.path().join("u.txt"), "untracked\n").expect("write u.txt");

    let adapter = GitScmAdapter::new(repo.path());
    let src = source();
    let head = adapter
        .resolve(&src, "HEAD")
        .expect("HEAD resolves in a real repo");
    assert_eq!(head.handle().as_str(), head_sha);

    // HEAD bytes are the COMMITTED ones, not the worktree's.
    let inventory = adapter.inventory(&head).expect("head inventory");
    let a_path = key("a.txt");
    assert_eq!(
        inventory.find(&a_path).expect("a.txt tracked").content,
        digest_of(b"v1\n"),
        "the inventory reports the committed bytes of a.txt"
    );
    assert_eq!(
        adapter.content(&head, &a_path).expect("content"),
        b"v1\n",
        "content() analyzes HEAD bytes, never the worktree"
    );

    // The staged set is HEAD -> INDEX: digest after = v2, not v3.
    let staged = adapter.staged_changes(&src).expect("staged changes");
    assert_eq!(staged.base().as_str(), head_sha, "staged base is HEAD");
    let modified_a = staged
        .changes()
        .iter()
        .find(|c| c.path() == &a_path)
        .expect("a.txt is staged-modified");
    assert!(matches!(modified_a.kind(), ChangeKind::Modified));
    assert_eq!(
        modified_a.after().cloned().expect("after digest"),
        digest_of(b"v2\n"),
        "the staged side reads the INDEX bytes"
    );
    assert_ne!(
        modified_a.after().cloned().expect("after digest"),
        digest_of(b"v3\n"),
        "the staged side must never read the WORKTREE bytes"
    );
    assert_eq!(
        staged
            .changes()
            .iter()
            .filter(|c| c.path() == &key("b.txt"))
            .count(),
        0,
        "unchanged content never claims work"
    );

    // Dirty paths: worktree-vs-index differences plus untracked.
    let dirty = adapter.dirty_paths(&src).expect("dirty paths");
    assert!(
        dirty.contains(&a_path),
        "a.txt worktree differs from its index entry: {dirty:?}"
    );
    assert!(
        dirty.contains(&key("u.txt")),
        "untracked files are working-copy dirt: {dirty:?}"
    );
    assert!(
        !dirty.contains(&key("b.txt")),
        "b.txt is untouched: {dirty:?}"
    );
}

#[test]
fn inventory_and_revision_share_one_content_digest() {
    let (repo, _sha) = repo_with_baseline();
    let adapter = GitScmAdapter::new(repo.path());
    let head = adapter.resolve(&source(), "HEAD").expect("HEAD");
    let inventory = adapter.inventory(&head).expect("inventory");
    assert_eq!(
        head.content(),
        &inventory.content_digest(),
        "a revision's content IS its tracked-tree digest (the CAA-04 reuse key)"
    );
}

// ----- AC #2: B...F vs D...merge(D,F) are different comparisons ----

/// base commit C1; branch F touches f.txt; branch D touches d.txt.
fn diverging_repo() -> (TempDir, String, String, String) {
    let repo = init_repo();
    fs::write(repo.path().join("f.txt"), "base\n").expect("write");
    fs::write(repo.path().join("d.txt"), "base\n").expect("write");
    let c1 = commit_all(repo.path(), "C1");
    run_git(repo.path(), &["checkout", "-q", "-b", "F"]);
    fs::write(repo.path().join("f.txt"), "on-F\n").expect("write");
    let f = commit_all(repo.path(), "F");
    run_git(repo.path(), &["checkout", "-q", "main"]);
    run_git(repo.path(), &["checkout", "-q", "-b", "D"]);
    fs::write(repo.path().join("d.txt"), "on-D\n").expect("write");
    let d = commit_all(repo.path(), "D");
    (repo, c1, d, f)
}

#[test]
fn two_dot_unrelated_branches_fail_closed_with_named_ancestry_error() {
    let (repo, _c1, d, f) = diverging_repo();
    let adapter = GitScmAdapter::new(repo.path());
    let src = source();
    let rev_d = adapter.resolve(&src, &d).expect("D");
    let rev_f = adapter.resolve(&src, &f).expect("F");
    let error = adapter
        .changes_between(&rev_d, &rev_f)
        .expect_err("unrelated pair has no honest two-dot diff");
    assert!(
        matches!(error, ScmError::MissingAncestor { .. }),
        "got {error:?}"
    );
    assert!(
        !adapter.is_ancestor(&rev_d, &rev_f).expect("query answers"),
        "D is not an ancestor of F"
    );
}

#[test]
fn three_dot_reads_merge_base_and_reports_it_explicitly() {
    let (repo, c1, d, f) = diverging_repo();
    let adapter = GitScmAdapter::new(repo.path());
    // B...F == changes since merge-base(D,F) up to F.
    let changes = adapter
        .changes_since_merge_base("D", "F")
        .expect("three-dot comparison");
    assert_eq!(
        changes.base().as_str(),
        adapter
            .resolve(&source(), &c1)
            .expect("C1")
            .handle()
            .as_str(),
        "the base is the merge-base SHA, stated explicitly"
    );
    assert_eq!(changes.head().as_str(), f);
    let paths: Vec<&str> = changes
        .changes()
        .iter()
        .map(|c| c.path().as_str())
        .collect();
    assert_eq!(paths, ["f.txt"], "B...F sees only F's side: {paths:?}");
    // The 40-hex SHA contract holds for every handle.
    assert!(
        changes.base().as_str().len() == 40 && d.len() == 40,
        "handles are explicit full SHAs"
    );
}

#[test]
fn merge_candidate_compares_against_a_virtual_tree() {
    let (repo, _c1, _d, _f) = diverging_repo();
    let adapter = GitScmAdapter::new(repo.path());
    let candidate = adapter
        .merge_candidate("D", "F")
        .expect("clean merge-tree candidate");
    assert!(
        candidate.conflicts.is_empty(),
        "disjoint edits merge cleanly: {:?}",
        candidate.conflicts
    );
    assert_eq!(
        candidate.revision.handle().as_str().len(),
        40,
        "the candidate tree is named by an explicit SHA"
    );

    let changes = adapter
        .changes_vs_merge_candidate("D", "F")
        .expect("D...merge(D,F)");
    let rev_d = adapter.resolve(&source(), "D").expect("D");
    assert_eq!(changes.base(), rev_d.handle(), "base is D's own SHA");
    let paths: Vec<&str> = changes
        .changes()
        .iter()
        .map(|c| c.path().as_str())
        .collect();
    assert_eq!(
        paths,
        ["f.txt"],
        "from D's view, only F's change is new content"
    );
    // And it is NOT the same comparison as B...F: different head handle
    // (virtual tree SHA, not commit F).
    let three_dot = adapter
        .changes_since_merge_base("D", "F")
        .expect("three-dot");
    assert_ne!(
        three_dot.head().as_str(),
        changes.head().as_str(),
        "D...merge(D,F) ends at the candidate tree, not at F"
    );
    // The candidate really contains BOTH sides' bytes.
    let inv = adapter
        .inventory(&candidate.revision)
        .expect("candidate inventory");
    assert_eq!(
        inv.find(&key("f.txt")).expect("f.txt").content,
        digest_of(b"on-F\n")
    );
    assert_eq!(
        inv.find(&key("d.txt")).expect("d.txt").content,
        digest_of(b"on-D\n")
    );
}

// ----- AC #3: special shapes never collapse to a fake empty --------

#[test]
fn rename_is_folded_into_one_renamed_change_with_evidence() {
    let (repo, before) = repo_with_baseline();
    fs::rename(repo.path().join("a.txt"), repo.path().join("renamed.txt")).expect("rename");
    let after = commit_all(repo.path(), "rename a");
    let adapter = GitScmAdapter::new(repo.path());
    let src = source();
    let base = adapter.resolve(&src, &before).expect("base");
    let head = adapter.resolve(&src, &after).expect("head");
    let changes = adapter.changes_between(&base, &head).expect("rename diff");
    let folded: Vec<&PathKey> = changes
        .changes()
        .iter()
        .filter(|c| matches!(c.kind(), ChangeKind::Renamed { .. }))
        .map(|c| c.path())
        .collect();
    assert_eq!(
        folded,
        vec![&key("renamed.txt")],
        "delete+add pairs into ONE rename: {:?}",
        changes.changes()
    );
    let renamed = changes
        .changes()
        .iter()
        .find(|c| matches!(c.kind(), ChangeKind::Renamed { .. }))
        .expect("rename present");
    assert!(
        matches!(renamed.kind(), ChangeKind::Renamed { from } if from == &key("a.txt")),
        "the rename names its source"
    );
    assert!(!renamed.requires_reparse(), "renamed content is reusable");
}

#[test]
fn copy_is_marked_with_evidence() {
    let (repo, before) = repo_with_baseline();
    let content = fs::read(repo.path().join("a.txt")).expect("read");
    fs::write(repo.path().join("a_copy.txt"), content).expect("write copy");
    let after = commit_all(repo.path(), "copy a");
    let adapter = GitScmAdapter::new(repo.path());
    let src = source();
    let base = adapter.resolve(&src, &before).expect("base");
    let head = adapter.resolve(&src, &after).expect("head");
    let changes = adapter.changes_between(&base, &head).expect("copy diff");
    let copied = changes
        .changes()
        .iter()
        .find(|c| matches!(c.kind(), ChangeKind::Copied { .. }))
        .unwrap_or_else(|| panic!("copy evidence expected, got {:?}", changes.changes()));
    assert!(
        matches!(copied.kind(), ChangeKind::Copied { from } if from == &key("a.txt")),
        "the clone names its source"
    );
}

#[test]
fn staged_rename_via_git_mv_is_not_delete_plus_add() {
    let (repo, _head) = repo_with_baseline();
    run_git(repo.path(), &["mv", "a.txt", "moved.txt"]);
    let adapter = GitScmAdapter::new(repo.path());
    let staged = adapter.staged_changes(&source()).expect("staged");
    assert!(
        staged.changes().iter().any(|c| {
            matches!(c.kind(), ChangeKind::Renamed { from } if *from == key("a.txt"))
                && c.path() == &key("moved.txt")
        }),
        "git mv stages as a rename: {:?}",
        staged.changes()
    );
}

#[test]
fn submodule_pointer_change_is_visible_and_its_bytes_are_named_absent() {
    let (repo, before) = repo_with_baseline();
    // A real recorded gitlink without needing the network.
    let fake_oid = "0123456789012345678901234567890123456789";
    run_git(
        repo.path(),
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{fake_oid},vendor/sub"),
        ],
    );
    // Commit the recorded gitlink DIRECTLY: `git add -A` would rewrite
    // the index from a worktree where vendor/sub does not exist and
    // silently drop the entry git is being asked to prove it reports.
    run_git(repo.path(), &["commit", "-q", "-m", "add submodule"]);
    let after = run_git(repo.path(), &["rev-parse", "HEAD"])
        .trim()
        .to_string();
    let adapter = GitScmAdapter::new(repo.path());
    let src = source();
    let base = adapter.resolve(&src, &before).expect("base");
    let head = adapter.resolve(&src, &after).expect("head");

    let changes = adapter
        .changes_between(&base, &head)
        .expect("submodule diff is not empty");
    let added = changes
        .changes()
        .iter()
        .find(|c| c.path() == &key("vendor/sub"))
        .unwrap_or_else(|| panic!("submodule change visible: {:?}", changes.changes()));
    assert!(matches!(added.kind(), ChangeKind::Added));

    // Its bytes are NOT in this object store: named absence, not empty.
    let error = adapter
        .content(&head, &key("vendor/sub"))
        .expect_err("gitlink content lives in the other repo");
    assert!(
        matches!(error, ScmError::ContentUnavailable { .. }),
        "got {error:?}"
    );
}

#[test]
fn newline_and_unicode_paths_survive_the_wire_format() {
    let repo = init_repo();
    let odd = "línea\nrara.txt";
    fs::write(repo.path().join(odd), "rara\n").expect("write odd name");
    let sha = commit_all(repo.path(), "odd name");
    let adapter = GitScmAdapter::new(repo.path());
    let src = source();
    let head = adapter.resolve(&src, &sha).expect("head");
    let odd_key = key(odd);
    let inventory = adapter.inventory(&head).expect("inventory");
    assert!(
        inventory.find(&odd_key).is_some(),
        "the NUL-safe read keeps the newline in the name: {:?}",
        inventory
            .entries()
            .iter()
            .map(|e| e.path.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        adapter.content(&head, &odd_key).expect("content"),
        b"rara\n",
        "bytes of a newline-named path are read exactly"
    );
    // And it stays dirty-visible with an untracked odd name.
    fs::write(repo.path().join("ñu\nño.txt"), "new\n").expect("write");
    let dirty = adapter.dirty_paths(&src).expect("dirty");
    assert!(
        dirty.contains(&key("ñu\nño.txt")),
        "untracked newline path is dirty: {dirty:?}"
    );
}

#[test]
fn shallow_clone_fails_named_instead_of_fictitiously_empty() {
    let (origin, first_sha) = repo_with_baseline();
    fs::write(origin.path().join("a.txt"), "v2\n").expect("write");
    let tip = commit_all(origin.path(), "second");
    let clone_parent = tempfile::tempdir().expect("clone parent");
    let clone_path = clone_parent.path().join("shallow");
    // file:// is required: --depth is silently ignored in local-path clones.
    let url = format!("file://{}", origin.path().display());
    run_git(
        clone_parent.path(),
        &[
            "clone",
            "-q",
            "--depth",
            "1",
            "--",
            &url,
            clone_path.to_str().expect("utf8 path"),
        ],
    );
    let adapter = GitScmAdapter::new(&clone_path);
    let src = source();
    // The shallow tip reads fine…
    let head = adapter.resolve(&src, "HEAD").expect("tip present");
    assert_eq!(head.handle().as_str(), tip);
    // …but the parent was never fetched: a NAMED failure, never an
    // Ok(empty) diff that would silently claim "nothing changed".
    let error = adapter
        .resolve(&src, &first_sha)
        .expect_err("parent absent from a depth-1 clone");
    assert!(
        matches!(error, ScmError::ReferenceNotFound { .. }),
        "got {error:?}"
    );
}

// ----- AC #4: command safety ---------------------------------------

#[test]
fn dash_prefixed_reference_is_data_not_an_option() {
    let (repo, _sha) = repo_with_baseline();
    let adapter = GitScmAdapter::new(repo.path());
    let error = adapter
        .resolve(&source(), "--help")
        .expect_err("a dash-prefixed reference cannot reach argv as an option");
    assert!(
        matches!(error, ScmError::ReferenceNotFound { .. }),
        "got {error:?}"
    );
    assert!(
        std::fs::read_dir(repo.path())
            .expect("read dir")
            .flatten()
            .any(|e| e.file_name() == "a.txt"),
        "the repository is untouched by the refused reference"
    );
}

#[test]
fn missing_git_program_is_a_named_provider_failure_not_an_empty_ok() {
    let (repo, _sha) = repo_with_baseline();
    let adapter = GitScmAdapter::with_git_program(repo.path(), "/nonexistent-caa03/git");
    let error = adapter
        .resolve(&source(), "HEAD")
        .expect_err("spawn failure is reported, not swallowed");
    assert!(
        matches!(error, ScmError::ProviderFailure { .. }),
        "got {error:?}"
    );
}

#[test]
fn unknown_provider_is_refused_by_name() {
    let (repo, _sha) = repo_with_baseline();
    let adapter = GitScmAdapter::new(repo.path());
    let foreign = SourceRef::new("mercurial", "adapter-test").expect("valid ref");
    let error = adapter
        .resolve(&foreign, "HEAD")
        .expect_err("the git adapter only serves git providers");
    assert!(
        matches!(error, ScmError::ReferenceNotFound { ref provider, .. } if provider.as_str() == "mercurial"),
        "got {error:?}"
    );
}
