//! Regression guard: every advertised tool must be callable.
//!
//! # The defect this pins
//!
//! `composite_impact_query` (live at `34038ffe`) was advertised in
//! `product/tools.json` with a published output schema, but had no arm in
//! `call_tool_handler`. A real MCP client that discovered it through `tools/list`
//! and then called it received `isError=true` with `tool not found`. `cogh`, a
//! different ghost, was removed earlier in the same cycle for the same reason.
//!
//! # Why the existing guard missed it
//!
//! `mcp_roundtrip_tests::test_all_listed_tools_are_dispatchable` compares
//! `tools/list` against `dispatchable_tool_names()` — a second, hand-maintained
//! array. Hand-maintained lists drift together: adding a name to the catalog
//! *and* to the array satisfies the guard while the runtime still cannot route
//! the name. The guard proves the two documents agree; it never asks the
//! dispatcher.
//!
//! # What this guard does instead
//!
//! It calls every advertised tool through the real dispatch path and only
//! fails on the one failure that means "no arm exists": the dispatcher's
//! `ToolNotFound` fallthrough. A tool that has an arm but rejects empty
//! arguments (`missing field`) or lacks a precondition (`No graph available`)
//! counts as callable, because both are honest answers from a tool that
//! exists. Anything else is left visible in an `UNKNOWN` bucket rather than
//! being waved through, so a new failure mode cannot hide inside "fine".

use cognicode_core::interface::mcp::error::InterfaceError;
use cognicode_core::interface::mcp::rmcp_adapter;

/// Outcome of calling one advertised tool with empty arguments.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// The dispatcher has an arm and the call reached it.
    Dispatched,
    /// No arm: the name fell through to `ToolNotFound`.
    NoArm,
    /// The server returned an error that is not "tool not found".
    Other(String),
    /// The call panicked, timed out, or the process died.
    Fatal(String),
}

/// Probe the dispatcher directly.
///
/// `call_tool_handler` is private to the module, so this exercises it through
/// the same re-export the `Server` uses. If that surface ever disappears the
/// test fails to compile, which is the correct outcome: the guard would
/// otherwise silently stop testing anything.
async fn probe(
    name: &str,
    ctx: &cognicode_core::interface::mcp::handlers::HandlerContext,
) -> Outcome {
    match rmcp_adapter::dispatch_tool_for_contract_test(ctx, name, serde_json::json!({})).await {
        Ok(_) => Outcome::Dispatched,
        Err(InterfaceError::ToolNotFound(_)) => Outcome::NoArm,
        Err(other) => Outcome::Other(format!("{other}")),
    }
}

fn make_ctx(dir: &tempfile::TempDir) -> cognicode_core::interface::mcp::handlers::HandlerContext {
    cognicode_core::interface::mcp::handlers::HandlerContext::builder()
        .with_working_dir(dir.path().to_path_buf())
        .build()
}

#[tokio::test]
async fn no_advertised_tool_is_a_ghost() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("src")).expect("src dir");
    std::fs::write(
        dir.path().join("src/lib.rs"),
        "fn alpha() -> i32 {\n    42\n}\n",
    )
    .expect("write lib.rs");

    let ctx = make_ctx(&dir);
    let advertised: Vec<String> = rmcp_adapter::build_all_tools()
        .iter()
        .map(|t| t.name.as_ref().to_string())
        .collect();

    assert!(
        !advertised.is_empty(),
        "build_all_tools() returned nothing; the guard would pass vacuously"
    );

    let mut ghosts: Vec<String> = Vec::new();
    let mut unknown: Vec<String> = Vec::new();

    for name in &advertised {
        match probe(name, &ctx).await {
            Outcome::NoArm => ghosts.push(name.clone()),
            Outcome::Other(msg) => {
                // Empty arguments are a valid probe for "does an arm exist",
                // but the error must be the *tool's* error, not the
                // dispatcher's. Anything we cannot recognise is surfaced
                // rather than assumed benign.
                if msg.contains("missing field")
                    || msg.contains("invalid input")
                    || msg.contains("No graph available")
                    || msg.contains("not initialized")
                    || msg.contains("no index")
                {
                    // Expected: the arm ran and rejected the empty call.
                } else {
                    unknown.push(format!("{name}: {msg}"));
                }
            }
            Outcome::Fatal(msg) => unknown.push(format!("{name}: FATAL {msg}")),
            Outcome::Dispatched => {}
        }
    }

    assert!(
        ghosts.is_empty(),
        "tools/list advertises {} tool(s) with no dispatch arm: {ghosts:?}.\n\
         A client that calls one gets isError=true / 'tool not found'. Either \
         add the arm or remove the entry from product/tools.json.",
        ghosts.len()
    );

    assert!(
        unknown.is_empty(),
        "{} advertised tool(s) failed in a way this guard does not recognise. \
         If these are honest rejections, teach `probe` to classify them; if they \
         are defects, fix them. Do not widen the classifier to make this pass.\n{:#?}",
        unknown.len(),
        unknown
    );
}

#[tokio::test]
async fn composite_impact_query_answers_instead_of_reporting_tool_not_found() {
    // The specific regression, named separately so the failure message points
    // at the tool rather than at "some ghost somewhere".
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = make_ctx(&dir);

    let result = rmcp_adapter::dispatch_tool_for_contract_test(
        &ctx,
        "composite_impact_query",
        serde_json::json!({ "qualified_name": "alpha" }),
    )
    .await;

    let payload = match result {
        Ok(json) => json,
        Err(InterfaceError::ToolNotFound(_)) => {
            panic!("composite_impact_query has no dispatch arm")
        }
        Err(e) => panic!("composite_impact_query failed with a real error: {e}"),
    };

    // The load-bearing B4.W5 invariant must survive the projection. A result
    // that omits the refusal reads as an all-clear.
    assert_eq!(
        payload.get("verdict_refused"),
        Some(&serde_json::Value::Bool(true)),
        "composite_impact_query must always refuse to conclude"
    );
    assert_eq!(
        payload
            .get("verdict_refused_reason")
            .and_then(|v| v.as_str()),
        Some(cognicode_core::impact::composite::COMPOSITE_VERDICT_REFUSED_REASON),
        "the refusal reason is the canonical, statically anchored string"
    );
}
