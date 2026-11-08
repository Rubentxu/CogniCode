//! The product's answer to «¿por qué se reporta este hallazgo?».
//!
//! A finding that arrives without a reason is an accusation, and a consumer
//! that cannot ask "why?" has no way to tell a real problem from a
//! misfiring detector. This suite drives the published tool over the wire,
//! through the real binary, and pins three things:
//!
//! 1. the answer names the finding and carries the grounding behind it;
//! 2. the outcomes distinguish "reported and explained" from "reported
//!    nothing" and from "not the one you asked about" — an empty list is
//!    never the whole answer;
//! 3. a path that leaves the workspace is refused rather than read.
//!
//! The corpus is the one the findings pipeline is measured on: two function
//! definitions, both grounded, both reportable. A finding the pipeline
//! cannot ground would be a different assertion, and it is not the one
//! this file makes.

use std::path::PathBuf;

use serde_json::{Value, json};

use crate::common;
use common::McpSession;

/// The source file the findings pipeline is measured on.
fn corpus_fixture() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures/lsi-grounding/sample.rs");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("the corpus fixture must be readable: {e}"))
}

/// The catalogue the operator maintains by hand.
fn tool_document() -> Value {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../cognicode-core/product/tools.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("tools.json must be readable: {e}"));
    serde_json::from_str(&text).expect("tools.json must be valid JSON")
}

/// Ask the tool about one file.
///
/// A tool that cannot do its job answers with `isError`, and that answer is
/// an error, not a payload: the two are not the same thing and folding them
/// together would let a refusal pass as an explanation.
async fn explain(session: &mut McpSession, path: &str) -> Result<Value, String> {
    let response = session
        .request(
            "tools/call",
            json!({
                "name": "explain_finding",
                "arguments": {"path": path},
            }),
        )
        .await?;
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    if response["result"]["isError"].as_bool().unwrap_or(false) {
        return Err(text);
    }
    serde_json::from_str(&text).map_err(|e| format!("the answer is not JSON ({e}): {text}"))
}

/// A workspace with the corpus file in it.
fn workspace_with_the_corpus() -> tempfile::TempDir {
    let workspace = tempfile::tempdir().expect("tempdir");
    std::fs::write(workspace.path().join("sample.rs"), corpus_fixture())
        .expect("the corpus fixture is written");
    workspace
}

#[tokio::test]
async fn the_answer_names_the_finding_and_carries_its_grounding() {
    let workspace = workspace_with_the_corpus();
    // TASK-84: la superficie se pide explicitamente. La del servidor por
    // defecto anuncia los goals, y esta suite audita una tool.
    let mut session = McpSession::spawn_with_flags(workspace.path(), &["--profile", "full"])
        .await
        .expect("spawn");

    let answer = explain(&mut session, "sample.rs")
        .await
        .expect("the tool answers");

    assert_eq!(
        answer["outcome"], "explained",
        "a file with findings is explained, not reported as empty: {answer}"
    );
    let findings = answer["findings"]
        .as_array()
        .expect("the answer carries an array of findings");
    assert!(
        !findings.is_empty(),
        "the corpus carries two function definitions and both are reported: {answer}"
    );

    for finding in findings {
        assert!(
            !finding["id"].as_str().unwrap_or_default().is_empty(),
            "a finding without an id cannot be asked about again: {finding}"
        );
        assert!(
            !finding["message"].as_str().unwrap_or_default().is_empty(),
            "a finding without a message is an accusation, not an explanation: {finding}"
        );
        assert!(
            !finding["kind"].as_str().unwrap_or_default().is_empty(),
            "the answer names what was found: {finding}"
        );
        assert!(
            !finding["causal_chain"]
                .as_array()
                .unwrap_or(&Vec::new())
                .is_empty(),
            "the answer carries the chain that produced the finding: {finding}"
        );
        assert!(
            !finding["detector"]["id"]
                .as_str()
                .unwrap_or_default()
                .is_empty(),
            "the answer names the detector that produced the finding: {finding}"
        );

        // The grounding is the point of the tool: a finding whose evidence
        // cannot be resolved says so, and one whose evidence is resolved
        // says that too.
        let grounding = &finding["grounding"];
        assert_eq!(
            grounding["verified"], true,
            "the corpus findings are grounded, so the answer says so: {finding}"
        );
        assert!(
            !grounding["evidence"]
                .as_array()
                .unwrap_or(&Vec::new())
                .is_empty(),
            "a verified grounding names the evidence it rests on: {finding}"
        );
        assert!(
            !grounding["facts"]
                .as_array()
                .unwrap_or(&Vec::new())
                .is_empty(),
            "a verified grounding names the canonical facts it rests on: {finding}"
        );
        assert_eq!(
            finding["gateable"], true,
            "a grounded finding under this gate may block: {finding}"
        );
    }

    // The coverage says what the run looked at, so a consumer can tell the
    // difference between "nothing was found" and "nothing was there".
    assert_eq!(
        answer["coverage"]["constructs_observed"], 2,
        "the corpus carries two function definitions: {answer}"
    );
    assert_eq!(
        answer["coverage"]["constructs_grounded"], 2,
        "both constructs are grounded in the corpus: {answer}"
    );
    assert_eq!(
        answer["coverage"]["constructs_ungrounded"], 0,
        "no construct is left ungrounded in the corpus: {answer}"
    );

    session.shutdown().await;
}

#[tokio::test]
async fn asking_for_one_finding_returns_that_finding() {
    let workspace = workspace_with_the_corpus();
    // TASK-84: la superficie se pide explicitamente. La del servidor por
    // defecto anuncia los goals, y esta suite audita una tool.
    let mut session = McpSession::spawn_with_flags(workspace.path(), &["--profile", "full"])
        .await
        .expect("spawn");

    let answer = explain(&mut session, "sample.rs")
        .await
        .expect("the tool answers");
    let wanted = answer["findings"][0]["id"]
        .as_str()
        .expect("a finding id")
        .to_string();

    let one = session
        .request(
            "tools/call",
            json!({
                "name": "explain_finding",
                "arguments": {"path": "sample.rs", "finding_id": wanted},
            }),
        )
        .await
        .expect("tools/call");
    let text = one["result"]["content"][0]["text"].as_str().expect("text");
    let payload: Value = serde_json::from_str(text).expect("the answer is JSON");

    assert_eq!(
        payload["outcome"], "explained",
        "the finding that was asked about is explained: {payload}"
    );
    let findings = payload["findings"].as_array().expect("findings array");
    assert_eq!(
        findings.len(),
        1,
        "asking for one finding returns that finding and no other: {payload}"
    );
    assert_eq!(
        findings[0]["id"], wanted,
        "the answer is about the finding that was asked about: {payload}"
    );

    session.shutdown().await;
}

#[tokio::test]
async fn a_finding_that_was_not_reported_says_so() {
    let workspace = workspace_with_the_corpus();
    // TASK-84: la superficie se pide explicitamente. La del servidor por
    // defecto anuncia los goals, y esta suite audita una tool.
    let mut session = McpSession::spawn_with_flags(workspace.path(), &["--profile", "full"])
        .await
        .expect("spawn");

    let response = session
        .request(
            "tools/call",
            json!({
                "name": "explain_finding",
                "arguments": {"path": "sample.rs", "finding_id": "not-a-finding-id"},
            }),
        )
        .await
        .expect("tools/call");
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("text");
    let payload: Value = serde_json::from_str(text).expect("the answer is JSON");

    assert_eq!(
        payload["outcome"], "not_reported",
        "a finding the pipeline did not report is not explained: {payload}"
    );
    assert!(
        payload["findings"]
            .as_array()
            .unwrap_or(&Vec::new())
            .is_empty(),
        "the answer carries no finding it did not report: {payload}"
    );
    assert!(
        !payload["summary"].as_str().unwrap_or_default().is_empty(),
        "the answer says what happened instead of returning nothing: {payload}"
    );

    session.shutdown().await;
}

#[tokio::test]
async fn a_file_without_findings_says_so() {
    let workspace = tempfile::tempdir().expect("tempdir");
    std::fs::write(workspace.path().join("empty.rs"), "// nothing here\n")
        .expect("the fixture is written");
    // TASK-84: la superficie se pide explicitamente. La del servidor por
    // defecto anuncia los goals, y esta suite audita una tool.
    let mut session = McpSession::spawn_with_flags(workspace.path(), &["--profile", "full"])
        .await
        .expect("spawn");

    let answer = explain(&mut session, "empty.rs")
        .await
        .expect("the tool answers");

    assert_eq!(
        answer["outcome"], "no_findings",
        "a file with no findings says so instead of answering with an empty list: {answer}"
    );
    assert!(
        answer["findings"]
            .as_array()
            .unwrap_or(&Vec::new())
            .is_empty(),
        "there is nothing to explain: {answer}"
    );
    assert!(
        !answer["summary"].as_str().unwrap_or_default().is_empty(),
        "the answer says what happened: {answer}"
    );

    session.shutdown().await;
}

#[tokio::test]
async fn a_path_outside_the_workspace_is_refused() {
    let workspace = workspace_with_the_corpus();
    // TASK-84: la superficie se pide explicitamente. La del servidor por
    // defecto anuncia los goals, y esta suite audita una tool.
    let mut session = McpSession::spawn_with_flags(workspace.path(), &["--profile", "full"])
        .await
        .expect("spawn");

    let refusal = explain(&mut session, "../outside.rs").await;

    let message = match &refusal {
        Err(message) => message.clone(),
        Ok(answer) => panic!("a path that leaves the workspace is refused, not read: {answer}"),
    };
    assert!(
        message.to_lowercase().contains("traversal"),
        "the refusal says what was wrong: {message}"
    );

    session.shutdown().await;
}

#[tokio::test]
async fn the_tool_is_advertised_with_its_output_contract() {
    let workspace = tempfile::tempdir().expect("tempdir");
    // TASK-84: la superficie se pide explicitamente. La del servidor por
    // defecto anuncia los goals, y esta suite audita una tool.
    let mut session = McpSession::spawn_with_flags(workspace.path(), &["--profile", "full"])
        .await
        .expect("spawn");

    // The catalogue is paginated, so a single page is a partial view of the
    // surface — and a partial view looks exactly like a tool that does not
    // exist.
    let mut tools: Vec<Value> = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..64 {
        let params = match &cursor {
            Some(c) => json!({"cursor": c}),
            None => json!({}),
        };
        let response = session
            .request("tools/list", params)
            .await
            .expect("tools/list");
        let result = &response["result"];
        if let Some(page) = result["tools"].as_array() {
            tools.extend(page.iter().cloned());
        }
        match result["nextCursor"].as_str() {
            Some(next) if !next.is_empty() => cursor = Some(next.to_string()),
            _ => break,
        }
    }

    let tool = tools
        .iter()
        .find(|t| t["name"] == "explain_finding")
        .unwrap_or_else(|| panic!("explain_finding must be advertised"));

    let advertised = tool
        .get("outputSchema")
        .expect("the tool advertises a contract");
    assert!(
        !advertised.is_null(),
        "the tool answers with a typed payload, so it advertises its contract"
    );
    assert_eq!(
        advertised.get("type").and_then(|t| t.as_str()),
        Some("object"),
        "the contract describes an object: {advertised}"
    );

    session.shutdown().await;
}

#[test]
fn the_catalogue_publishes_the_derived_contract() {
    // The catalogue is the document an adopter reads; the wire is what the
    // server sends. A catalogue entry that describes a shape the runtime
    // never sends is a promise nobody keeps, and the two are compared here
    // byte for byte so they cannot drift apart unnoticed.
    let published = tool_document()["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .find(|t| t["name"] == "explain_finding")
        .unwrap_or_else(|| panic!("the catalogue must publish explain_finding"))
        .get("output_schema")
        .cloned()
        .expect("the catalogue publishes the tool's contract");

    let derived =
        cognicode_core::interface::mcp::output_contracts::output_schema_for("explain_finding")
            .expect("the runtime derives the tool's contract");

    assert_eq!(
        published,
        serde_json::to_value(derived).expect("the derived contract is JSON"),
        "the catalogue must publish the contract the runtime derives, not a \
         hand-written copy of it"
    );
}
