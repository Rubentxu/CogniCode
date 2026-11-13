//! A name that two symbols carry reaches the client as data, not as prose.
//!
//! ## The gap
//!
//! The handler layer already refuses an ambiguous name with its own JSON-RPC
//! code (`-32006`) and the candidates as `data`. The route a client really
//! takes goes through `call_tool_handler`, whose error type is
//! `InterfaceError`, and there the ambiguity became `InvalidInput(String)`:
//! the candidates were glued into a sentence and the client had to parse
//! prose to learn which symbols it could ask for instead.
//!
//! ## What this file pins
//!
//! The answer a client reads, through the same two functions the server
//! uses: `call_tool_handler` for the dispatch and `error_result` for the
//! wire. The text block keeps the words it always had (the consumers of this
//! product read it); the code and the candidates are in `structuredContent`.
//!
//! The suite writes its own fixture into a temporary directory: no process,
//! no binary, no network, no clock.

use cognicode_core::interface::mcp::error::InterfaceError;
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_mcp::rmcp_adapter::call_tool_handler;
use cognicode_mcp::tool_response::error_result;
use rmcp::model::CallToolRequestParams;
use serde_json::{Value, json};
use std::path::Path;

/// A context scoped to the workspace.
fn ctx_for(workspace: &Path) -> HandlerContext {
    HandlerContext::builder()
        .with_working_dir(workspace.to_path_buf())
        .build()
}

/// Two files that each declare a `helper` of their own.
fn workspace_with_two_homonyms() -> tempfile::TempDir {
    let workspace = tempfile::tempdir().expect("workspace");
    let src = workspace.path().join("src");
    std::fs::create_dir_all(&src).expect("src dir");
    for (file, called) in [("a.rs", "compute_a"), ("b.rs", "compute_b")] {
        std::fs::write(
            src.join(file),
            format!("fn helper() {{ {called}(); }}\nfn {called}() {{}}\n"),
        )
        .expect("fixture file");
    }
    workspace
}

/// One tool call through the real dispatch.
async fn call(
    ctx: &HandlerContext,
    tool: &str,
    arguments: Value,
) -> Result<String, InterfaceError> {
    let request = CallToolRequestParams::new(tool.to_string())
        .with_arguments(arguments.as_object().cloned().unwrap_or_default());
    call_tool_handler(ctx, request).await
}

/// The refusal a client reads when it traces from `helper` to `helper`.
async fn the_refusal_of_an_ambiguous_trace() -> (InterfaceError, rmcp::model::CallToolResult) {
    let workspace = workspace_with_two_homonyms();
    let ctx = ctx_for(workspace.path());
    call(&ctx, "build_graph", json!({}))
        .await
        .expect("the graph builds");

    let error = call(
        &ctx,
        "trace_path",
        json!({"source": "helper", "target": "helper"}),
    )
    .await
    .expect_err("a name that two symbols carry is not resolved in silence");
    let wire = error_result(&error);
    (error, wire)
}

#[tokio::test]
async fn the_wire_carries_the_code_and_the_candidates_as_data() {
    let (_, wire) = the_refusal_of_an_ambiguous_trace().await;

    assert_eq!(
        wire.is_error,
        Some(true),
        "a refusal is a failure: {wire:?}"
    );
    let structured = wire
        .structured_content
        .as_ref()
        .expect("the client reads the refusal as data, not only as prose");
    assert_eq!(
        structured["error"]["code"], -32006,
        "the ambiguity has its own code: {structured}"
    );
    let data = &structured["error"]["data"];
    assert_eq!(data["name"], "helper", "{structured}");
    let candidates: Vec<&str> = data["candidates"]
        .as_array()
        .expect("the candidates are a list")
        .iter()
        .map(|candidate| candidate.as_str().expect("a candidate is text"))
        .collect();
    assert_eq!(candidates.len(), 2, "{candidates:?}");
    assert!(
        candidates
            .iter()
            .any(|candidate| candidate.contains("a.rs"))
            && candidates
                .iter()
                .any(|candidate| candidate.contains("b.rs")),
        "one candidate per homonym: {candidates:?}"
    );
}

#[tokio::test]
async fn the_text_block_keeps_the_words_the_consumers_read() {
    let (_, wire) = the_refusal_of_an_ambiguous_trace().await;

    let text = wire.content[0]
        .raw
        .as_text()
        .expect("the first block is text")
        .text
        .clone();
    assert!(
        text.starts_with("invalid input: ambiguous symbol 'helper': "),
        "the sentence a client already reads did not change: {text}"
    );
}

#[tokio::test]
async fn a_failure_without_data_adds_nothing_to_the_wire() {
    let wire = error_result(&InterfaceError::InvalidInput("bad".to_string()));

    assert_eq!(wire.is_error, Some(true));
    assert!(
        wire.structured_content.is_none(),
        "only a refusal that carries data publishes it: {wire:?}"
    );
}
