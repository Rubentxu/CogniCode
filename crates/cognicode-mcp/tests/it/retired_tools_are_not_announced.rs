//! TASK-64 — the MCP surface advertises only what it serves.
//!
//! Two tools were published in `product/tools.json` as `stable` while
//! the code behind them computed nothing:
//!
//! - `graph_analyze` answered every mode with the same canned
//!   `{"type": ..., "nodes": <symbol count>}`, and its payload even
//!   claimed the SCC had been computed with `petgraph::algo::tarjan_scc`.
//!   The three algorithms it names are computed by `graph_condensed`,
//!   `graph_reduced` and `graph_feedback_arcs`.
//! - `composite_impact_query` promised an evidence bundle — `impact`,
//!   `architecture`, `findings` — that it could never deliver, and the
//!   follow-ups it suggested were looked up by comparing a *symbol* name
//!   against a *tool* name, so the lookup could not match a symbol. The
//!   impact of a symbol is computed by `analyze_impact`.
//!
//! The server also announced the `resources` capability while
//! implementing none of it, and three modules carried a whole
//! implementation of capabilities that nobody ever called.
//!
//! What this file pins:
//!
//! - The retired names are not advertised: not in the catalogue, not in
//!   `tools/list`, not in the runtime capture.
//! - The retired names still answer, served by the tool that computes,
//!   and the answer says which tool served it.
//!
//! The announcement of capabilities is pinned in the crate's own tests
//! (`interface::mcp::rmcp_adapter`), where the protocol types are in
//! scope.

use serde_json::Value;

use cognicode_core::interface::catalog::load_tool_catalog;
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::retired::{
    GraphAnalyzeInput, handle_composite_impact_query, handle_graph_analyze,
};
use cognicode_mcp::rmcp_adapter::build_all_tools;
use cognicode_core::interface::mcp::schemas::CompositeImpactQueryInput;

use crate::fixtures;

/// The catalogue the operator maintains by hand.
const CATALOG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../product/tools.json");

/// The capture of what the server ships.
const RUNTIME: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../product/tool-catalog-runtime.json"
);

/// The names that left the advertised surface.
const RETIRED: [&str; 2] = ["graph_analyze", "composite_impact_query"];

fn read_json(path: &str) -> Value {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"));
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("cannot parse {path}: {e}"))
}

/// The names the catalogue publishes.
fn catalog_names(catalog: &Value) -> Vec<String> {
    catalog
        .get("tools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(|t| t.get("name").and_then(Value::as_str).map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// The names the runtime capture carries.
fn runtime_names(capture: &Value) -> Vec<String> {
    capture
        .as_array()
        .map(|tools| {
            tools
                .iter()
                .filter_map(|t| t.get("name").and_then(Value::as_str).map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn the_retired_tools_are_not_in_the_catalogue() {
    let catalog = read_json(CATALOG);
    let names = catalog_names(&catalog);

    for retired in RETIRED {
        assert!(
            !names.iter().any(|n| n == retired),
            "`{retired}` computes nothing, so the catalogue must not offer it; \
             the catalogue still publishes it"
        );
    }

    // The declared count is the count of the entries: a number that
    // drifts from the list is a second thing to keep in sync.
    let declared = catalog
        .get("runtime_tool_count")
        .and_then(Value::as_u64)
        .expect("the catalogue declares how many tools it publishes");
    assert_eq!(
        declared,
        names.len() as u64,
        "runtime_tool_count must be the number of entries the catalogue holds"
    );
}

#[test]
fn the_retired_tools_are_not_in_the_runtime_capture() {
    let capture = read_json(RUNTIME);
    let names = runtime_names(&capture);

    for retired in RETIRED {
        assert!(
            !names.iter().any(|n| n == retired),
            "`{retired}` left the advertised surface, so the capture the server \
             ships must not carry it either"
        );
    }
}

#[test]
fn the_retired_tools_are_not_in_tools_list() {
    let listed: Vec<String> = build_all_tools()
        .iter()
        .map(|t| t.name.to_string())
        .collect();

    for retired in RETIRED {
        assert!(
            !listed.iter().any(|n| n == retired),
            "`tools/list` advertises `{retired}`, which computes nothing"
        );
    }
}

/// The catalogue and the server agree on what is offered.
#[test]
fn the_catalogue_and_the_server_agree_on_the_retired_names() {
    let catalog = load_tool_catalog();
    let listed: Vec<String> = build_all_tools()
        .iter()
        .map(|t| t.name.to_string())
        .collect();

    for retired in RETIRED {
        let in_catalogue = catalog.iter().any(|d| d.name == retired);
        let in_list = listed.iter().any(|n| n == retired);
        assert_eq!(
            in_catalogue, in_list,
            "`{retired}` is in the catalogue ({in_catalogue}) and in tools/list \
             ({in_list}); the two surfaces cannot disagree"
        );
    }
}

/// The graph of the fixture the repository holds.
async fn graph_of_rust_callgraph(dir: &std::path::Path) -> HandlerContext {
    fixtures::graph_of(dir, "rust-callgraph").await
}

/// The answer of the retired `graph_analyze` name, as JSON.
async fn graph_analyze_answer(ctx: &HandlerContext, mode: &str) -> Value {
    let out = handle_graph_analyze(
        ctx,
        GraphAnalyzeInput {
            mode: mode.to_string(),
        },
    )
    .await
    .unwrap_or_else(|e| panic!("the retired name must still answer (mode {mode}): {e}"));
    serde_json::to_value(&out).expect("the answer is serialisable")
}

#[tokio::test]
async fn the_retired_graph_analyze_serves_the_algorithm_it_named() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = graph_of_rust_callgraph(dir.path()).await;

    let scc = graph_analyze_answer(&ctx, "scc").await;
    assert_eq!(
        scc.get("served_by").and_then(Value::as_str),
        Some("graph_condensed"),
        "the mode `scc` is served by the tool that computes the condensation: {scc}"
    );
    assert_eq!(
        scc.get("algorithm").and_then(Value::as_str),
        Some("condensation"),
        "the answer names the algorithm it computed: {scc}"
    );
    let components = scc
        .get("component_count")
        .and_then(Value::as_u64)
        .expect("the condensation counts the components it found");
    assert!(
        components > 0,
        "the condensation of a graph with symbols has components: {scc}"
    );

    let reduced = graph_analyze_answer(&ctx, "reduced").await;
    assert_eq!(
        reduced.get("served_by").and_then(Value::as_str),
        Some("graph_reduced"),
        "the mode `reduced` is served by the tool that computes the transitive \
         reduction: {reduced}"
    );
    assert_eq!(
        reduced.get("algorithm").and_then(Value::as_str),
        Some("transitive_reduction"),
        "the answer names the algorithm it computed: {reduced}"
    );

    let feedback = graph_analyze_answer(&ctx, "feedback_arcs").await;
    assert_eq!(
        feedback.get("served_by").and_then(Value::as_str),
        Some("graph_feedback_arcs"),
        "the mode `feedback_arcs` is served by the tool that computes the \
         feedback arc set: {feedback}"
    );
    assert_eq!(
        feedback.get("algorithm").and_then(Value::as_str),
        Some("feedback_arc_set"),
        "the answer names the algorithm it computed: {feedback}"
    );
}

/// The three modes are three algorithms, so the three answers are three
/// shapes.
///
/// A handler that computes nothing answers every mode with the same
/// payload wearing a different label — the same keys, the same number,
/// only the label changes. That is what "it does not compute" looks
/// like from the wire, and comparing the *shapes* is what sees it: the
/// labels alone would have hidden it.
#[tokio::test]
async fn the_three_modes_do_not_answer_with_the_same_shape() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = graph_of_rust_callgraph(dir.path()).await;

    let scc = graph_analyze_answer(&ctx, "scc").await;
    let reduced = graph_analyze_answer(&ctx, "reduced").await;
    let feedback = graph_analyze_answer(&ctx, "feedback_arcs").await;

    let keys = |answer: &Value| {
        let mut keys: Vec<String> = answer
            .as_object()
            .expect("the answer is an object")
            .keys()
            .cloned()
            .collect();
        keys.sort();
        keys
    };

    assert_ne!(
        keys(&scc),
        keys(&reduced),
        "the condensation and the transitive reduction answered with the same \
         fields: {scc} / {reduced}"
    );
    assert_ne!(
        keys(&reduced),
        keys(&feedback),
        "the transitive reduction and the feedback arc set answered with the \
         same fields: {reduced} / {feedback}"
    );
    assert_ne!(
        keys(&scc),
        keys(&feedback),
        "the condensation and the feedback arc set answered with the same \
         fields: {scc} / {feedback}"
    );
}

#[tokio::test]
async fn the_retired_composite_impact_query_serves_the_tool_that_computes_the_impact() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = graph_of_rust_callgraph(dir.path()).await;

    let out = handle_composite_impact_query(
        &ctx,
        CompositeImpactQueryInput {
            qualified_name: "helper".to_string(),
            file_path: None,
        },
    )
    .await
    .expect("the retired name must still answer");
    let answer = serde_json::to_value(&out).expect("the answer is serialisable");

    assert_eq!(
        answer.get("served_by").and_then(Value::as_str),
        Some("analyze_impact"),
        "the impact of a symbol is computed by analyze_impact: {answer}"
    );
    assert_eq!(
        answer.get("symbol").and_then(Value::as_str),
        Some("helper"),
        "the answer names the symbol it was asked about: {answer}"
    );
    let impacted = answer
        .get("impacted_symbols")
        .and_then(Value::as_array)
        .expect("the impact of a symbol names the symbols it impacts: {answer}");
    assert!(
        impacted.iter().any(|s| s.as_str() == Some("main")),
        "main calls helper, so main is impacted by changing it: {answer}"
    );
}

/// The answer says which tool served it, and why the name is still
/// there. A caller that gets an answer it cannot interpret is worse
/// off than one that gets a refusal.
#[tokio::test]
async fn the_retired_names_say_which_tool_served_them() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ctx = graph_of_rust_callgraph(dir.path()).await;

    for answer in [
        graph_analyze_answer(&ctx, "scc").await,
        serde_json::to_value(
            handle_composite_impact_query(
                &ctx,
                CompositeImpactQueryInput {
                    qualified_name: "helper".to_string(),
                    file_path: None,
                },
            )
            .await
            .expect("the retired name must still answer"),
        )
        .expect("the answer is serialisable"),
    ] {
        let alias = answer
            .get("alias")
            .and_then(Value::as_str)
            .expect("the answer names the retired name the caller used");
        assert!(
            RETIRED.contains(&alias),
            "the answer names a retired name, not `{alias}`"
        );
        let deprecation = answer
            .get("deprecation")
            .and_then(Value::as_str)
            .expect("the answer says why the name is still there");
        assert!(
            !deprecation.trim().is_empty(),
            "the deprecation note must say something"
        );
    }
}
