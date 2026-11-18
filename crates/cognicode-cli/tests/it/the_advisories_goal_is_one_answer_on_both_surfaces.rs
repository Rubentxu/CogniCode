//! The `advisories` goal on the CLI and on the MCP.
//!
//! The goal is declared once and both surfaces run the same function, so what
//! this file measures is what a user gets: the real `cognicode` binary run as
//! a sub-process, and the MCP dispatch fed the same repository, the same
//! pinned feed and the same question.
//!
//! * The two surfaces answer with the same document, anchored to the feed
//!   digest and to the lockfile digest.
//! * A feed that is not the pinned one is discarded on both surfaces, and
//!   nothing is reported clean.
//! * Asking leaves the workspace tree and Git exactly as they were.

use crate::common;

use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_mcp::rmcp_adapter::call_tool_handler_with_window;
use rmcp::model::CallToolRequestParams;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

const WORKSPACE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../sandbox/fixtures/dependency-inventory/cargo-workspace"
);
const FEED: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../sandbox/fixtures/advisory-matching/osv-feed.json"
);

fn git(root: &Path, args: &[&str]) -> String {
    let none = root.join("no-git-configuration");
    let output = Command::new("git")
        .env("GIT_CONFIG_GLOBAL", &none)
        .env("GIT_CONFIG_SYSTEM", &none)
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git is installed");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8")
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("create directory");
    for entry in std::fs::read_dir(from).expect("read fixture") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("copy file");
        }
    }
}

/// The fixture workspace committed once, with the feed beside it.
fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("repository");
    let root = directory.path();
    git(root, &["init", "--quiet"]);
    git(root, &["config", "user.name", "CogniCode tests"]);
    git(root, &["config", "user.email", "tests@example.invalid"]);
    copy_tree(Path::new(WORKSPACE), root);
    git(root, &["add", "-A"]);
    git(root, &["commit", "--quiet", "-m", "base"]);
    std::fs::copy(FEED, root.join("osv-feed.json")).expect("feed");
    directory
}

fn feed_digest() -> String {
    format!("{:x}", Sha256::digest(std::fs::read(FEED).expect("feed")))
}

fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else {
                let name = path.strip_prefix(root).expect("under root");
                files.insert(
                    name.to_string_lossy().into_owned(),
                    std::fs::read(&path).expect("read"),
                );
            }
        }
    }
    files
}

fn question(digest: &str) -> Value {
    json!({ "feed": "osv-feed.json", "expected_digest": format!("sha256:{digest}") })
}

fn cli_answer(workspace: &Path, question: &Value) -> Value {
    let cache = common::isolated_cache();
    let output = Command::new(common::binary_path("cognicode"))
        .current_dir(workspace)
        .args([
            "advisories",
            "--format",
            "json",
            "--feed",
            question["feed"].as_str().expect("feed"),
            "--expected-digest",
            question["expected_digest"].as_str().expect("digest"),
        ])
        .env("XDG_CACHE_HOME", cache.path())
        .output()
        .expect("the CLI starts");
    assert!(
        output.status.success(),
        "the CLI exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value =
        serde_json::from_slice(&output.stdout).expect("the CLI answers with JSON");
    envelope["result"].clone()
}

async fn mcp_answer(workspace: &Path, question: &Value) -> Value {
    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.to_path_buf())
        .build();
    let request = CallToolRequestParams::new("advisories".to_string())
        .with_arguments(question.as_object().cloned().unwrap_or_default());
    let text = call_tool_handler_with_window(&ctx, request, None)
        .await
        .expect("the MCP answers");
    serde_json::from_str(&text).expect("the MCP answers with JSON")
}

fn state_of<'a>(answer: &'a Value, name: &str, version: &str) -> &'a Value {
    answer["units"][0]["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .find(|package| package["name"] == name && package["version"] == version)
        .unwrap_or_else(|| panic!("{name} {version} is resolved"))
}

#[tokio::test]
async fn the_cli_and_the_mcp_answer_with_the_same_document() {
    let repo = repository();
    let ask = question(&feed_digest());

    let from_cli = cli_answer(repo.path(), &ask);
    let from_mcp = mcp_answer(repo.path(), &ask).await;

    assert_eq!(from_cli, from_mcp, "the two surfaces publish one answer");
    assert_eq!(from_cli["goal"], "advisories");
    assert_eq!(from_cli["feed"]["digest"], json!(feed_digest()));
    assert_eq!(from_cli["feed"]["pinned"], json!(true));
    let unit = &from_cli["units"][0];
    assert_eq!(unit["feed_digest"], json!(feed_digest()));
    assert!(
        unit["lockfile"]["digest"]
            .as_str()
            .is_some_and(|d| d.len() == 64),
        "the answer names the lockfile bytes it was computed against"
    );
    assert!(
        unit["resolved_digest"]
            .as_str()
            .is_some_and(|d| !d.is_empty())
    );
}

#[tokio::test]
async fn the_fixture_feed_decides_each_boundary() {
    let repo = repository();

    let answer = cli_answer(repo.path(), &question(&feed_digest()));

    let hit = state_of(&answer, "bitflags", "1.3.2");
    assert_eq!(hit["state"], "affected");
    assert_eq!(hit["advisories"][0]["id"], "RUSTSEC-2026-0001");
    assert_eq!(
        state_of(&answer, "bitflags", "2.13.2")["state"],
        "not_found_in_scope"
    );
    assert_eq!(
        state_of(&answer, "memchr", "2.8.3")["state"],
        "not_found_in_scope",
        "`fixed` is exclusive"
    );
    assert_eq!(
        state_of(&answer, "smallvec", "1.16.5")["state"],
        "affected",
        "`last_affected` is inclusive"
    );
    assert_eq!(
        state_of(&answer, "itoa", "1.0.19")["state"],
        "not_found_in_scope",
        "a withdrawn advisory does not match"
    );
    assert_eq!(
        state_of(&answer, "app", "0.3.0")["state"],
        "not_evaluated",
        "a workspace member is not a crates.io package"
    );
}

#[tokio::test]
async fn a_feed_that_is_not_the_pinned_one_is_discarded_on_both_surfaces() {
    let repo = repository();
    let ask = question(&"0".repeat(64));

    let from_cli = cli_answer(repo.path(), &ask);
    let from_mcp = mcp_answer(repo.path(), &ask).await;

    assert_eq!(from_cli, from_mcp);
    assert_eq!(from_cli["feed"]["status"], "digest_mismatch");
    assert_eq!(from_cli["evaluation"], "not_evaluated");
    assert_eq!(from_cli["totals"]["not_found_in_scope"], 0);
    assert_eq!(from_cli["totals"]["affected"], 0);
}

#[tokio::test]
async fn asking_from_either_surface_leaves_the_workspace_as_it_was() {
    let repo = repository();
    let ask = question(&feed_digest());
    let tree_before = tree(repo.path());
    let git_before = git(
        repo.path(),
        &["status", "--porcelain=v1", "--untracked-files=all"],
    );
    let head_before = git(repo.path(), &["rev-parse", "HEAD"]);

    cli_answer(repo.path(), &ask);
    mcp_answer(repo.path(), &ask).await;

    assert_eq!(tree(repo.path()), tree_before);
    assert_eq!(
        git(
            repo.path(),
            &["status", "--porcelain=v1", "--untracked-files=all"]
        ),
        git_before
    );
    assert_eq!(git(repo.path(), &["rev-parse", "HEAD"]), head_before);
}
