use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::consolidated_handlers::{
    ProjectInsightsInput, handle_project_insights,
};

fn add_symbol(
    graph: &mut CallGraph,
    name: &str,
    line: u32,
) -> cognicode_core::domain::aggregates::call_graph::SymbolId {
    graph.add_symbol(Symbol::new(
        name,
        SymbolKind::Function,
        Location::new("lib.rs", line, 0),
    ))
}

fn context_holding(graph: CallGraph) -> HandlerContext {
    let ctx = HandlerContext::builder().build();
    ctx.analysis_service().graph_cache().replace(graph);
    ctx
}

async fn insights(
    ctx: &HandlerContext,
) -> cognicode_core::interface::mcp::handlers::consolidated_handlers::ProjectInsightsOutput {
    handle_project_insights(ctx, ProjectInsightsInput {})
        .await
        .expect("project_insights answers")
}

#[tokio::test]
async fn symbols_without_edges_do_not_claim_god_nodes() {
    let mut graph = CallGraph::new();
    for (line, name) in ["a", "b", "c"].into_iter().enumerate() {
        add_symbol(&mut graph, name, line as u32 + 1);
    }
    let output = insights(&context_holding(graph)).await;

    assert_eq!(output.total_symbols, 3);
    assert_eq!(output.total_edges, 0);
    assert!(output.hot_paths.is_empty());
    assert!(
        output.summary.contains("no ranking signal"),
        "{}",
        output.summary
    );
}

#[tokio::test]
async fn a_hub_is_ranked_by_edges_even_when_its_id_sorts_last() {
    let mut graph = CallGraph::new();
    let callers = ["a", "b", "c", "d", "e"]
        .into_iter()
        .enumerate()
        .map(|(line, name)| add_symbol(&mut graph, name, line as u32 + 1))
        .collect::<Vec<_>>();
    let hub = add_symbol(&mut graph, "zz_hub", 6);
    for caller in callers {
        graph
            .add_dependency(&caller, &hub, DependencyType::Calls)
            .expect("the graph contains both endpoints");
    }
    let output = insights(&context_holding(graph)).await;

    assert_eq!(
        output.hot_paths.first().map(|path| path.symbol_id.as_str()),
        Some("zz_hub")
    );
}
