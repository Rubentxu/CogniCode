//! The `advisories` goal over the real MCP wire.
//!
//! What a client sees: the tool is listed, a call correlates the lockfile of
//! `HEAD` with a feed file of the workspace and answers with the feed digest,
//! the match and what was and was not considered, and a call without a feed
//! never answers clean.

use crate::common::McpSession;
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) {
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
}

const MANIFEST: &str =
    "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n";

const FEED: &str = r#"[{"id":"RUSTSEC-2026-0100","modified":"2026-11-01T00:00:00Z",
  "affected":[{"package":{"ecosystem":"crates.io","name":"serde"},
    "ranges":[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.0.150"}]}]}]}]"#;

fn lock() -> String {
    format!(
        "version = 3\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\ndependencies = [\"serde\"]\n\n[[package]]\nname = \"serde\"\nversion = \"1.0.100\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"{}\"\n",
        "a".repeat(64)
    )
}

fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("repository");
    let root = directory.path();
    git(root, &["init", "--quiet"]);
    git(root, &["config", "user.name", "CogniCode tests"]);
    git(root, &["config", "user.email", "tests@example.invalid"]);
    std::fs::write(root.join("Cargo.toml"), MANIFEST).expect("manifest");
    std::fs::write(root.join("Cargo.lock"), lock()).expect("lock");
    git(root, &["add", "-A"]);
    git(root, &["commit", "--quiet", "-m", "base"]);
    std::fs::write(root.join("feed.json"), FEED).expect("feed");
    directory
}

async fn call(workspace: &Path, arguments: Value) -> Value {
    let cache = tempfile::tempdir().expect("cache");
    let mut session = McpSession::spawn_with_env(workspace, &[("XDG_CACHE_HOME", cache.path())])
        .await
        .expect("spawn");
    let response = session
        .request(
            "tools/call",
            json!({"name": "advisories", "arguments": arguments}),
        )
        .await
        .expect("tools/call");
    session.shutdown().await;
    assert_eq!(
        response["result"]["isError"].as_bool(),
        Some(false),
        "the call is not a failure: {response}"
    );
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    serde_json::from_str(text).expect("the answer is JSON")
}

#[tokio::test]
async fn the_tool_is_listed_in_the_full_surface() {
    let scratch = tempfile::tempdir().expect("scratch");
    let mut session = McpSession::spawn_with_flags(scratch.path(), &["--profile", "full"])
        .await
        .expect("spawn");
    let tools = session.list_tools().await.expect("tools/list");
    session.shutdown().await;
    assert!(tools.iter().any(|name| name == "advisories"), "{tools:?}");
}

#[tokio::test]
async fn a_call_correlates_the_lockfile_with_the_feed_and_names_what_it_considered() {
    let repo = repository();

    let answer = call(repo.path(), json!({"feed": "feed.json"})).await;

    assert_eq!(answer["goal"], "advisories");
    assert_eq!(answer["evaluation"], "evaluated");
    let serde = &answer["units"][0]["packages"][1];
    assert_eq!(serde["purl"], "pkg:cargo/serde@1.0.100");
    assert_eq!(serde["state"], "affected");
    assert_eq!(serde["advisories"][0]["id"], "RUSTSEC-2026-0100");
    assert_eq!(serde["advisories"][0]["fixed_in"], json!(["1.0.150"]));
    assert_eq!(
        answer["coverage"]["ecosystems_considered"],
        json!(["crates.io"])
    );
    assert_eq!(answer["coverage"]["advisories_considered"], 1);
}

#[tokio::test]
async fn a_call_without_a_feed_is_not_evaluated_and_never_clean() {
    let repo = repository();

    let answer = call(repo.path(), json!({})).await;

    assert_eq!(answer["evaluation"], "not_evaluated");
    assert_eq!(answer["feed"]["status"], "absent");
    assert_eq!(answer["totals"]["not_found_in_scope"], 0);
}
