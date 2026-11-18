//! The `pack` goal over the real MCP wire.
//!
//! What a client sees: the tool is listed in the full surface and not in the
//! default one, a call answers with evidence per symbol, a homonym is never
//! resolved by its name, and the answer travels in the envelope.

use crate::common::McpSession;
use serde_json::{Value, json};
use std::path::Path;

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

/// The answer a call returns: the text of the result and the envelope the
/// structured content carries.
async fn call(workspace: &Path, arguments: Value) -> (Value, Value) {
    let cache = tempfile::tempdir().expect("cache");
    let mut session = McpSession::spawn_with_env(workspace, &[("XDG_CACHE_HOME", cache.path())])
        .await
        .expect("spawn");
    let response = session
        .request(
            "tools/call",
            json!({"name": "pack", "arguments": arguments}),
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
    let result = serde_json::from_str(text).expect("the answer is JSON");
    (result, response["result"]["structuredContent"].clone())
}

#[tokio::test]
async fn the_tool_is_listed_in_the_full_surface_and_not_in_the_default_one() {
    let scratch = tempfile::tempdir().expect("scratch");
    let mut full = McpSession::spawn_with_flags(scratch.path(), &["--profile", "full"])
        .await
        .expect("spawn");
    let listed = full.list_tools().await.expect("tools/list");
    full.shutdown().await;
    assert!(listed.iter().any(|name| name == "pack"), "{listed:?}");

    let mut default = McpSession::spawn(scratch.path()).await.expect("spawn");
    let listed = default.list_tools().await.expect("tools/list");
    default.shutdown().await;
    assert!(!listed.iter().any(|name| name == "pack"), "{listed:?}");
}

#[tokio::test]
async fn a_homonym_is_named_and_its_id_answers_with_evidence() {
    let repo = workspace();

    let (ambiguous, envelope) = call(repo.path(), json!({"symbols": ["parse"]})).await;
    let ids: Vec<String> = ambiguous["ambiguities"][0]["candidates"]
        .as_array()
        .expect("the candidates")
        .iter()
        .map(|id| id.as_str().expect("an id").to_string())
        .collect();
    assert_eq!(ids.len(), 2, "{ambiguous}");
    assert!(ambiguous["items"].as_array().expect("items").is_empty());
    let link = envelope["_links"]
        .as_array()
        .and_then(|links| links.iter().find(|link| link["rel"] == "resolve-ambiguity"))
        .expect("the envelope offers the call that resolves the homonym");
    assert_eq!(link["tool"], "pack");
    assert_eq!(link["required_args"]["symbols"], json!([ids[0]]));

    let (exact, envelope) = call(repo.path(), json!({"symbols": [ids[1]]})).await;
    let item = &exact["items"][0];
    assert_eq!(item["id"], ids[1]);
    assert!(
        item["source_digest"]
            .as_str()
            .is_some_and(|digest| digest.starts_with("sha256:"))
    );
    assert_eq!(exact["goal"], "pack");
    assert_eq!(
        envelope["result"], exact,
        "the envelope carries the answer whole"
    );
    assert_ne!(envelope["state"], "complete", "{envelope}");
}

/// A client moves through the levels of knowledge by following the links of
/// the envelope, with the arguments they carry and nothing else: every
/// level the pack offers answers, and the consumers come back by identity.
#[tokio::test]
async fn the_links_of_a_pack_walk_from_the_range_up_to_the_project() {
    let repo = workspace();
    let cache = tempfile::tempdir().expect("cache");
    let mut session = McpSession::spawn_with_env(repo.path(), &[("XDG_CACHE_HOME", cache.path())])
        .await
        .expect("spawn");

    let answer = session
        .request(
            "tools/call",
            json!({"name": "pack", "arguments": {"intent": "parse run", "max_tokens": 4000}}),
        )
        .await
        .expect("pack");
    let envelope = answer["result"]["structuredContent"].clone();
    let items = envelope["result"]["items"].as_array().expect("items");
    let reach = envelope["result"]["navigation"]["reach"]
        .as_array()
        .expect("the pack publishes what expanding each symbol reaches");
    assert_eq!(reach.len(), items.len(), "{envelope}");

    let links: Vec<Value> = envelope["_links"]
        .as_array()
        .expect("links")
        .iter()
        .filter(|link| {
            [
                "read-range",
                "inspect-contract",
                "trace-consumers",
                "inspect-component",
                "inspect-overview",
            ]
            .contains(&link["rel"].as_str().unwrap_or_default())
        })
        .cloned()
        .collect();
    assert!(
        links.iter().any(|link| link["rel"] == "read-range")
            && links.iter().any(|link| link["rel"] == "inspect-contract"),
        "{envelope}"
    );
    assert!(links.iter().all(|link| link["authority"] == "read"));

    for link in &links {
        let tool = link["tool"].as_str().expect("a tool");
        let response = session
            .request(
                "tools/call",
                json!({"name": tool, "arguments": link["required_args"]}),
            )
            .await
            .expect("tools/call");
        assert_eq!(
            response["result"]["isError"].as_bool(),
            Some(false),
            "the link `{}` is an action that answers: {response}",
            link["rel"]
        );
    }
    session.shutdown().await;
}
