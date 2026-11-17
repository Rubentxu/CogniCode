//! Behaviour of the `preflight` goal over real Git repositories: what the
//! answer says, and what the call leaves behind.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::sync::Barrier;

use super::*;
use crate::interface::goals::revision_support::produce_for_ref;

const ALPHA: &str = "pub fn alpha() { first(); }\n";
const BETA: &str = "pub fn beta() { helper(); }\n";
const DELTA: &str = "pub fn delta() {}\n";

fn git(root: &Path, args: &[&str]) -> String {
    let no_configuration = root.join("no-git-configuration");
    let output = Command::new("git")
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_GLOBAL", &no_configuration)
        .env("GIT_CONFIG_SYSTEM", &no_configuration)
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git is installed");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("git prints UTF-8")
}

/// A repository whose only commit tracks `files`.
fn repository(files: &[(&str, &str)]) -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("temporary repository");
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

fn standard() -> tempfile::TempDir {
    repository(&[("a.rs", ALPHA), ("b.rs", BETA), ("d.rs", DELTA)])
}

fn input(edits: &[(&str, Option<&str>)]) -> PreflightInput {
    PreflightInput {
        base_ref: "HEAD".to_string(),
        edits: edits
            .iter()
            .map(|(path, text)| (path.to_string(), text.map(str::to_string)))
            .collect(),
        session: None,
    }
}

fn session_input(session: &str, edits: &[(&str, Option<&str>)]) -> PreflightInput {
    PreflightInput {
        session: Some(session.to_string()),
        ..input(edits)
    }
}

/// Every file under `root` with its bytes, `.git` excluded.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(dir: &Path, root: &Path, into: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(&path, root, into);
            } else {
                let key = path.strip_prefix(root).expect("under root");
                into.insert(
                    key.to_string_lossy().into_owned(),
                    std::fs::read(&path).expect("read file"),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    files
}

/// What Git itself would say changed: the head, the refs and the status.
fn git_state(root: &Path) -> (String, String, String) {
    (
        git(root, &["rev-parse", "HEAD"]),
        git(root, &["for-each-ref"]),
        git(root, &["status", "--porcelain=v1", "--untracked-files=all"]),
    )
}

#[test]
fn edits_in_give_a_facts_delta_and_its_cost_out() {
    let repo = standard();

    let answer = preflight(
        repo.path(),
        None,
        &input(&[
            ("a.rs", Some("pub fn alpha() { second(); }\n")),
            ("c.rs", Some("pub fn gamma() {}\n")),
            ("b.rs", None),
        ]),
    )
    .expect("preflight");

    assert_eq!(
        answer.base.revision,
        git(repo.path(), &["rev-parse", "HEAD"]).trim()
    );
    assert!(answer.candidate.handle.starts_with("overlay:"));
    assert_ne!(answer.candidate.handle, answer.base.revision);
    assert_eq!(
        answer.impact_status, "evaluated",
        "{}",
        answer.impact_reason
    );
    assert!(answer.delta.facts_added > 0 && answer.delta.facts_removed > 0);
    let added: Vec<&str> = answer.delta.added.iter().map(|f| f.path.as_str()).collect();
    let removed: Vec<&str> = answer
        .delta
        .removed
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    assert!(
        added.contains(&"c.rs") && added.contains(&"a.rs"),
        "{added:?}"
    );
    assert!(
        removed.contains(&"b.rs") && removed.contains(&"a.rs"),
        "{removed:?}"
    );
    assert!(!added.contains(&"d.rs") && !removed.contains(&"d.rs"));
    assert_eq!(answer.cost.changed_paths, 3);
    assert_eq!(answer.cost.files_to_reparse, 2, "a.rs modified, c.rs added");
    assert_eq!(
        answer.cost.bytes_edited,
        "pub fn alpha() { second(); }\n".len() + "pub fn gamma() {}\n".len()
    );
    assert_eq!(answer.cost.base.tracked_files, 3);
    assert_eq!(answer.cost.candidate.tracked_files, 3);
    assert!(!answer.persisted);
}

#[test]
fn an_edit_that_restores_the_base_bytes_changes_nothing() {
    let repo = standard();

    let answer = preflight(repo.path(), None, &input(&[("a.rs", Some(ALPHA))])).expect("preflight");

    assert_eq!(answer.cost.changed_paths, 0);
    assert_eq!(
        (answer.delta.facts_added, answer.delta.facts_removed),
        (0, 0)
    );
}

#[test]
fn a_changed_file_no_reader_claims_makes_the_delta_partial() {
    let repo = standard();

    let answer =
        preflight(repo.path(), None, &input(&[("NOTES.txt", Some("hello\n"))])).expect("preflight");

    assert_eq!(answer.impact_status, "partial");
    assert_eq!(answer.delta.unanalyzed_changed_paths, vec!["NOTES.txt"]);
}

#[test]
fn a_preflight_writes_neither_the_workspace_nor_the_real_revision_store() {
    let repo = standard();
    let cache = tempfile::tempdir().expect("cache");
    let real =
        produce_for_ref(repo.path(), Some(cache.path()), "HEAD", true).expect("real revision");
    assert!(
        real.persisted,
        "the real revision is stored before the preflight"
    );
    let workspace_before = tree(repo.path());
    let git_before = git_state(repo.path());
    let store_before = tree(cache.path());
    assert!(!store_before.is_empty());

    let answer = preflight(
        repo.path(),
        Some(cache.path()),
        &input(&[
            ("a.rs", Some("pub fn alpha() { second(); }\n")),
            ("c.rs", Some("pub fn gamma() {}\n")),
            ("b.rs", None),
        ]),
    )
    .expect("preflight");

    assert_eq!(
        tree(repo.path()),
        workspace_before,
        "the workspace tree is byte-identical"
    );
    assert_eq!(git_state(repo.path()), git_before, "Git saw nothing happen");
    assert_eq!(
        tree(cache.path()),
        store_before,
        "the real revision store is byte-identical"
    );
    assert_eq!(
        answer.cost.candidate.reused_files, 1,
        "the unchanged file was served from the stored object, read-only"
    );
    assert!(
        store_before
            .keys()
            .all(|name| !name.contains(answer.candidate.handle.trim_start_matches("overlay:"))),
        "no entry of the store is keyed by the candidate"
    );
}

#[test]
fn a_preflight_over_an_empty_cache_root_creates_nothing_in_it() {
    let repo = standard();
    let cache = tempfile::tempdir().expect("cache");

    preflight(
        repo.path(),
        Some(cache.path()),
        &input(&[("a.rs", Some("fn x() {}\n"))]),
    )
    .expect("preflight");

    assert!(tree(cache.path()).is_empty());
    assert_eq!(std::fs::read_dir(cache.path()).expect("cache").count(), 0);
}

#[test]
fn two_parallel_sessions_over_the_same_base_never_see_each_other() {
    let repo = standard();
    let cache = tempfile::tempdir().expect("cache");
    produce_for_ref(repo.path(), Some(cache.path()), "HEAD", true).expect("real revision");
    let store_before = tree(cache.path());
    let barrier = Barrier::new(2);
    let run_session = |session: &str, name: &str| {
        let edit = format!("pub fn {name}() {{ call_{name}(); }}\n");
        (0..6)
            .map(|_| {
                barrier.wait();
                preflight(
                    repo.path(),
                    Some(cache.path()),
                    &session_input(session, &[("a.rs", Some(&edit))]),
                )
                .expect("preflight")
            })
            .collect::<Vec<_>>()
    };

    let (alice, bob) = std::thread::scope(|scope| {
        let alice = scope.spawn(|| run_session("alice", "only_alice"));
        let bob = scope.spawn(|| run_session("bob", "only_bob"));
        (alice.join().expect("alice"), bob.join().expect("bob"))
    });

    let text = |answers: &[GoalPreflight]| serde_json::to_string(answers).expect("serialise");
    assert!(text(&alice).contains("only_alice") && !text(&alice).contains("only_bob"));
    assert!(text(&bob).contains("only_bob") && !text(&bob).contains("only_alice"));
    assert!(
        alice
            .iter()
            .all(|a| a.candidate.handle == alice[0].candidate.handle)
    );
    assert!(
        bob.iter()
            .all(|b| b.candidate.handle == bob[0].candidate.handle)
    );
    assert_ne!(alice[0].candidate.handle, bob[0].candidate.handle);
    assert_eq!(alice[0].base.revision, bob[0].base.revision);
    assert_eq!(
        tree(cache.path()),
        store_before,
        "neither session left anything behind"
    );
}

#[test]
fn the_same_edits_from_two_sessions_name_two_candidates() {
    let repo = standard();
    let edits = [("a.rs", Some("pub fn alpha() { second(); }\n"))];

    let one = preflight(repo.path(), None, &session_input("one", &edits)).expect("one");
    let two = preflight(repo.path(), None, &session_input("two", &edits)).expect("two");

    assert_ne!(one.candidate.handle, two.candidate.handle);
    assert_eq!(one.candidate.content_digest, two.candidate.content_digest);
    assert_eq!(one.delta.added.len(), two.delta.added.len());
}

#[test]
fn the_goal_reads_its_arguments_from_the_wire_and_defaults_the_base_to_head() {
    let repo = standard();
    let environment = GoalEnvironment::new(repo.path(), None);

    let answer = run(
        &environment,
        &serde_json::json!({ "edits": { "a.rs": "pub fn alpha() { second(); }\n", "b.rs": null } }),
    )
    .expect("run");

    assert_eq!(answer["goal"], "preflight");
    assert_eq!(answer["base"]["requested_ref"], "HEAD");
    assert_eq!(answer["cost"]["changed_paths"], 2);
    assert!(render(&answer).contains("nothing written"));
}

#[test]
fn input_that_cannot_be_laid_over_the_base_is_refused() {
    let repo = standard();
    let refuse = |input: PreflightInput| preflight(repo.path(), None, &input).expect_err("refused");

    assert!(refuse(input(&[])).contains("at least one edit"));
    assert!(refuse(input(&[("../outside.rs", Some("x"))])).contains("unsafe path"));
    assert!(refuse(input(&[("/etc/passwd", Some("x"))])).contains("unsafe path"));
    assert!(refuse(input(&[("nope.rs", None)])).contains("does not track"));
    let mut unknown = input(&[("a.rs", Some("x"))]);
    unknown.base_ref = "no-such-ref".to_string();
    assert!(refuse(unknown).contains("cannot resolve revision"));
}
