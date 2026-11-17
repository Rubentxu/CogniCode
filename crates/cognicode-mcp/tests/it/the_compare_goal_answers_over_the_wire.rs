//! The `compare` goal over the real MCP wire.
//!
//! The goal is implemented once and the CLI and the MCP share it. What this
//! file pins is what an MCP client sees:
//!
//! 1. **A success is data, and only data, on stdout.** Every line the server
//!    writes parses as JSON-RPC; the harness fails on any prose line.
//! 2. **The answer names its evidence**: both revisions, the producer, the
//!    coverage, and why impact and reuse were not evaluated.
//! 3. **A comparison that cannot be made is a failure** (`isError: true`),
//!    never an empty comparison that reads as "nothing changed".
//! 4. **The dispatch deadline can cut the goal.** The goal does blocking Git
//!    work; running it on the async worker made the deadline unobservable.

use crate::common::McpSession;
use cognicode_core::interface::mcp::error::{ErrorKind, InterfaceError};
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_mcp::rmcp_adapter::call_tool_handler_with_window;
use rmcp::model::CallToolRequestParams;
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

fn git(root: &Path, args: &[&str]) -> String {
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
    String::from_utf8(output.stdout).expect("git prints UTF-8")
}

/// A repository with two commits that change `src.rs`.
fn repository_with_two_commits() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().expect("repository");
    let root = directory.path();
    git(root, &["init", "--quiet"]);
    git(root, &["config", "user.name", "CogniCode tests"]);
    git(root, &["config", "user.email", "tests@example.invalid"]);
    std::fs::write(root.join("src.rs"), "fn first() {}\n").expect("base source");
    git(root, &["add", "src.rs"]);
    git(root, &["commit", "--quiet", "-m", "base"]);
    let base = git(root, &["rev-parse", "HEAD"]).trim().to_string();
    std::fs::write(root.join("src.rs"), "fn second() {}\n").expect("candidate source");
    git(root, &["add", "src.rs"]);
    git(root, &["commit", "--quiet", "-m", "candidate"]);
    (directory, base)
}

async fn compare_over_the_wire(workspace: &Path, base: &str, head: &str) -> Value {
    let mut session = McpSession::spawn(workspace).await.expect("spawn");
    let response = session
        .request(
            "tools/call",
            json!({"name": "compare", "arguments": {"base_ref": base, "head_ref": head}}),
        )
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

#[tokio::test]
async fn a_comparison_answers_with_its_evidence_and_nothing_else_on_stdout() {
    let (repository, base) = repository_with_two_commits();

    let response = compare_over_the_wire(repository.path(), &base, "HEAD").await;

    assert_eq!(
        response["result"]["isError"].as_bool(),
        Some(false),
        "a comparison that could be made is not a failure: {response}"
    );
    let answer: Value = serde_json::from_str(&text(&response)).expect("the answer is JSON");
    assert_eq!(answer["producer"], "git");
    assert_eq!(answer["base"]["requested_ref"], base.as_str());
    assert_eq!(answer["candidate"]["requested_ref"], "HEAD");
    assert_ne!(answer["base"]["revision"], answer["candidate"]["revision"]);
    assert_eq!(answer["coverage"]["changed_paths"], 1);
    assert_eq!(answer["changes"][0]["path"], "src.rs");
    assert_eq!(answer["changes"][0]["kind"], "modified");
    // The impact is computed from produced revision facts; what could not be
    // evaluated (reuse, with no stored read-sets) is said, with its reason.
    assert_eq!(answer["impact_status"], "evaluated");
    assert_eq!(answer["work_reuse_status"], "not_evaluated");
    assert!(
        !answer["work_reuse_reason"]
            .as_str()
            .unwrap_or_default()
            .is_empty(),
        "an unevaluated reuse says why: {answer}"
    );
}

#[tokio::test]
async fn a_workspace_that_is_not_a_repository_answers_as_a_failure() {
    let workspace = tempfile::tempdir().expect("workspace");

    let response = compare_over_the_wire(workspace.path(), "HEAD~1", "HEAD").await;

    assert_eq!(
        response["result"]["isError"].as_bool(),
        Some(true),
        "no repository means no comparison, not an empty one: {response}"
    );
    assert!(
        text(&response).contains("cannot"),
        "the failure says what could not be done: {}",
        text(&response)
    );
}

#[tokio::test]
async fn a_revision_that_does_not_exist_answers_as_a_failure() {
    let (repository, base) = repository_with_two_commits();

    let response = compare_over_the_wire(repository.path(), &base, "no-such-ref").await;

    assert_eq!(
        response["result"]["isError"].as_bool(),
        Some(true),
        "an unknown revision is not an empty comparison: {response}"
    );
    assert!(
        text(&response).contains("cannot resolve candidate revision"),
        "the failure names the revision that failed: {}",
        text(&response)
    );
}

#[tokio::test]
async fn two_unrelated_histories_answer_as_a_failure() {
    let (repository, base) = repository_with_two_commits();
    let root = repository.path();
    git(root, &["checkout", "--quiet", "--orphan", "island"]);
    git(root, &["rm", "-rf", "--quiet", "."]);
    std::fs::write(root.join("island.rs"), "fn island() {}\n").expect("island source");
    git(root, &["add", "island.rs"]);
    git(root, &["commit", "--quiet", "-m", "island"]);

    let response = compare_over_the_wire(root, &base, "island").await;

    assert_eq!(
        response["result"]["isError"].as_bool(),
        Some(true),
        "histories without a common ancestor are not comparable as a diff: {response}"
    );
}

/// The dispatch deadline cuts a `compare` call.
///
/// The goal runs blocking Git work. On the async worker the deadline cannot
/// be observed, so the call answered late, with its result, as if the window
/// had been respected.
#[tokio::test]
async fn the_dispatch_deadline_cuts_a_comparison() {
    let (repository, base) = repository_with_two_commits();
    let ctx = HandlerContext::builder()
        .with_working_dir(repository.path().to_path_buf())
        .build();
    let request = CallToolRequestParams::new("compare".to_string()).with_arguments(
        json!({"base_ref": base, "head_ref": "HEAD"})
            .as_object()
            .cloned()
            .unwrap_or_default(),
    );

    let outcome: Result<String, InterfaceError> =
        call_tool_handler_with_window(&ctx, request, Some(Duration::from_nanos(1))).await;

    let error = outcome.expect_err("the deadline cuts a comparison that outlives it");
    assert_eq!(
        error.kind(),
        ErrorKind::Timeout,
        "the call was cut by the deadline and the answer says another thing: {error}"
    );
}
