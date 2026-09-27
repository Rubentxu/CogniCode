//! A-012 / CP2.3 — structured output for the `reviewer` profile.
//!
//! A-009 taught the `reviewer` profile which tools it may call. A-012 answers
//! the question a client asks *after* calling one: "what will I get back?".
//!
//! Until this change, `0 de 73` tools published an `output_schema`. That is a
//! defensible answer for a tool whose result is free-form prose, and an
//! indefensible one for a tool that returns a struct the runtime already
//! owns. A client had to guess, and a wrong guess looked exactly like a
//! runtime bug.
//!
//! The fix derives the schema from the very type that serialises the
//! result, so the published contract cannot describe a shape the handler
//! does not produce. The tests below pin three things a derive alone does
//! not: that the contract reaches `tools/list`, that the catalogue and the
//! derived schema are the same object, and that the real response bytes
//! satisfy the schema the client was told to expect.
//!
//! RED→GREEN: before this change `tools/list` carried no `outputSchema` at
//! all, so the black-box assertions below could not even be written.

use cognicode_core::interface::mcp::output_contracts::{
    output_schema_for, published_output_schemas, tools_with_output_schema,
};
use cognicode_core::interface::mcp::rmcp_adapter::build_all_tools;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

mod common;
use common::binary_path;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn tool_document() -> Value {
    let path = repo_root().join("product/tools.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("tools.json must be readable: {e}"));
    serde_json::from_str(&text).expect("tools.json must be valid JSON")
}

// --- Contract level: every derived schema is a real, usable promise ---

/// Every published contract must describe an object with properties.
///
/// A schema with no properties would be technically present and practically
/// worthless, and would let the count stay green while the feature rots.
#[test]
fn every_published_contract_describes_an_object() {
    let tools = tools_with_output_schema();
    assert!(
        tools.len() >= 10,
        "only {} tools publish an output contract; the work was quietly undone",
        tools.len()
    );
    for tool in tools {
        let schema = output_schema_for(tool).expect("derived contract");
        assert_eq!(
            schema.get("type").and_then(|t| t.as_str()),
            Some("object"),
            "{tool} does not describe an object"
        );
        let properties = schema
            .get("properties")
            .and_then(|p| p.as_object())
            .unwrap_or_else(|| panic!("{tool} has no properties: it promises nothing"));
        assert!(
            !properties.is_empty(),
            "{tool} has an empty properties map: it promises nothing"
        );
    }
}

/// No contract may carry a dialect declaration.
///
/// MCP expects a bare schema object. A `$schema` key would make a client
/// pick a validator dialect the server never intended.
#[test]
fn no_contract_carries_a_dialect_declaration() {
    for (tool, schema) in published_output_schemas() {
        assert!(
            !schema.contains_key("$schema"),
            "{tool} carries a $schema key; MCP expects a bare schema object"
        );
    }
}

// --- Runtime level: the catalog must actually carry the contract ---

/// The schema the catalog advertises must be the derived one, byte for byte.
///
/// This is the assertion that keeps a single source of truth. If someone
/// hand-writes a schema into the catalog, or a derive starts disagreeing
/// with the published copy, this test fails instead of the drift shipping.
#[test]
fn advertised_contract_is_identical_to_the_derived_contract() {
    let tools = build_all_tools();
    let mut advertised = 0usize;
    for tool in &tools {
        if let Some(schema) = tool.output_schema.as_ref() {
            let derived = output_schema_for(tool.name.as_ref()).unwrap_or_else(|| {
                panic!("{} advertises a contract with no derived source", tool.name)
            });
            let advertised_value = serde_json::to_value(schema.as_ref()).expect("schema is JSON");
            assert_eq!(
                Value::Object(derived),
                advertised_value,
                "{} advertises a schema that differs from the derived contract",
                tool.name
            );
            advertised += 1;
        }
    }
    assert_eq!(
        advertised,
        tools_with_output_schema().len(),
        "the catalog advertises {advertised} contracts but {} are derived",
        tools_with_output_schema().len()
    );
}

/// A derived contract that never reaches the catalog is a lie of omission.
///
/// The client is told nothing about that tool, so the work "done" is invisible
/// from the outside. Counting the reachable contracts keeps the two honest.
#[test]
fn every_derived_contract_reaches_the_catalog() {
    let names: Vec<String> = build_all_tools()
        .iter()
        .map(|t| t.name.to_string())
        .collect();
    for tool in tools_with_output_schema() {
        assert!(
            names.iter().any(|n| n == tool),
            "{tool} has a derived output contract but is absent from the tool catalog"
        );
    }
}

/// The `reviewer` posture must not degrade the contract.
///
/// A read-only client still deserves a checkable output schema for the tools
/// it is allowed to call. If the contracts only existed for mutating tools,
/// the `reviewer` profile — the whole point of A-009 — would gain nothing.
#[test]
fn read_only_tools_carry_their_contracts() {
    let mut read_contracts = 0usize;
    for tool in build_all_tools() {
        let authority = tool
            .meta
            .as_ref()
            .and_then(|m| m.get("cognicode"))
            .and_then(|m| m.get("authority"))
            .and_then(|v| v.as_str())
            .unwrap_or("read");
        if authority == "read" && tool.output_schema.is_some() {
            read_contracts += 1;
        }
    }
    assert!(
        read_contracts >= 10,
        "only {read_contracts} read-only tools advertise an output contract"
    );
}

// --- Black-box level: the wire must agree with the contract ---

fn spawn_server(read_only: bool) -> Child {
    let mut cmd = Command::new(binary_path());
    // The MCP binary is subcommand-free: its only input flag is `--cwd`, and
    // the read-only posture is selected the same way A-010 selects it. Using
    // a fixture workspace keeps the tool calls hermetic and fast.
    cmd.arg("--cwd")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_03_ws"));
    if read_only {
        cmd.arg("--read-only");
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    cmd.spawn().expect("mcp server must start")
}

/// Sends the requests and returns the responses that carry an `id`.
///
/// Keyed by filtering, not by position: a notification produces no response,
/// so counting lines and indexing would silently shift every answer by one.
/// A server that leaks non-JSON on stdout is a contract violation worth
/// failing on, so nothing is skipped silently.
async fn rpc(mut child: Child, requests: &[&str]) -> Vec<Value> {
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));
    let mut stdin = child.stdin.take().expect("stdin");
    let mut out = Vec::new();
    for request in requests {
        stdin
            .write_all(format!("{request}\n").as_bytes())
            .await
            .expect("write request");
        stdin.flush().await.expect("flush");
        // Notifications get no reply, so read only while one is outstanding.
        if !request.contains("\"id\"") {
            continue;
        }
        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(60), stdout.read_line(&mut line))
            .await
            .expect("response within timeout")
            .expect("read response");
        let line = line.trim();
        assert!(!line.is_empty(), "server closed stdout without replying");
        let value: Value = serde_json::from_str(line)
            .unwrap_or_else(|e| panic!("response must be JSON ({e}): {line}"));
        assert!(
            value.get("result").is_some() || value.get("error").is_some(),
            "response is neither result nor error: {value}"
        );
        out.push(value);
    }
    let _ = child.kill().await;
    out
}

/// The `result` payload for the request with this id.
fn result_of(responses: &[Value], id: u64) -> &Value {
    responses
        .iter()
        .find(|r| r["id"] == id)
        .map(|r| &r["result"])
        .unwrap_or_else(|| panic!("no response for id {id}"))
}

const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"a012","version":"0.0.0"}}}"#;
const INITIALIZED: &str = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;

/// Every tool the server will advertise, following the pagination cursor.
///
/// `tools/list` serves 20 tools per page, so a single call under
/// `--read-only` would report one page's worth of contracts and quietly hide
/// the rest. Paging is therefore not optional here: an un-paged read of this
/// API is a partial view of the contract, and a partial view looks exactly
/// like a small feature.
async fn all_listed_tools(read_only: bool) -> Vec<Value> {
    let mut collected: Vec<Value> = Vec::new();
    let mut cursor: Option<String> = None;
    let mut id = 2u64;
    loop {
        let list = match &cursor {
            Some(c) => format!(
                r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/list","params":{{"cursor":{c:?}}}}}"#
            ),
            None => format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"tools/list","params":{{}}}}"#),
        };
        let responses = rpc(spawn_server(read_only), &[INIT, INITIALIZED, &list]).await;
        let result = result_of(&responses, id);
        let tools = result["tools"].as_array().expect("tools array");
        let returned = tools.len();
        collected.extend(tools.iter().cloned());
        match result.get("nextCursor").and_then(|c| c.as_str()) {
            Some(next) if !next.is_empty() => cursor = Some(next.to_string()),
            _ => break,
        }
        id += 1;
        assert!(returned > 0, "cursor advanced but a page came back empty");
    }
    collected
}

/// Under `--read-only`, `tools/list` must carry the output contracts.
///
/// This is the assertion that closes the gap for the `reviewer` profile: a
/// read-only client can now check what it gets back, not just what it may
/// call. It runs against the real binary because the contract has to
/// survive serialisation, not merely exist in a Rust struct.
#[tokio::test]
async fn read_only_tools_list_carries_output_schemas() {
    let tools = all_listed_tools(true).await;
    let with_schema: Vec<&str> = tools
        .iter()
        .filter(|t| advertised_schema(t).is_some())
        .map(|t| t["name"].as_str().expect("tool name"))
        .collect();
    assert!(
        with_schema.len() >= 10,
        "only {} tools advertise an outputSchema over the wire: {with_schema:?}",
        with_schema.len()
    );
    // Every advertised schema must be a usable object contract, not a stub.
    for tool in &tools {
        if let Some(schema) = advertised_schema(tool) {
            assert_eq!(
                schema.get("type").and_then(|t| t.as_str()),
                Some("object"),
                "{} advertises a non-object outputSchema",
                tool["name"]
            );
        }
    }
}

/// The wire schema must be the derived schema, unchanged.
///
/// Serialisation is where a hand-written schema and a derived one usually
/// part ways. This pins that they do not.
#[tokio::test]
async fn wire_schema_matches_the_derived_schema() {
    let tools = all_listed_tools(true).await;
    let mut checked = 0usize;
    for tool in &tools {
        let Some(schema) = advertised_schema(tool) else {
            continue;
        };
        let name = tool["name"].as_str().expect("tool name");
        let derived = output_schema_for(name)
            .unwrap_or_else(|| panic!("{name} advertises a contract with no derived source"));
        assert_eq!(
            Value::Object(derived),
            schema,
            "{name} advertises a wire schema that differs from the derived contract"
        );
        checked += 1;
    }
    assert!(
        checked >= 10,
        "only {checked} wire schemas were cross-checked"
    );
}

/// A real call must satisfy the schema the client was handed.
///
/// The rest of this suite proves the contract exists. This proves it is
/// *true*: after building the graph, a real `graph_query` returns an object
/// whose shape the advertised schema accepts. A contract the runtime
/// violates is worse than no contract, because the client trusts it.
#[tokio::test]
async fn a_real_response_satisfies_the_advertised_schema() {
    // `graph_query` needs a graph, so the session builds one first. Under
    // `--read-only` that call is still permitted: it derives state inside the
    // workspace and declares no mutating authority. This is the exact sequence
    // a real client follows, which is why it is worth exercising.
    let build = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": { "name": "build_graph", "arguments": {} },
    })
    .to_string();
    // Built as JSON rather than a `format!` template: brace escaping in a
    // hand-rolled MCP payload is a bug factory, and this test has no business
    // being clever about transport.
    let call = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "graph_query",
            "arguments": { "question": "which functions are defined here?" },
        },
    })
    .to_string();

    // One session for both calls: the graph built by the first must be
    // visible to the second, and splitting them would test nothing.
    let responses = rpc(spawn_server(true), &[INIT, INITIALIZED, &build, &call]).await;
    let build_result = result_of(&responses, 3);
    assert!(
        !build_result["isError"].as_bool().unwrap_or(false),
        "build_graph must succeed under --read-only: {build_result}"
    );
    let result = result_of(&responses, 4);
    assert!(
        !result["isError"].as_bool().unwrap_or(false),
        "graph_query must succeed under --read-only: {result}"
    );
    let content = result["content"][0]["text"]
        .as_str()
        .expect("text content")
        .to_string();
    let payload: Value =
        serde_json::from_str(&content).unwrap_or_else(|e| panic!("payload is JSON ({e})"));

    let listed = all_listed_tools(true).await;
    let schema = listed
        .iter()
        .find(|t| t["name"] == "graph_query")
        .and_then(advertised_schema)
        .unwrap_or_else(|| panic!("graph_query must advertise a non-null outputSchema"));

    assert_object_contract_holds(&schema, &payload, "graph_query");
}

/// The tool's advertised output schema, or `None` when it advertises none.
///
/// MCP represents "no contract" as an absent or null `outputSchema`; both
/// mean the same thing to a client, so both collapse to `None` here.
fn advertised_schema(tool: &Value) -> Option<Value> {
    tool.get("outputSchema").filter(|s| !s.is_null()).cloned()
}

/// Checks the payload against the advertised contract.
///
/// Intentionally narrow: it verifies the object's declared type and that
/// every field the schema *requires* is actually present. A full JSON-Schema
/// implementation would become a second source of truth about the runtime,
/// which is precisely the duplication this work unit removes.
fn assert_object_contract_holds(schema: &Value, payload: &Value, tool: &str) {
    assert_eq!(
        schema.get("type").and_then(|t| t.as_str()),
        Some("object"),
        "{tool} advertises a non-object schema"
    );
    let properties = schema
        .get("properties")
        .and_then(|p| p.as_object())
        .unwrap_or_else(|| panic!("{tool} advertises no properties"));
    for required in schema
        .get("required")
        .and_then(|r| r.as_array())
        .into_iter()
        .flatten()
    {
        let field = required.as_str().expect("required field name");
        assert!(
            payload.get(field).is_some(),
            "{tool} promises `{field}` but the real response omits it"
        );
    }
    for (field, spec) in properties {
        let Some(value) = payload.get(field) else {
            continue;
        };
        let declared = spec.get("type").and_then(|t| t.as_str());
        if let Some(declared) = declared {
            assert!(
                type_matches(declared, value),
                "{tool}.{field} is declared `{declared}` but the response is `{}`",
                json_type(value)
            );
        }
    }
}

fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_f64() => "number",
        Value::Number(_) => "integer",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn type_matches(declared: &str, value: &Value) -> bool {
    let actual = json_type(value);
    if declared == "number" {
        return matches!(actual, "number" | "integer");
    }
    if declared == actual {
        return true;
    }
    // A nullable field is declared as the declared type by schemars; the
    // runtime may legitimately send `null` for an optional value.
    declared == "string" && actual == "null"
}

// --- Catalogue level: `product/tools.json` must tell the same story ---

/// The published catalogue must agree with the runtime about who has a
/// contract. `product/tools.json` is the artefact an adopter reads; if it
/// says `null` for a tool that now answers with a schema, the documentation
/// is stale and A-012 has only half landed.
#[test]
fn published_catalogue_matches_the_runtime_contracts() {
    let runtime: std::collections::BTreeSet<String> = tools_with_output_schema()
        .iter()
        .map(|s| s.to_string())
        .collect();
    let mut mismatches = Vec::new();
    for tool in tool_document()["tools"].as_array().expect("tools array") {
        let name = tool["name"].as_str().expect("tool name");
        let published_has = !tool["output_schema"].is_null();
        if published_has != runtime.contains(name) {
            mismatches.push(format!(
                "{name}: tools.json says {} but runtime says {}",
                if published_has { "schema" } else { "no schema" },
                if runtime.contains(name) {
                    "schema"
                } else {
                    "no schema"
                }
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "product/tools.json has drifted from the runtime contracts:\n{}",
        mismatches.join("\n")
    );
}
