use super::graph_query_says_what_it_traversed::context_holding;
use cognicode_core::domain::aggregates::call_graph::{CallGraph, SymbolId};
use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::services::ExtractionContext;
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_core::interface::mcp::handlers::graph_query_handlers::{
    GraphExplainInput, handle_graph_explain,
};
use serde_json::Value;

fn add_symbol(graph: &mut CallGraph, name: &str, line: u32) -> SymbolId {
    graph.add_symbol(Symbol::new(
        name,
        SymbolKind::Function,
        Location::new("lib.rs", line, 0),
    ))
}

#[tokio::test]
async fn graph_explain_preserves_edge_metadata_coverage_and_truncation() {
    let mut graph = CallGraph::new();
    let target = add_symbol(&mut graph, "target", 1);
    for index in 0..21 {
        let name = format!("caller_{index:02}");
        let caller = add_symbol(&mut graph, &name, index + 2);
        let kind = if index % 2 == 0 {
            DependencyType::Calls
        } else {
            DependencyType::References
        };
        let how = if index % 2 == 0 {
            ExtractionContext::DirectExtraction
        } else {
            ExtractionContext::Heuristic { score: 0.7 }
        };
        graph
            .add_dependency_with_provenance(&caller, &target, kind, how)
            .expect("both relation endpoints exist");
    }
    let outgoing = add_symbol(&mut graph, "outgoing", 30);
    graph
        .add_dependency_with_provenance(
            &target,
            &outgoing,
            DependencyType::Inherits,
            ExtractionContext::Manual,
        )
        .expect("both relation endpoints exist");

    let context = context_holding(graph);
    let output = handle_graph_explain(
        &context,
        GraphExplainInput {
            symbol: "target".into(),
            depth: 1,
        },
    )
    .await
    .expect("graph_explain answers");
    let answer: Value = serde_json::to_value(output).expect("serialize MCP output");

    assert_eq!(answer["callers"].as_array().unwrap().len(), 20);
    assert_eq!(answer["callees"].as_array().unwrap().len(), 1);
    assert_eq!(answer["callers_truncated"], true);
    assert_eq!(answer["callees_truncated"], false);
    assert_eq!(answer["coverage"]["status"], "unknown");

    let caller = answer["callers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| {
            entry["symbol"]
                .as_str()
                .is_some_and(|id| id.contains("caller_00:"))
        })
        .unwrap_or_else(|| panic!("first caller missing from graph_explain: {answer}"));
    assert_eq!(caller["relation"], "calls");
    assert_eq!(caller["side"], "incoming");
    assert_eq!(caller["source"], caller["symbol"]);
    assert_eq!(caller["target"], answer["node"]["id"]);
    assert_eq!(caller["provenance"], "Extracted");
    assert_eq!(caller["confidence"], 1.0);

    let inferred = answer["callers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| {
            entry["symbol"]
                .as_str()
                .is_some_and(|id| id.contains("caller_01:"))
        })
        .expect("the second caller is within the limit");
    assert_eq!(inferred["relation"], "references");
    assert_eq!(inferred["provenance"], "Inferred");
    assert_eq!(inferred["confidence"], 0.7);

    let callee = &answer["callees"][0];
    assert_eq!(callee["relation"], "inherits");
    assert_eq!(callee["side"], "outgoing");
    assert_eq!(callee["source"], answer["node"]["id"]);
    assert_eq!(callee["target"], callee["symbol"]);
    assert_eq!(callee["provenance"], "Manual");
}
