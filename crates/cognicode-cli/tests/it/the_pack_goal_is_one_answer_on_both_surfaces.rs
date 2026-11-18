//! The `pack` goal on the CLI and on the MCP.
//!
//! The goal is declared once and both surfaces run the same function, so what
//! this file measures is what a user gets: the real `cognicode` binary run as
//! a sub-process, and the MCP dispatch fed the same workspace and the same
//! arguments.
//!
//! * The two surfaces answer with the same document, by id and by intent.
//! * A homonym is never resolved by its bare name, on either surface.
//! * The budget is verified and never claims an exact count without a
//!   configured tokenizer.
//! * An empty request is refused on both surfaces.

use crate::common;

use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_mcp::rmcp_adapter::call_tool_handler_with_window;
use rmcp::model::CallToolRequestParams;
use serde_json::{Value, json};
use std::path::Path;
use std::process::Command;

fn workspace() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("workspace");
    let src = directory.path().join("src");
    std::fs::create_dir_all(&src).expect("src");
    std::fs::write(
        src.join("a.rs"),
        "pub fn parse(input: &str) -> u32 {\n    input.len() as u32\n}\n\npub fn run() -> u32 {\n    parse(\"x\")\n}\n",
    )
    .expect("a.rs");
    std::fs::write(
        src.join("b.rs"),
        "pub fn parse(input: &[u8]) -> u32 {\n    input.len() as u32\n}\n",
    )
    .expect("b.rs");
    directory
}

/// The arguments as terminal flags: a list is a repeated flag.
fn flags_of(arguments: &Value) -> Vec<String> {
    let mut flags = Vec::new();
    for (name, value) in arguments.as_object().expect("an object") {
        let flag = format!("--{}", name.replace('_', "-"));
        let values = match value {
            Value::Array(items) => items.iter().map(plain).collect(),
            other => vec![plain(other)],
        };
        for text in values {
            flags.push(flag.clone());
            flags.push(text);
        }
    }
    flags
}

fn plain(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

fn cli_envelope(workspace: &Path, arguments: &Value) -> Value {
    let cache = common::isolated_cache();
    let output = Command::new(common::binary_path("cognicode"))
        .current_dir(workspace)
        .args(["pack", "--format", "json"])
        .args(flags_of(arguments))
        .env("XDG_CACHE_HOME", cache.path())
        .output()
        .expect("the CLI starts");
    assert!(
        output.status.success(),
        "the CLI exited with {}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("the CLI answers with JSON")
}

async fn mcp_answer(workspace: &Path, arguments: &Value) -> Value {
    let ctx = HandlerContext::builder()
        .with_working_dir(workspace.to_path_buf())
        .build();
    let request = CallToolRequestParams::new("pack".to_string())
        .with_arguments(arguments.as_object().cloned().unwrap_or_default());
    let text = call_tool_handler_with_window(&ctx, request, None)
        .await
        .expect("the MCP answers");
    serde_json::from_str(&text).expect("the MCP answers with JSON")
}

fn candidates_of(answer: &Value, name: &str) -> Vec<String> {
    answer["ambiguities"]
        .as_array()
        .expect("ambiguities")
        .iter()
        .find(|ambiguity| ambiguity["query"] == name)
        .and_then(|ambiguity| ambiguity["candidates"].as_array())
        .map(|ids| {
            ids.iter()
                .map(|id| id.as_str().expect("an id").to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn a_homonym_is_ambiguous_on_both_surfaces_and_its_id_answers_the_same() {
    let repo = workspace();

    let by_name = json!({"symbols": ["parse"]});
    let cli = cli_envelope(repo.path(), &by_name)["result"].clone();
    let mcp = mcp_answer(repo.path(), &by_name).await;
    assert_eq!(cli, mcp, "the two surfaces publish one answer");
    assert!(cli["items"].as_array().expect("items").is_empty());
    let ids = candidates_of(&cli, "parse");
    assert_eq!(ids.len(), 2, "both homonyms are named: {cli}");

    let by_id = json!({"symbols": [ids[0]]});
    let cli = cli_envelope(repo.path(), &by_id)["result"].clone();
    let mcp = mcp_answer(repo.path(), &by_id).await;
    assert_eq!(cli, mcp, "the two surfaces publish one answer");
    assert_eq!(cli["items"][0]["id"], ids[0]);
    assert_eq!(cli["items"][0]["reason"], "anchor");
    assert!(
        cli["items"][0]["source_digest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("sha256:"))
    );
    assert_ne!(cli["status"], "complete");
}

#[tokio::test]
async fn a_budget_is_declared_verified_and_never_exact_without_a_tokenizer() {
    let repo = workspace();

    let arguments = json!({"intent": "parse run", "max_tokens": 600});
    let first = cli_envelope(repo.path(), &arguments);
    let answer = &first["result"];
    assert_eq!(answer, &mcp_answer(repo.path(), &arguments).await);

    assert!(
        answer["items"]
            .as_array()
            .is_some_and(|items| items.len() == 3),
        "a phrase finds the symbols its words name: {answer}"
    );
    let budget = &answer["budget"];
    assert_eq!(budget["limit"], 600);
    assert_ne!(
        budget["used"]["kind"], "exact",
        "no tokenizer, no exact count"
    );
    assert_eq!(budget["verified"], true);
    assert_eq!(answer["semantic_coverage"]["assessed"], false);
}

#[tokio::test]
async fn an_empty_request_is_refused_on_both_surfaces() {
    let repo = workspace();

    let cache = common::isolated_cache();
    let output = Command::new(common::binary_path("cognicode"))
        .current_dir(repo.path())
        .args(["pack", "--format", "json"])
        .env("XDG_CACHE_HOME", cache.path())
        .output()
        .expect("the CLI starts");
    assert!(!output.status.success(), "the CLI refuses an empty request");

    let ctx = HandlerContext::builder()
        .with_working_dir(repo.path().to_path_buf())
        .build();
    let refused =
        call_tool_handler_with_window(&ctx, CallToolRequestParams::new("pack".to_string()), None)
            .await;
    assert!(refused.is_err(), "the MCP refuses an empty request");
}
