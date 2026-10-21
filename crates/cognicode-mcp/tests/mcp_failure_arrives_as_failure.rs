//! A tool that cannot do its job says so over the wire.
//!
//! The graph analytics handlers answered a missing graph with a
//! successful result carrying an `error` key inside. Over MCP that
//! result arrives as `isError: false`: the transport cannot tell it
//! apart from a result that carries data, so the caller read "it
//! worked" where the tool had failed.
//!
//! What this file pins is the shape the consumer sees. A tool that
//! cannot run answers with `isError: true` inside a successful call:
//! the call itself was well-formed, and the tool could not do its job.
//! The failure says what happened, and it never arrives as a success.
//!
//! RED→GREEN: before the fix the answer to `graph_pagerank` without a
//! graph was `isError: false` with the error inside the payload.

mod common;

use common::McpSession;
use serde_json::{Value, json};

/// The `isError` marker of a tool answer, if it carries one.
fn is_error(response: &Value) -> Option<bool> {
    response["result"]["isError"].as_bool()
}

/// The text the tool answered with.
fn text(response: &Value) -> String {
    response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[tokio::test]
async fn a_graph_tool_without_a_graph_answers_as_a_failure() {
    let workspace = tempfile::tempdir().expect("tempdir");
    let mut session = McpSession::spawn(workspace.path()).await.expect("spawn");

    let response = session
        .request(
            "tools/call",
            json!({
                "name": "graph_pagerank",
                "arguments": {"alpha": 0.85, "max_iterations": 100}
            }),
        )
        .await
        .expect("tools/call");

    assert_eq!(
        is_error(&response),
        Some(true),
        "a tool that cannot run must not answer as a success: {response}"
    );
    assert!(
        text(&response).to_lowercase().contains("graph"),
        "the failure says what happened: {}",
        text(&response)
    );

    session.shutdown().await;
}

#[tokio::test]
async fn a_graph_tool_with_a_graph_answers_as_a_success() {
    let workspace = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        workspace.path().join("lib.rs"),
        "pub fn helper() -> u32 { 7 }\npub fn caller() -> u32 { helper() }\n",
    )
    .expect("the fixture is written");

    let mut session = McpSession::spawn(workspace.path()).await.expect("spawn");

    let built = session
        .request(
            "tools/call",
            json!({"name": "build_graph", "arguments": {}}),
        )
        .await
        .expect("tools/call");
    assert_ne!(
        is_error(&built),
        Some(true),
        "the graph must build in a workspace with source: {built}"
    );

    let response = session
        .request(
            "tools/call",
            json!({
                "name": "graph_pagerank",
                "arguments": {"alpha": 0.85, "max_iterations": 100}
            }),
        )
        .await
        .expect("tools/call");

    assert_ne!(
        is_error(&response),
        Some(true),
        "a tool that did its job must not answer as a failure: {response}"
    );
    assert!(
        text(&response).contains("scores"),
        "the answer carries the data the tool was asked for: {}",
        text(&response)
    );

    session.shutdown().await;
}
