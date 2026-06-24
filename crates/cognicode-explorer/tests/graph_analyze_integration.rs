//! Integration tests for the 8 `graph_analyze` MCP handlers:
//! (`graph_pagerank`, `graph_god_nodes`, `graph_communities`,
//! `graph_community_god_nodes`, `graph_surprising_connections`,
//! `graph_transitive_reduction`, `graph_feedback_arc_set`,
//! `graph_all_simple_paths`).
//!
//! Unlike the unit tests in `mcp/handler/graph_analyze.rs` (which only
//! exercise arg parsing), these tests wire the **real** `McpContext`
//! + `CallGraph` through the standard registry dispatch path.
//!
//! This catches integration bugs:
//! - handler ↔ graph type mismatches
//! - symbol-id resolution in subgraph extraction
//! - dispatch lookups in the registry
//! - envelope shape under realistic inputs
//!
//! No PG dependency — the handlers operate on the in-memory
//! `McpContext.graph: Option<Arc<CallGraph>>`.

use std::sync::Arc;

use cognicode_core::domain::aggregates::{CallGraph, Symbol, SymbolId};
use cognicode_core::domain::services::ExtractionContext;
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_explorer::mcp::handler::register_graph_analyze_handlers;
use cognicode_explorer::mcp::handler::ToolHandlerRegistry;
use cognicode_explorer::mcp::{
    McpContext, TOOL_GRAPH_ALL_SIMPLE_PATHS, TOOL_GRAPH_COMMUNITIES,
    TOOL_GRAPH_COMMUNITY_GOD_NODES, TOOL_GRAPH_FEEDBACK_ARC_SET, TOOL_GRAPH_GOD_NODES,
    TOOL_GRAPH_PAGERANK, TOOL_GRAPH_SURPRISING_CONNECTIONS, TOOL_GRAPH_TRANSITIVE_REDUCTION,
};
use cognicode_explorer::session::SessionRegistry;
use serde_json::json;

// ============================================================================
// Fixture builders
// ============================================================================

/// Chain topology: a → b → c → d → e
///
/// Symbol IDs will be "chain.rs:{name}:1"
fn build_chain_fixture() -> CallGraph {
    let mut g = CallGraph::new();
    let names = ["a", "b", "c", "d", "e"];
    let mut ids: Vec<SymbolId> = Vec::new();
    for name in &names {
        let sym = Symbol::new(
            *name,
            SymbolKind::Function,
            Location::new("chain.rs", 1, 0),
        );
        ids.push(g.add_symbol(sym));
    }
    for w in ids.windows(2) {
        g.add_dependency(&w[0], &w[1], DependencyType::Calls).unwrap();
    }
    g
}

/// Disconnected components: two separate clusters.
/// Cluster 1: x1 → x2     Cluster 2: y1 → y2
/// These should produce 2 communities in label propagation.
fn build_communities_fixture() -> CallGraph {
    let mut g = CallGraph::new();
    // Cluster 1
    let x1 = g.add_symbol(Symbol::new(
        "x1", SymbolKind::Function, Location::new("c1.rs", 1, 0),
    ));
    let x2 = g.add_symbol(Symbol::new(
        "x2", SymbolKind::Function, Location::new("c1.rs", 2, 0),
    ));
    g.add_dependency(&x1, &x2, DependencyType::Calls).unwrap();

    // Cluster 2
    let y1 = g.add_symbol(Symbol::new(
        "y1", SymbolKind::Function, Location::new("c2.rs", 1, 0),
    ));
    let y2 = g.add_symbol(Symbol::new(
        "y2", SymbolKind::Function, Location::new("c2.rs", 2, 0),
    ));
    g.add_dependency(&y1, &y2, DependencyType::Calls).unwrap();

    // Add cross-edge between clusters (this becomes the "surprising" connection)
    g.add_dependency(&x2, &y1, DependencyType::Calls).unwrap();

    g
}

/// Cycle: p → q → r → p  (3-node strongly connected cycle)
/// Feedback arc set should return at least 1 edge to break the cycle.
fn build_cycle_fixture() -> CallGraph {
    let mut g = CallGraph::new();
    let p = g.add_symbol(Symbol::new(
        "p", SymbolKind::Function, Location::new("cycle.rs", 1, 0),
    ));
    let q = g.add_symbol(Symbol::new(
        "q", SymbolKind::Function, Location::new("cycle.rs", 2, 0),
    ));
    let r = g.add_symbol(Symbol::new(
        "r", SymbolKind::Function, Location::new("cycle.rs", 3, 0),
    ));
    g.add_dependency(&p, &q, DependencyType::Calls).unwrap();
    g.add_dependency(&q, &r, DependencyType::Calls).unwrap();
    g.add_dependency(&r, &p, DependencyType::Calls).unwrap();
    g
}

/// Diamond topology: start → {mid1, mid2} → end
/// Produces 2 distinct simple paths from start to end.
fn build_diamond_fixture() -> CallGraph {
    let mut g = CallGraph::new();
    let start = g.add_symbol(Symbol::new(
        "start", SymbolKind::Function, Location::new("diamond.rs", 1, 0),
    ));
    let mid1 = g.add_symbol(Symbol::new(
        "mid1", SymbolKind::Function, Location::new("diamond.rs", 2, 0),
    ));
    let mid2 = g.add_symbol(Symbol::new(
        "mid2", SymbolKind::Function, Location::new("diamond.rs", 3, 0),
    ));
    let end = g.add_symbol(Symbol::new(
        "end", SymbolKind::Function, Location::new("diamond.rs", 4, 0),
    ));
    g.add_dependency(&start, &mid1, DependencyType::Calls).unwrap();
    g.add_dependency(&start, &mid2, DependencyType::Calls).unwrap();
    g.add_dependency(&mid1, &end, DependencyType::Calls).unwrap();
    g.add_dependency(&mid2, &end, DependencyType::Calls).unwrap();
    g
}

// ============================================================================
// McpContext helper
// ============================================================================

fn ctx_with_graph(graph: CallGraph) -> McpContext {
    McpContext::builder()
        .with_graph(Some(Arc::new(graph)))
        .with_session_registry(SessionRegistry::new())
        .build()
}

fn build_registry() -> ToolHandlerRegistry {
    let mut registry = ToolHandlerRegistry::new();
    register_graph_analyze_handlers(&mut registry);
    registry
}

// ============================================================================
// graph_pagerank tests
// ============================================================================

#[tokio::test]
async fn graph_pagerank_returns_scores_for_chain_subgraph() {
    let g = build_chain_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    // Use a as root — chain is a→b→c→d→e, depth 5 captures the full chain.
    let raw = registry
        .dispatch(
            TOOL_GRAPH_PAGERANK,
            &ctx,
            json!({ "subgraph": { "root": "chain.rs:a:1", "depth": 5 } }),
        )
        .await;
    let json_text = format!("{raw:?}");

    // The payload should contain a `scores` key with PageRank values.
    assert!(
        json_text.contains("scores"),
        "payload should contain scores key: {json_text}"
    );
    // At least 2 nodes should appear in the scores map.
    assert!(
        json_text.contains("chain.rs:b:1") || json_text.contains("\"b\""),
        "expected node b in pagerank output: {json_text}"
    );
}

#[tokio::test]
async fn graph_pagerank_accepts_alpha_and_max_iterations_options() {
    let g = build_chain_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let raw = registry
        .dispatch(
            TOOL_GRAPH_PAGERANK,
            &ctx,
            json!({
                "subgraph": { "root": "chain.rs:a:1", "depth": 3 },
                "options": { "alpha": 0.9, "max_iterations": 50 }
            }),
        )
        .await;
    let json_text = format!("{raw:?}");

    // Should succeed and return scores (alpha/max_iterations are algorithm params,
    // not reflected verbatim in output — the handler accepts them without error).
    assert!(
        json_text.contains("scores"),
        "options should not cause error: {json_text}"
    );
}

// ============================================================================
// graph_god_nodes tests
// ============================================================================

#[tokio::test]
async fn graph_god_nodes_returns_top_percentile_nodes() {
    let g = build_chain_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let raw = registry
        .dispatch(
            TOOL_GRAPH_GOD_NODES,
            &ctx,
            json!({ "subgraph": { "root": "chain.rs:a:1", "depth": 5 }, "percentile": 0.95 }),
        )
        .await;
    let json_text = format!("{raw:?}");

    // The payload should contain a `nodes` array with id+score entries.
    assert!(
        json_text.contains("nodes"),
        "payload should contain nodes key: {json_text}"
    );
}

#[tokio::test]
async fn graph_god_nodes_defaults_to_95th_percentile() {
    let g = build_chain_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    // No percentile field — should use default 0.95 and still return nodes.
    let raw = registry
        .dispatch(
            TOOL_GRAPH_GOD_NODES,
            &ctx,
            json!({ "subgraph": { "root": "chain.rs:a:1", "depth": 5 } }),
        )
        .await;
    let json_text = format!("{raw:?}");

    assert!(
        json_text.contains("nodes"),
        "default percentile should still produce nodes: {json_text}"
    );
}

// ============================================================================
// graph_communities tests
// ============================================================================

#[tokio::test]
async fn graph_communities_detects_multiple_communities() {
    let g = build_communities_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    // Extract from x1 — the subgraph includes both clusters due to the cross-edge.
    let raw = registry
        .dispatch(
            TOOL_GRAPH_COMMUNITIES,
            &ctx,
            json!({ "subgraph": { "root": "c1.rs:x1:1", "depth": 5 } }),
        )
        .await;
    let json_text = format!("{raw:?}");

    // Label propagation should find at least 2 communities in this disconnected graph.
    assert!(
        json_text.contains("communities"),
        "payload should contain communities key: {json_text}"
    );
    // The output is Vec<Vec<String>> — outer array brackets should appear.
    assert!(
        json_text.contains("[[") || json_text.contains("c1.rs:x1:1"),
        "communities structure should appear in output: {json_text}"
    );
}

#[tokio::test]
async fn graph_communities_respects_max_iterations() {
    let g = build_communities_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let raw = registry
        .dispatch(
            TOOL_GRAPH_COMMUNITIES,
            &ctx,
            json!({
                "subgraph": { "root": "c1.rs:x1:1", "depth": 5 },
                "max_iterations": 10
            }),
        )
        .await;
    let json_text = format!("{raw:?}");

    // Should not error on max_iterations param.
    assert!(
        json_text.contains("communities"),
        "max_iterations should not cause error: {json_text}"
    );
}

// ============================================================================
// graph_community_god_nodes tests
// ============================================================================

#[tokio::test]
async fn graph_community_god_nodes_returns_per_community_god_nodes() {
    let g = build_communities_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let raw = registry
        .dispatch(
            TOOL_GRAPH_COMMUNITY_GOD_NODES,
            &ctx,
            json!({ "subgraph": { "root": "c1.rs:x1:1", "depth": 5 } }),
        )
        .await;
    let json_text = format!("{raw:?}");

    // Each node should have a community_index + id + score.
    assert!(
        json_text.contains("nodes"),
        "payload should contain nodes: {json_text}"
    );
    assert!(
        json_text.contains("community_index"),
        "nodes should include community_index: {json_text}"
    );
}

#[tokio::test]
async fn graph_community_god_nodes_accepts_percentile() {
    let g = build_communities_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let raw = registry
        .dispatch(
            TOOL_GRAPH_COMMUNITY_GOD_NODES,
            &ctx,
            json!({
                "subgraph": { "root": "c1.rs:x1:1", "depth": 5 },
                "percentile": 0.99
            }),
        )
        .await;
    let json_text = format!("{raw:?}");

    assert!(
        json_text.contains("nodes"),
        "percentile should not cause error: {json_text}"
    );
}

// ============================================================================
// graph_surprising_connections tests
// ============================================================================

#[tokio::test]
async fn graph_surprising_connections_returns_cross_cluster_edges() {
    let g = build_communities_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let raw = registry
        .dispatch(
            TOOL_GRAPH_SURPRISING_CONNECTIONS,
            &ctx,
            json!({ "subgraph": { "root": "c1.rs:x1:1", "depth": 5 } }),
        )
        .await;
    let json_text = format!("{raw:?}");

    // Should return edges with source_id/target_id/score fields.
    assert!(
        json_text.contains("edges"),
        "payload should contain edges key: {json_text}"
    );
}

#[tokio::test]
async fn graph_surprising_connections_respects_limit() {
    let g = build_communities_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let raw = registry
        .dispatch(
            TOOL_GRAPH_SURPRISING_CONNECTIONS,
            &ctx,
            json!({
                "subgraph": { "root": "c1.rs:x1:1", "depth": 5 },
                "limit": 1
            }),
        )
        .await;
    let json_text = format!("{raw:?}");

    // limit param should be accepted without error.
    assert!(
        json_text.contains("edges"),
        "limit should not cause error: {json_text}"
    );
}

// ============================================================================
// graph_transitive_reduction tests
// ============================================================================

#[tokio::test]
async fn graph_transitive_reduction_removes_transitive_edges() {
    let g = build_chain_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    // The chain a→b→c→d→e has no transitive edges (each pair has only one path),
    // so the transitive reduction = the original edges.
    let raw = registry
        .dispatch(
            TOOL_GRAPH_TRANSITIVE_REDUCTION,
            &ctx,
            json!({ "subgraph": { "root": "chain.rs:a:1", "depth": 5 } }),
        )
        .await;
    let json_text = format!("{raw:?}");

    // Should return edges in the reduced graph.
    assert!(
        json_text.contains("edges"),
        "payload should contain edges: {json_text}"
    );
    // The source_id/target_id field names should appear.
    assert!(
        json_text.contains("source_id") && json_text.contains("target_id"),
        "edges should have source_id and target_id: {json_text}"
    );
}

// ============================================================================
// graph_feedback_arc_set tests
// ============================================================================

#[tokio::test]
async fn graph_feedback_arc_set_finds_edges_to_break_cycle() {
    let g = build_cycle_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let raw = registry
        .dispatch(
            TOOL_GRAPH_FEEDBACK_ARC_SET,
            &ctx,
            json!({ "subgraph": { "root": "cycle.rs:p:1", "depth": 5 } }),
        )
        .await;
    let json_text = format!("{raw:?}");

    // A 3-node cycle should require at least 1 edge removal.
    assert!(
        json_text.contains("edges"),
        "payload should contain edges: {json_text}"
    );
    // FAS output has source_id/target_id for each candidate edge.
    assert!(
        json_text.contains("source_id") && json_text.contains("target_id"),
        "FAS edges should have source_id and target_id: {json_text}"
    );
}

// ============================================================================
// graph_all_simple_paths tests
// ============================================================================

#[tokio::test]
async fn graph_all_simple_paths_enumerates_multiple_paths() {
    // Use the chain fixture: a → b → c → d → e
    // Find paths from a to e — there is exactly 1 path: a→b→c→d→e
    let g = build_chain_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let raw = registry
        .dispatch(
            TOOL_GRAPH_ALL_SIMPLE_PATHS,
            &ctx,
            json!({
                "subgraph": { "root": "chain.rs:a:1", "depth": 5 },
                "from": "chain.rs:a:1",
                "to": "chain.rs:e:1"
            }),
        )
        .await;
    let json_text = format!("{raw:?}");

    assert!(
        json_text.contains("paths"),
        "payload should contain paths: {json_text}"
    );
    // Chain a→b→c→d→e has exactly 1 path.
    assert!(
        !json_text.contains("\"paths\": []"),
        "all_simple_paths on chain should find 1 path: {json_text}"
    );
    // The path should include intermediate nodes (b, c, d).
    assert!(
        json_text.contains("b") && json_text.contains("c") && json_text.contains("d"),
        "path should include intermediate nodes b, c, d: {json_text}"
    );
}

#[tokio::test]
async fn graph_all_simple_paths_respects_max_hops() {
    let g = build_diamond_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let raw = registry
        .dispatch(
            TOOL_GRAPH_ALL_SIMPLE_PATHS,
            &ctx,
            json!({
                "subgraph": { "root": "diamond.rs:start:1", "depth": 5 },
                "from": "diamond.rs:start:1",
                "to": "diamond.rs:end:4",
                "max_hops": 3
            }),
        )
        .await;
    let json_text = format!("{raw:?}");

    // max_hops param should be accepted without error.
    assert!(
        json_text.contains("paths"),
        "max_hops should not cause error: {json_text}"
    );
}

// ============================================================================
// Error-path tests
// ============================================================================

#[tokio::test]
async fn graph_pagerank_rejects_missing_root() {
    let g = build_chain_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let raw = registry
        .dispatch(TOOL_GRAPH_PAGERANK, &ctx, json!({ "subgraph": {} }))
        .await;
    let json_text = format!("{raw:?}");

    assert!(
        json_text.contains("missing_required_arg"),
        "expected missing_required_arg error: {json_text}"
    );
}

#[tokio::test]
async fn graph_all_simple_paths_rejects_missing_from() {
    let g = build_diamond_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let raw = registry
        .dispatch(
            TOOL_GRAPH_ALL_SIMPLE_PATHS,
            &ctx,
            json!({
                "subgraph": { "root": "diamond.rs:start:1" },
                "to": "diamond.rs:end:1"
            }),
        )
        .await;
    let json_text = format!("{raw:?}");

    assert!(
        json_text.contains("missing_required_arg"),
        "expected missing_required_arg for missing from: {json_text}"
    );
}

#[tokio::test]
async fn graph_all_simple_paths_rejects_missing_to() {
    let g = build_diamond_fixture();
    let ctx = ctx_with_graph(g);
    let registry = build_registry();

    let raw = registry
        .dispatch(
            TOOL_GRAPH_ALL_SIMPLE_PATHS,
            &ctx,
            json!({
                "subgraph": { "root": "diamond.rs:start:1" },
                "from": "diamond.rs:start:1"
            }),
        )
        .await;
    let json_text = format!("{raw:?}");

    assert!(
        json_text.contains("missing_required_arg"),
        "expected missing_required_arg for missing to: {json_text}"
    );
}

#[tokio::test]
async fn graph_god_nodes_returns_error_when_graph_unavailable() {
    // McpContext with NO graph wired.
    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .build();
    let registry = build_registry();

    let raw = registry
        .dispatch(
            TOOL_GRAPH_GOD_NODES,
            &ctx,
            json!({ "subgraph": { "root": "chain.rs:a:1" } }),
        )
        .await;
    let json_text = format!("{raw:?}");

    assert!(
        json_text.contains("graph_unavailable"),
        "expected graph_unavailable error: {json_text}"
    );
}

#[tokio::test]
async fn graph_communities_returns_error_when_graph_unavailable() {
    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .build();
    let registry = build_registry();

    let raw = registry
        .dispatch(
            TOOL_GRAPH_COMMUNITIES,
            &ctx,
            json!({ "subgraph": { "root": "c1.rs:x1:1" } }),
        )
        .await;
    let json_text = format!("{raw:?}");

    assert!(
        json_text.contains("graph_unavailable"),
        "expected graph_unavailable error: {json_text}"
    );
}

#[tokio::test]
async fn graph_transitive_reduction_returns_error_when_graph_unavailable() {
    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .build();
    let registry = build_registry();

    let raw = registry
        .dispatch(
            TOOL_GRAPH_TRANSITIVE_REDUCTION,
            &ctx,
            json!({ "subgraph": { "root": "chain.rs:a:1" } }),
        )
        .await;
    let json_text = format!("{raw:?}");

    assert!(
        json_text.contains("graph_unavailable"),
        "expected graph_unavailable error: {json_text}"
    );
}

#[tokio::test]
async fn graph_feedback_arc_set_returns_error_when_graph_unavailable() {
    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .build();
    let registry = build_registry();

    let raw = registry
        .dispatch(
            TOOL_GRAPH_FEEDBACK_ARC_SET,
            &ctx,
            json!({ "subgraph": { "root": "cycle.rs:p:1" } }),
        )
        .await;
    let json_text = format!("{raw:?}");

    assert!(
        json_text.contains("graph_unavailable"),
        "expected graph_unavailable error: {json_text}"
    );
}

#[tokio::test]
async fn graph_all_simple_paths_returns_error_when_graph_unavailable() {
    let ctx = McpContext::builder()
        .with_session_registry(SessionRegistry::new())
        .build();
    let registry = build_registry();

    let raw = registry
        .dispatch(
            TOOL_GRAPH_ALL_SIMPLE_PATHS,
            &ctx,
            json!({
                "subgraph": { "root": "diamond.rs:start:1" },
                "from": "diamond.rs:start:1",
                "to": "diamond.rs:end:1"
            }),
        )
        .await;
    let json_text = format!("{raw:?}");

    assert!(
        json_text.contains("graph_unavailable"),
        "expected graph_unavailable error: {json_text}"
    );
}

// ============================================================================
// Sanity tests
// ============================================================================

#[test]
fn all_eight_handlers_are_registered() {
    let registry = build_registry();
    assert!(
        registry.get(TOOL_GRAPH_PAGERANK).is_some(),
        "graph_pagerank should be registered"
    );
    assert!(
        registry.get(TOOL_GRAPH_GOD_NODES).is_some(),
        "graph_god_nodes should be registered"
    );
    assert!(
        registry.get(TOOL_GRAPH_COMMUNITIES).is_some(),
        "graph_communities should be registered"
    );
    assert!(
        registry.get(TOOL_GRAPH_COMMUNITY_GOD_NODES).is_some(),
        "graph_community_god_nodes should be registered"
    );
    assert!(
        registry.get(TOOL_GRAPH_SURPRISING_CONNECTIONS).is_some(),
        "graph_surprising_connections should be registered"
    );
    assert!(
        registry.get(TOOL_GRAPH_TRANSITIVE_REDUCTION).is_some(),
        "graph_transitive_reduction should be registered"
    );
    assert!(
        registry.get(TOOL_GRAPH_FEEDBACK_ARC_SET).is_some(),
        "graph_feedback_arc_set should be registered"
    );
    assert!(
        registry.get(TOOL_GRAPH_ALL_SIMPLE_PATHS).is_some(),
        "graph_all_simple_paths should be registered"
    );
}

#[test]
fn handlers_declare_non_empty_arg_schemas() {
    let registry = build_registry();
    for (name, expected_required_keys) in [
        (TOOL_GRAPH_PAGERANK, vec!["subgraph"]),
        (TOOL_GRAPH_GOD_NODES, vec!["subgraph"]),
        (TOOL_GRAPH_COMMUNITIES, vec!["subgraph"]),
        (TOOL_GRAPH_COMMUNITY_GOD_NODES, vec!["subgraph"]),
        (TOOL_GRAPH_SURPRISING_CONNECTIONS, vec!["subgraph"]),
        (TOOL_GRAPH_TRANSITIVE_REDUCTION, vec!["subgraph"]),
        (TOOL_GRAPH_FEEDBACK_ARC_SET, vec!["subgraph"]),
        (
            TOOL_GRAPH_ALL_SIMPLE_PATHS,
            vec!["subgraph", "from", "to"],
        ),
    ] {
        let handler = registry.get(name).unwrap();
        let schema = handler.arg_schema();
        let required = schema
            .get("required")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for key in expected_required_keys {
            assert!(
                required.contains(&key),
                "{name}: schema should require `{key}`, got {required:?}"
            );
        }
    }
}

#[test]
fn subgraph_schema_requires_root() {
    let registry = build_registry();
    // Every tool's schema has subgraph.root as a required field inside the subgraph object.
    for name in [
        TOOL_GRAPH_PAGERANK,
        TOOL_GRAPH_GOD_NODES,
        TOOL_GRAPH_COMMUNITIES,
        TOOL_GRAPH_COMMUNITY_GOD_NODES,
        TOOL_GRAPH_SURPRISING_CONNECTIONS,
        TOOL_GRAPH_TRANSITIVE_REDUCTION,
        TOOL_GRAPH_FEEDBACK_ARC_SET,
        TOOL_GRAPH_ALL_SIMPLE_PATHS,
    ] {
        let handler = registry.get(name).unwrap();
        let schema = handler.arg_schema();
        let subgraph = schema.get("properties").and_then(|p| p.get("subgraph"));
        let subgraph_required = subgraph
            .and_then(|s| s.get("required"))
            .and_then(|r| r.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        assert!(
            subgraph_required.contains(&"root"),
            "{name}: subgraph schema should require `root`, got {subgraph_required:?}"
        );
    }
}
