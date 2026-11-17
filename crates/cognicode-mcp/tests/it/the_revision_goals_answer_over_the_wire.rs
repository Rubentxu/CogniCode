//! The revision goals (`inspect --revision` and `verify`) over the real MCP
//! wire.
//!
//! The goals are implemented once and the CLI and the MCP share them. What
//! this file pins is what an MCP client sees:
//!
//! 1. **A success is data, and only data, on stdout.** The harness fails on
//!    any line that is not JSON-RPC.
//! 2. **A snapshot produced by `inspect` is the one `verify` checks**, across
//!    server processes that share a cache.
//! 3. **What cannot be established is not a success**: no stored snapshot is
//!    `not_evaluated`, an unknown revision is `isError`.
//! 4. **The dispatch deadline can cut both goals.**

use crate::common::McpSession;
use cognicode_core::interface::mcp::error::{ErrorKind, InterfaceError};
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_mcp::rmcp_adapter::call_tool_handler_with_window;
use rmcp::model::CallToolRequestParams;
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

fn git(root: &Path, args: &[&str]) {
    let no_configuration = root.join("no-git-configuration");
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", &no_configuration)
        .env("GIT_CONFIG_SYSTEM", &no_configuration)
        .output()
        .expect("git is installed");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("repository");
    let root = directory.path();
    git(root, &["init", "--quiet"]);
    git(root, &["config", "user.name", "CogniCode tests"]);
    git(root, &["config", "user.email", "tests@example.invalid"]);
    std::fs::write(root.join("src.rs"), "fn first() {}\nfn second() {}\n").expect("source");
    git(root, &["add", "src.rs"]);
    git(root, &["commit", "--quiet", "-m", "base"]);
    directory
}

async fn call(workspace: &Path, cache: &Path, tool: &str, arguments: Value) -> Value {
    let mut session = McpSession::spawn_with_cache(workspace, cache)
        .await
        .expect("spawn");
    let response = session
        .request("tools/call", json!({"name": tool, "arguments": arguments}))
        .await
        .expect("tools/call");
    session.shutdown().await;
    response
}

fn text(response: &Value) -> String {
    response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

fn answer(response: &Value) -> Value {
    assert_eq!(
        response["result"]["isError"].as_bool(),
        Some(false),
        "the call is not a failure: {response}"
    );
    serde_json::from_str(&text(response)).expect("the answer is JSON")
}

#[tokio::test]
async fn inspecting_a_revision_persists_the_snapshot_that_verify_then_confirms() {
    let repository = repository();
    let cache = tempfile::tempdir().expect("cache");

    let inspected = answer(
        &call(
            repository.path(),
            cache.path(),
            "inspect",
            json!({"revision": "HEAD"}),
        )
        .await,
    );
    let facts = &inspected["revision_facts"];
    assert_eq!(facts["persisted"], true, "{inspected}");
    assert_eq!(facts["coverage"]["coverage"], "full", "{inspected}");
    assert!(facts["coverage"]["facts"].as_u64().unwrap_or(0) > 0);

    let verified = answer(
        &call(
            repository.path(),
            cache.path(),
            "verify",
            json!({"revision": "HEAD"}),
        )
        .await,
    );
    assert_eq!(verified["status"], "verified", "{verified}");
    assert_eq!(verified["revision"], facts["revision"]);
    assert_eq!(verified["snapshot"]["found"], true);
    assert_eq!(verified["snapshot"]["mismatched_paths"], json!([]));
}

#[tokio::test]
async fn verifying_without_a_stored_snapshot_is_not_evaluated() {
    let repository = repository();
    let cache = tempfile::tempdir().expect("cache");

    let verified = answer(
        &call(
            repository.path(),
            cache.path(),
            "verify",
            json!({"revision": "HEAD"}),
        )
        .await,
    );

    assert_eq!(verified["status"], "not_evaluated", "{verified}");
    assert_eq!(verified["snapshot"]["found"], false);
    assert!(
        !verified["reason"].as_str().unwrap_or_default().is_empty(),
        "an unevaluated verification says why: {verified}"
    );
}

#[tokio::test]
async fn an_unknown_revision_answers_as_a_failure_for_both_goals() {
    let repository = repository();
    let cache = tempfile::tempdir().expect("cache");

    for (tool, arguments) in [
        ("inspect", json!({"revision": "no-such-ref"})),
        ("verify", json!({"revision": "no-such-ref"})),
    ] {
        let response = call(repository.path(), cache.path(), tool, arguments).await;
        assert_eq!(
            response["result"]["isError"].as_bool(),
            Some(true),
            "`{tool}` on an unknown revision is not a verdict: {response}"
        );
    }
}

#[tokio::test]
async fn the_dispatch_deadline_cuts_both_revision_goals() {
    let repository = repository();
    let cache = tempfile::tempdir().expect("cache");
    let ctx = HandlerContext::builder()
        .with_working_dir(repository.path().to_path_buf())
        .with_cache_root(cache.path().to_path_buf())
        .build();

    for (tool, arguments) in [
        ("inspect", json!({"revision": "HEAD"})),
        ("verify", json!({"revision": "HEAD"})),
    ] {
        let request = CallToolRequestParams::new(tool.to_string())
            .with_arguments(arguments.as_object().cloned().unwrap_or_default());

        let outcome: Result<String, InterfaceError> =
            call_tool_handler_with_window(&ctx, request, Some(Duration::from_nanos(1))).await;

        let error = outcome.expect_err("the deadline cuts a goal that outlives it");
        assert_eq!(
            error.kind(),
            ErrorKind::Timeout,
            "`{tool}` was cut by the deadline and the answer says another thing: {error}"
        );
    }
}
