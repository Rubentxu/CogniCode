//! The goal `pack` over the change of a real Git repository.

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};

use super::*;
use crate::application::context_pack::TokenMeasure;

const LIB: &str = "pub fn parse(input: &str) -> u32 {\n    input.len() as u32\n}\n\nfn helper(x: u32) -> u32 {\n    x + 1\n}\n\npub fn run() -> u32 {\n    parse(\"x\") + helper(1)\n}\n\n#[test]\nfn parse_counts_bytes() {\n    let n = parse(\"ab\");\n    assert_eq!(n, 2);\n}\n";
/// A fixture whose test reaches the changed symbol through another symbol:
/// the path the pack publishes has more than one hop.
const DEEP: &str = "pub fn parse(input: &str) -> u32 {\n    input.len() as u32\n}\n\nfn helper(x: u32) -> u32 {\n    parse(\"x\") + x\n}\n\n#[test]\nfn parse_counts_bytes_through_helper() {\n    let n = helper(1);\n    assert_eq!(n, 2);\n}\n";
const OTHER: &str = "pub fn unrelated() -> u32 {\n    1\n}\n";

fn git(repo: &Path, args: &[&str]) {
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
        .expect("git is available");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write(repo: &Path, relative: &str, text: &str) {
    let path = repo.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q", "-b", "main"]);
    write(repo, "src/lib.rs", LIB);
    write(repo, "src/other.rs", OTHER);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "base"]);
    dir
}

fn run_pack(root: &Path, arguments: Value) -> Result<Value, String> {
    run(&GoalEnvironment::new(root, None), &arguments)
}

fn ids(answer: &Value) -> Vec<String> {
    answer["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap().to_string())
        .collect()
}

fn names(answer: &Value) -> Vec<String> {
    ids(answer)
        .iter()
        .map(|id| id.split(':').nth(1).unwrap().to_string())
        .collect()
}

fn reason_of<'a>(answer: &'a Value, name: &str) -> &'a str {
    answer["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == name)
        .unwrap_or_else(|| panic!("`{name}` is not in {answer}"))["reason"]
        .as_str()
        .unwrap()
}

fn detail_of<'a>(answer: &'a Value, name: &str) -> &'a str {
    answer["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == name)
        .unwrap_or_else(|| panic!("`{name}` is not in {answer}"))["reason_detail"]
        .as_str()
        .unwrap()
}

/// The names of the symbols of a path the pack published.
fn hops_of(detail: &str) -> Vec<String> {
    detail
        .split(" through ")
        .nth(1)
        .unwrap_or_else(|| panic!("the detail names no path: {detail}"))
        .split(" -> ")
        .map(|hop| hop.split(':').nth(1).unwrap_or(hop).to_string())
        .collect()
}

fn gap_kinds(answer: &Value) -> Vec<String> {
    answer["change"]["gaps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|gap| gap["kind"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn staged_and_working_tree_show_exactly_the_scope_asked() {
    let dir = project();
    let repo = dir.path();
    write(
        repo,
        "src/other.rs",
        "pub fn unrelated() -> u32 {\n    2\n}\n",
    );
    git(repo, &["add", "src/other.rs"]);
    write(repo, "src/lib.rs", &LIB.replace("x + 1", "x + 2"));

    let staged = run_pack(repo, json!({"change_scope": "staged"})).unwrap();
    assert_eq!(names(&staged), ["unrelated"], "staged: {staged}");
    assert_eq!(staged["change"]["head"], "index");

    let tree = run_pack(repo, json!({"change_scope": "working_tree"})).unwrap();
    let tree_names = names(&tree);
    assert!(tree_names.contains(&"unrelated".to_string()), "{tree}");
    assert!(tree_names.contains(&"helper".to_string()), "{tree}");
    assert_eq!(tree["change"]["head"], "working_tree");
}

#[test]
fn a_commit_does_not_inherit_the_changes_of_the_working_tree() {
    let dir = project();
    let repo = dir.path();
    write(
        repo,
        "src/other.rs",
        "pub fn unrelated() -> u32 {\n    2\n}\n",
    );
    git(repo, &["commit", "-q", "-am", "change other"]);
    write(repo, "src/lib.rs", &LIB.replace("x + 1", "x + 2"));

    let commit = run_pack(repo, json!({"change_scope": "commit", "head": "HEAD"})).unwrap();
    assert_eq!(names(&commit), ["unrelated"], "{commit}");
    let files = commit["change"]["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["path"], "src/other.rs");
    assert_eq!(files[0]["state"], "mapped");
}

#[test]
fn a_file_that_moved_on_since_the_commit_is_unmapped_never_mixed() {
    let dir = project();
    let repo = dir.path();
    write(repo, "src/lib.rs", &LIB.replace("x + 1", "x + 2"));
    git(repo, &["commit", "-q", "-am", "change helper"]);
    write(repo, "src/lib.rs", &LIB.replace("x + 1", "x + 3"));

    let commit = run_pack(repo, json!({"change_scope": "commit", "head": "HEAD"})).unwrap();
    assert!(ids(&commit).is_empty(), "{commit}");
    let file = &commit["change"]["files"][0];
    assert_eq!(file["state"], "unmapped");
    assert_eq!(file["reason"], "snapshot_differs");
    assert!(gap_kinds(&commit).contains(&"unmapped_file".to_string()));
    assert_eq!(commit["status"], "partial");
}

#[test]
fn a_signature_change_selects_consumers_and_tests_and_a_private_body_change_does_not() {
    let dir = project();
    let repo = dir.path();
    write(
        repo,
        "src/lib.rs",
        &LIB.replace(
            "fn parse(input: &str) -> u32",
            "fn parse(input: &str) -> u64",
        ),
    );
    let signature = run_pack(repo, json!({"change_scope": "working_tree"})).unwrap();
    assert_eq!(reason_of(&signature, "parse"), "changed_symbol");
    assert_eq!(reason_of(&signature, "run"), "consumer_of_change");
    let detail = signature["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == "run")
        .unwrap()["reason_detail"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(detail.contains("parse"), "the edge is named: {detail}");
    assert!(
        names(&signature).contains(&"parse_counts_bytes".to_string()),
        "{signature}"
    );
    assert!(!names(&signature).contains(&"helper".to_string()));

    git(repo, &["checkout", "-q", "--", "."]);
    write(repo, "src/lib.rs", &LIB.replace("x + 1", "x + 2"));
    let private = run_pack(repo, json!({"change_scope": "working_tree"})).unwrap();
    assert_eq!(names(&private), ["helper"], "{private}");
    assert!(
        gap_kinds(&private).contains(&"no_test_found".to_string()),
        "missing coverage is a gap, not safety: {private}"
    );
    assert_eq!(private["status"], "partial");
}

#[test]
fn the_path_of_the_test_that_reaches_the_change_is_the_path_of_the_graph() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, &["init", "-q", "-b", "main"]);
    write(repo, "src/lib.rs", DEEP);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "base"]);

    write(
        repo,
        "src/lib.rs",
        &DEEP.replace(
            "fn parse(input: &str) -> u32",
            "fn parse(input: &str) -> u64",
        ),
    );

    let answer = run_pack(repo, json!({"change_scope": "working_tree"})).unwrap();

    assert_eq!(
        reason_of(&answer, "parse_counts_bytes_through_helper"),
        "test_of_change"
    );
    assert_eq!(
        hops_of(detail_of(&answer, "parse_counts_bytes_through_helper")),
        ["parse_counts_bytes_through_helper", "helper", "parse"],
        "the published path is the one the graph holds"
    );
}

#[test]
fn a_conflicted_merge_candidate_is_unknown_and_never_complete() {
    let dir = project();
    let repo = dir.path();
    git(repo, &["checkout", "-q", "-b", "topic"]);
    write(repo, "src/lib.rs", &LIB.replace("x + 1", "x + 5"));
    git(repo, &["commit", "-q", "-am", "topic"]);
    git(repo, &["checkout", "-q", "main"]);
    write(repo, "src/lib.rs", &LIB.replace("x + 1", "x + 9"));
    git(repo, &["commit", "-q", "-am", "main"]);

    let answer = run_pack(
        repo,
        json!({"change_scope": "merge_candidate", "base": "main", "head": "topic"}),
    )
    .unwrap();
    assert!(ids(&answer).is_empty(), "{answer}");
    let file = &answer["change"]["files"][0];
    assert_eq!(file["state"], "unmapped");
    assert_eq!(file["reason"], "unknown_change");
    assert!(gap_kinds(&answer).contains(&"unmapped_file".to_string()));
    assert_eq!(answer["status"], "partial");
}

#[test]
fn a_clean_tree_is_an_empty_change_and_still_not_complete() {
    let dir = project();
    let answer = run_pack(dir.path(), json!({"change_scope": "working_tree"})).unwrap();
    assert!(ids(&answer).is_empty());
    assert_eq!(answer["change"]["files_total"], 0);
    assert_ne!(answer["status"], "complete");
    assert_eq!(answer["semantic_coverage"]["assessed"], false);
}

#[test]
fn a_continuation_token_is_bound_to_its_change() {
    let dir = project();
    let repo = dir.path();
    write(
        repo,
        "src/lib.rs",
        &LIB.replace(
            "fn parse(input: &str) -> u32",
            "fn parse(input: &str) -> u64",
        ),
    );
    let (limit, token) = (60..800)
        .step_by(20)
        .find_map(|limit| {
            let page = run_pack(
                repo,
                json!({"change_scope": "working_tree", "max_tokens": limit}),
            )
            .ok()?;
            let token = page["continuation_token"].as_str()?.to_string();
            Some((limit, token))
        })
        .expect("some budget cuts the pack and gives a token");

    let same = run_pack(
        repo,
        json!({"change_scope": "working_tree", "max_tokens": limit, "continuation_token": token}),
    )
    .unwrap();
    assert!(same["items"].as_array().is_some());

    write(
        repo,
        "src/other.rs",
        "pub fn unrelated() -> u32 {\n    7\n}\n",
    );
    let moved = run_pack(
        repo,
        json!({"change_scope": "working_tree", "max_tokens": limit, "continuation_token": token}),
    );
    assert!(
        moved.unwrap_err().contains("another snapshot"),
        "a page is never built over another change"
    );
}

#[test]
fn the_text_of_a_change_pack_is_what_the_budget_measured() {
    let dir = project();
    let repo = dir.path();
    write(repo, "src/lib.rs", &LIB.replace("x + 1", "x + 2"));
    let answer = run_pack(repo, json!({"change_scope": "working_tree"})).unwrap();
    let text = render(&answer);
    assert!(text.contains("change working_tree"), "{text}");
    assert!(text.contains("advisory:"), "{text}");
    assert_eq!(
        answer["budget"]["used"]["tokens"].as_u64().unwrap(),
        Utf8UpperBound.measure(&text).tokens
    );
}

#[test]
fn a_change_scope_takes_no_symbols_and_a_reference_needs_a_scope() {
    let dir = project();
    let repo = dir.path();
    assert!(
        run_pack(
            repo,
            json!({"change_scope": "staged", "symbols": ["parse"]})
        )
        .is_err()
    );
    assert!(run_pack(repo, json!({"change_scope": "staged", "intent": "parse"})).is_err());
    assert!(run_pack(repo, json!({"base": "main"})).is_err());
    assert!(run_pack(repo, json!({"change_scope": "range", "base": "main"})).is_err());
    assert!(run_pack(repo, json!({"change_scope": "everything"})).is_err());
}

#[test]
fn a_change_that_cannot_be_read_is_an_error_not_an_empty_pack() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "src/lib.rs", LIB);
    let error = run_pack(dir.path(), json!({"change_scope": "staged"})).unwrap_err();
    assert!(error.contains("not available"), "{error}");
}
