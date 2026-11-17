//! The `preflight` goal over the real MCP wire.
//!
//! What a client sees: the tool is listed, a call lays edits over `HEAD` and
//! answers with the facts delta and its cost, and the workspace is untouched.
//! Two parallel sessions with different edits over the same base never see
//! each other's facts or candidate handle.

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

fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("repository");
    let root = directory.path();
    git(root, &["init", "--quiet"]);
    git(root, &["config", "user.name", "CogniCode tests"]);
    git(root, &["config", "user.email", "tests@example.invalid"]);
    std::fs::write(root.join("a.rs"), "pub fn alpha() { first(); }\n").expect("a.rs");
    git(root, &["add", "-A"]);
    git(root, &["commit", "--quiet", "-m", "base"]);
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
            json!({"name": "preflight", "arguments": arguments}),
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
    assert!(tools.iter().any(|name| name == "preflight"), "{tools:?}");
}

#[tokio::test]
async fn edits_over_head_answer_with_a_delta_and_a_cost_and_write_nothing() {
    let repo = repository();

    let answer = call(
        repo.path(),
        json!({"edits": {"a.rs": "pub fn alpha() { second(); }\n", "c.rs": "pub fn gamma() {}\n"}}),
    )
    .await;

    assert_eq!(answer["goal"], "preflight");
    assert_eq!(answer["cost"]["changed_paths"], 2);
    assert_eq!(answer["persisted"], false);
    assert!(answer["delta"]["facts_added"].as_u64().unwrap_or(0) > 0);
    assert_eq!(
        std::fs::read_to_string(repo.path().join("a.rs")).expect("a.rs"),
        "pub fn alpha() { first(); }\n"
    );
    assert!(!repo.path().join("c.rs").exists());
}

#[tokio::test]
async fn two_sessions_with_different_edits_over_one_base_never_see_each_other() {
    let repo = repository();

    let (alice, bob) = tokio::join!(
        call(
            repo.path(),
            json!({"session": "alice", "edits": {"a.rs": "pub fn only_alice() {}\n"}})
        ),
        call(
            repo.path(),
            json!({"session": "bob", "edits": {"a.rs": "pub fn only_bob() {}\n"}})
        ),
    );

    let (alice_text, bob_text) = (alice.to_string(), bob.to_string());
    assert!(alice_text.contains("only_alice") && !alice_text.contains("only_bob"));
    assert!(bob_text.contains("only_bob") && !bob_text.contains("only_alice"));
    assert_ne!(alice["candidate"]["handle"], bob["candidate"]["handle"]);
    assert_eq!(alice["base"]["revision"], bob["base"]["revision"]);
}
