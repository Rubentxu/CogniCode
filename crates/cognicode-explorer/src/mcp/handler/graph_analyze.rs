//! Graph-analysis tool handlers exposing cognicode-graph-algos via MCP.
//!
//! Implements 8 MCP tools:
//! - `graph_pagerank`             — PageRank scores per symbol
//! - `graph_god_nodes`            — top-percentile symbols by PageRank
//! - `graph_communities`          — Label-Propagation community detection
//! - `graph_community_god_nodes`  — god nodes within each community
//! - `graph_surprising_connections` — cross-community edges
//! - `graph_transitive_reduction`  — minimal edge set preserving reachability
//! - `graph_feedback_arc_set`     — cycle-breaking edge candidates
//! - `graph_all_simple_paths`     — enumerate simple paths between two symbols

use std::sync::Arc;

use async_trait::async_trait;
use cognicode_core::application::services::community_detector::CommunityDetector;
use cognicode_core::application::services::graph_analytics::GraphAnalyticsService;
use cognicode_core::domain::aggregates::{CallGraph, Symbol, SymbolId};
use cognicode_core::domain::services::ExtractionContext;
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_core::infrastructure::graph::{CallGraphProjection, SubgraphDirection};
use rmcp::model::CallToolResult;
use serde::Deserialize;
use serde_json::Value;

use crate::mcp::envelope::{err_envelope, ok_envelope};
use crate::mcp::handler::ToolHandler;
use crate::mcp::{
    DEFAULT_SUBGRAPH_DEPTH, McpContext, TOOL_GRAPH_ALL_SIMPLE_PATHS, TOOL_GRAPH_COMMUNITIES,
    TOOL_GRAPH_COMMUNITY_GOD_NODES, TOOL_GRAPH_FEEDBACK_ARC_SET, TOOL_GRAPH_GOD_NODES,
    TOOL_GRAPH_PAGERANK, TOOL_GRAPH_SURPRISING_CONNECTIONS, TOOL_GRAPH_TRANSITIVE_REDUCTION,
};

// ============================================================================
// Arg structs
// ============================================================================

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SubgraphArgs {
    root: Option<String>,
    depth: Option<usize>,
    direction: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GraphPagerankArgs {
    subgraph: SubgraphArgs,
    options: Option<PagerankOptions>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PagerankOptions {
    alpha: Option<f64>,
    max_iterations: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GraphGodNodesArgs {
    subgraph: SubgraphArgs,
    percentile: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GraphCommunitiesArgs {
    subgraph: SubgraphArgs,
    max_iterations: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GraphCommunityGodNodesArgs {
    subgraph: SubgraphArgs,
    percentile: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GraphSurprisingConnectionsArgs {
    subgraph: SubgraphArgs,
    limit: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GraphTransitiveReductionArgs {
    subgraph: SubgraphArgs,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GraphFeedbackArcSetArgs {
    subgraph: SubgraphArgs,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GraphAllSimplePathsArgs {
    subgraph: SubgraphArgs,
    from: Option<String>,
    to: Option<String>,
    max_hops: Option<usize>,
}

// ============================================================================
// require_graph — shared guard
// ============================================================================

fn require_graph<'a>(
    ctx: &'a McpContext,
    tool: &str,
) -> Result<&'a Arc<CallGraph>, CallToolResult> {
    ctx.graph.as_ref().ok_or_else(|| {
        err_envelope(
            tool,
            "graph_unavailable",
            &format!("{tool}: graph analysis unavailable — no call graph loaded"),
        )
    })
}

// ============================================================================
// Shared helpers
// ============================================================================

/// Parse direction string into SubgraphDirection.
fn parse_direction(dir: Option<&str>) -> SubgraphDirection {
    match dir.unwrap_or("both") {
        "outgoing" => SubgraphDirection::Outgoing,
        "incoming" => SubgraphDirection::Incoming,
        _ => SubgraphDirection::Both,
    }
}

/// Extract a subgraph from the full call graph using CallGraphProjection.
fn extract_subgraph_view(
    graph: &CallGraph,
    root: &str,
    direction: SubgraphDirection,
    depth: usize,
) -> cognicode_core::infrastructure::graph::SubgraphView {
    let projection = CallGraphProjection::from_call_graph(graph);
    projection.extract_subgraph(&SymbolId::new(root), direction, depth)
}

/// Build a CallGraph from a SubgraphView.
///
// The subgraph nodes and edges carry only SymbolId on nodes and
// (DependencyType, confidence) on edges — we reconstruct a minimal
// CallGraph from those primitives.
fn build_subgraph_callgraph(
    view: &cognicode_core::infrastructure::graph::SubgraphView,
) -> CallGraph {
    let mut cg = CallGraph::new();

    // Map old SymbolId → new SymbolId after add_symbol, so we can
    // correctly re-map edges even when the new graph's internal SymbolId
    // differs from the original.
    let mut old_to_new: std::collections::HashMap<SymbolId, SymbolId> =
        std::collections::HashMap::new();

    for node_id in &view.nodes {
        // Use a placeholder name; after add_symbol, we use set_fqn_override
        // to restore the original FQN. This bypasses Location's inability to
        // store the symbol name (Location only carries file/line/column).
        let placeholder_name = node_id.as_str();
        let sym = Symbol::new(
            placeholder_name,
            SymbolKind::Function,
            Location::new("subgraph", 1, 1),
        );
        let new_id = cg.add_symbol(sym);
        // Restore the original FQN now that the symbol is owned by cg.
        cg.get_symbol_mut(&new_id)
            .expect("symbol must exist")
            .set_fqn_override(node_id.as_str());
        old_to_new.insert(node_id.clone(), new_id);
    }

    for edge in &view.edges {
        let src = old_to_new.get(&edge.source).unwrap_or(&edge.source);
        let dst = old_to_new.get(&edge.target).unwrap_or(&edge.target);
        let _ = cg.add_dependency_with_provenance(
            src,
            dst,
            edge.dependency_type,
            ExtractionContext::DirectExtraction,
        );
    }
    cg
}

// ============================================================================
// ToolHandler implementations
// ============================================================================

// --- graph_pagerank ---

struct GraphPagerankHandler;

#[async_trait]
impl ToolHandler for GraphPagerankHandler {
    fn name(&self) -> &'static str {
        TOOL_GRAPH_PAGERANK
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "subgraph": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Root symbol id (required)" },
                        "depth": { "type": "integer", "description": "BFS depth (default 3)" },
                        "direction": { "type": "string", "enum": ["outgoing", "incoming", "both"] }
                    },
                    "required": ["root"]
                },
                "options": {
                    "type": "object",
                    "properties": {
                        "alpha": { "type": "number", "default": 0.85 },
                        "max_iterations": { "type": "integer", "default": 100 }
                    }
                }
            },
            "required": ["subgraph"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: GraphPagerankArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_GRAPH_PAGERANK,
                    "invalid_args",
                    &format!("{TOOL_GRAPH_PAGERANK}: invalid args: {e}"),
                );
            }
        };

        let g = match require_graph(ctx, TOOL_GRAPH_PAGERANK) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let root = match args.subgraph.root {
            Some(r) if !r.is_empty() => r,
            _ => {
                return err_envelope(
                    TOOL_GRAPH_PAGERANK,
                    "missing_required_arg",
                    "graph_pagerank: missing required arg `subgraph.root`",
                );
            }
        };

        let direction = parse_direction(args.subgraph.direction.as_deref());
        let depth = args.subgraph.depth.unwrap_or(DEFAULT_SUBGRAPH_DEPTH);
        let alpha = args.options.as_ref().and_then(|o| o.alpha).unwrap_or(0.85);
        let max_iter = args
            .options
            .as_ref()
            .and_then(|o| o.max_iterations)
            .unwrap_or(100);

        let view = extract_subgraph_view(g, &root, direction, depth);

        if view.nodes.is_empty() {
            return ok_envelope(TOOL_GRAPH_PAGERANK, &serde_json::json!({ "scores": {} }));
        }

        let sub_cg = build_subgraph_callgraph(&view);
        let scores = GraphAnalyticsService::page_rank(&sub_cg, alpha, max_iter);
        let scores_str: std::collections::HashMap<String, f64> = scores
            .into_iter()
            .map(|(k, v)| (k.as_str().to_string(), v))
            .collect();
        let payload = serde_json::json!({ "scores": scores_str });
        ok_envelope(TOOL_GRAPH_PAGERANK, &payload)
    }
}

// --- graph_god_nodes ---

struct GraphGodNodesHandler;

#[async_trait]
impl ToolHandler for GraphGodNodesHandler {
    fn name(&self) -> &'static str {
        TOOL_GRAPH_GOD_NODES
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "subgraph": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Root symbol id (required)" },
                        "depth": { "type": "integer", "description": "BFS depth (default 3)" },
                        "direction": { "type": "string", "enum": ["outgoing", "incoming", "both"] }
                    },
                    "required": ["root"]
                },
                "percentile": { "type": "number", "description": "Percentile threshold (default 0.95)" }
            },
            "required": ["subgraph"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: GraphGodNodesArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_GRAPH_GOD_NODES,
                    "invalid_args",
                    &format!("{TOOL_GRAPH_GOD_NODES}: invalid args: {e}"),
                );
            }
        };

        let g = match require_graph(ctx, TOOL_GRAPH_GOD_NODES) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let root = match args.subgraph.root {
            Some(r) if !r.is_empty() => r,
            _ => {
                return err_envelope(
                    TOOL_GRAPH_GOD_NODES,
                    "missing_required_arg",
                    "graph_god_nodes: missing required arg `subgraph.root`",
                );
            }
        };

        let direction = parse_direction(args.subgraph.direction.as_deref());
        let depth = args.subgraph.depth.unwrap_or(DEFAULT_SUBGRAPH_DEPTH);
        let percentile = args.percentile.unwrap_or(0.95);

        let view = extract_subgraph_view(g, &root, direction, depth);

        if view.nodes.is_empty() {
            return ok_envelope(TOOL_GRAPH_GOD_NODES, &serde_json::json!({ "nodes": [] }));
        }

        let sub_cg = build_subgraph_callgraph(&view);
        let god = GraphAnalyticsService::god_nodes(&sub_cg, percentile);
        let nodes: Vec<_> = god
            .into_iter()
            .map(|(sid, score)| {
                serde_json::json!({
                    "id": sid.as_str().to_string(),
                    "score": score
                })
            })
            .collect();
        let payload = serde_json::json!({ "nodes": nodes });
        ok_envelope(TOOL_GRAPH_GOD_NODES, &payload)
    }
}

// --- graph_communities ---

struct GraphCommunitiesHandler;

#[async_trait]
impl ToolHandler for GraphCommunitiesHandler {
    fn name(&self) -> &'static str {
        TOOL_GRAPH_COMMUNITIES
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "subgraph": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Root symbol id (required)" },
                        "depth": { "type": "integer", "description": "BFS depth (default 3)" },
                        "direction": { "type": "string", "enum": ["outgoing", "incoming", "both"] }
                    },
                    "required": ["root"]
                },
                "max_iterations": { "type": "integer", "description": "Max Label Propagation iterations (default 100)" }
            },
            "required": ["subgraph"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: GraphCommunitiesArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_GRAPH_COMMUNITIES,
                    "invalid_args",
                    &format!("{TOOL_GRAPH_COMMUNITIES}: invalid args: {e}"),
                );
            }
        };

        let g = match require_graph(ctx, TOOL_GRAPH_COMMUNITIES) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let root = match args.subgraph.root {
            Some(r) if !r.is_empty() => r,
            _ => {
                return err_envelope(
                    TOOL_GRAPH_COMMUNITIES,
                    "missing_required_arg",
                    "graph_communities: missing required arg `subgraph.root`",
                );
            }
        };

        let direction = parse_direction(args.subgraph.direction.as_deref());
        let depth = args.subgraph.depth.unwrap_or(DEFAULT_SUBGRAPH_DEPTH);
        let max_iter = args
            .max_iterations
            .unwrap_or(CommunityDetector::MAX_ITERATIONS);

        let view = extract_subgraph_view(g, &root, direction, depth);

        if view.nodes.is_empty() {
            return ok_envelope(
                TOOL_GRAPH_COMMUNITIES,
                &serde_json::json!({ "communities": [] }),
            );
        }

        let sub_cg = build_subgraph_callgraph(&view);
        let result = CommunityDetector::detect(&sub_cg, max_iter);
        let communities: Vec<Vec<String>> = result
            .communities
            .into_iter()
            .map(|c| {
                c.nodes
                    .into_iter()
                    .map(|s| s.as_str().to_string())
                    .collect()
            })
            .collect();
        let payload = serde_json::json!({ "communities": communities });
        ok_envelope(TOOL_GRAPH_COMMUNITIES, &payload)
    }
}

// --- graph_community_god_nodes ---

struct GraphCommunityGodNodesHandler;

#[async_trait]
impl ToolHandler for GraphCommunityGodNodesHandler {
    fn name(&self) -> &'static str {
        TOOL_GRAPH_COMMUNITY_GOD_NODES
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "subgraph": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Root symbol id (required)" },
                        "depth": { "type": "integer", "description": "BFS depth (default 3)" },
                        "direction": { "type": "string", "enum": ["outgoing", "incoming", "both"] }
                    },
                    "required": ["root"]
                },
                "percentile": { "type": "number", "description": "Percentile threshold (default 0.95)" }
            },
            "required": ["subgraph"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: GraphCommunityGodNodesArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_GRAPH_COMMUNITY_GOD_NODES,
                    "invalid_args",
                    &format!("{TOOL_GRAPH_COMMUNITY_GOD_NODES}: invalid args: {e}"),
                );
            }
        };

        let g = match require_graph(ctx, TOOL_GRAPH_COMMUNITY_GOD_NODES) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let root = match args.subgraph.root {
            Some(r) if !r.is_empty() => r,
            _ => {
                return err_envelope(
                    TOOL_GRAPH_COMMUNITY_GOD_NODES,
                    "missing_required_arg",
                    "graph_community_god_nodes: missing required arg `subgraph.root`",
                );
            }
        };

        let direction = parse_direction(args.subgraph.direction.as_deref());
        let depth = args.subgraph.depth.unwrap_or(DEFAULT_SUBGRAPH_DEPTH);
        let percentile = args.percentile.unwrap_or(0.95);

        let view = extract_subgraph_view(g, &root, direction, depth);

        if view.nodes.is_empty() {
            return ok_envelope(
                TOOL_GRAPH_COMMUNITY_GOD_NODES,
                &serde_json::json!({ "nodes": [] }),
            );
        }

        let sub_cg = build_subgraph_callgraph(&view);
        let community_result =
            CommunityDetector::detect(&sub_cg, CommunityDetector::MAX_ITERATIONS);
        let top_n = ((percentile * 100.0) as usize).max(1);
        let community_god =
            CommunityDetector::community_god_nodes(&sub_cg, &community_result.communities, top_n);

        let nodes: Vec<_> = community_god
            .into_iter()
            .flat_map(|(comm_id, gods)| {
                gods.into_iter()
                    .map(|(sid, score)| {
                        serde_json::json!({
                            "community_index": comm_id,
                            "id": sid.as_str().to_string(),
                            "score": score
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        let payload = serde_json::json!({ "nodes": nodes });
        ok_envelope(TOOL_GRAPH_COMMUNITY_GOD_NODES, &payload)
    }
}

// --- graph_surprising_connections ---

struct GraphSurprisingConnectionsHandler;

#[async_trait]
impl ToolHandler for GraphSurprisingConnectionsHandler {
    fn name(&self) -> &'static str {
        TOOL_GRAPH_SURPRISING_CONNECTIONS
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "subgraph": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Root symbol id (required)" },
                        "depth": { "type": "integer", "description": "BFS depth (default 3)" },
                        "direction": { "type": "string", "enum": ["outgoing", "incoming", "both"] }
                    },
                    "required": ["root"]
                },
                "limit": { "type": "integer", "description": "Max edges to return (default 10)" }
            },
            "required": ["subgraph"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: GraphSurprisingConnectionsArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_GRAPH_SURPRISING_CONNECTIONS,
                    "invalid_args",
                    &format!("{TOOL_GRAPH_SURPRISING_CONNECTIONS}: invalid args: {e}"),
                );
            }
        };

        let g = match require_graph(ctx, TOOL_GRAPH_SURPRISING_CONNECTIONS) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let root = match args.subgraph.root {
            Some(r) if !r.is_empty() => r,
            _ => {
                return err_envelope(
                    TOOL_GRAPH_SURPRISING_CONNECTIONS,
                    "missing_required_arg",
                    "graph_surprising_connections: missing required arg `subgraph.root`",
                );
            }
        };

        let direction = parse_direction(args.subgraph.direction.as_deref());
        let depth = args.subgraph.depth.unwrap_or(DEFAULT_SUBGRAPH_DEPTH);
        let limit = args.limit.unwrap_or(10);

        let view = extract_subgraph_view(g, &root, direction, depth);

        if view.nodes.is_empty() {
            return ok_envelope(
                TOOL_GRAPH_SURPRISING_CONNECTIONS,
                &serde_json::json!({ "edges": [] }),
            );
        }

        let sub_cg = build_subgraph_callgraph(&view);
        let community_result =
            CommunityDetector::detect(&sub_cg, CommunityDetector::MAX_ITERATIONS);
        let surprising =
            CommunityDetector::surprising_connections(&sub_cg, &community_result, limit);

        let edges: Vec<_> = surprising
            .into_iter()
            .map(|(src, dst, _sc, _tc)| {
                serde_json::json!({
                    "source_id": src.as_str().to_string(),
                    "target_id": dst.as_str().to_string(),
                    "score": 1.0
                })
            })
            .collect();
        let payload = serde_json::json!({ "edges": edges });
        ok_envelope(TOOL_GRAPH_SURPRISING_CONNECTIONS, &payload)
    }
}

// --- graph_transitive_reduction ---

struct GraphTransitiveReductionHandler;

#[async_trait]
impl ToolHandler for GraphTransitiveReductionHandler {
    fn name(&self) -> &'static str {
        TOOL_GRAPH_TRANSITIVE_REDUCTION
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "subgraph": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Root symbol id (required)" },
                        "depth": { "type": "integer", "description": "BFS depth (default 3)" },
                        "direction": { "type": "string", "enum": ["outgoing", "incoming", "both"] }
                    },
                    "required": ["root"]
                }
            },
            "required": ["subgraph"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: GraphTransitiveReductionArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_GRAPH_TRANSITIVE_REDUCTION,
                    "invalid_args",
                    &format!("{TOOL_GRAPH_TRANSITIVE_REDUCTION}: invalid args: {e}"),
                );
            }
        };

        let g = match require_graph(ctx, TOOL_GRAPH_TRANSITIVE_REDUCTION) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let root = match args.subgraph.root {
            Some(r) if !r.is_empty() => r,
            _ => {
                return err_envelope(
                    TOOL_GRAPH_TRANSITIVE_REDUCTION,
                    "missing_required_arg",
                    "graph_transitive_reduction: missing required arg `subgraph.root`",
                );
            }
        };

        let direction = parse_direction(args.subgraph.direction.as_deref());
        let depth = args.subgraph.depth.unwrap_or(DEFAULT_SUBGRAPH_DEPTH);

        let view = extract_subgraph_view(g, &root, direction, depth);

        if view.nodes.is_empty() {
            return ok_envelope(
                TOOL_GRAPH_TRANSITIVE_REDUCTION,
                &serde_json::json!({ "edges": [] }),
            );
        }

        let sub_cg = build_subgraph_callgraph(&view);
        let reduced = GraphAnalyticsService::transitive_reduction(&sub_cg);

        let edges: Vec<_> = reduced
            .into_iter()
            .map(|(src, dst)| {
                serde_json::json!({
                    "source_id": src.as_str().to_string(),
                    "target_id": dst.as_str().to_string()
                })
            })
            .collect();
        let payload = serde_json::json!({ "edges": edges });
        ok_envelope(TOOL_GRAPH_TRANSITIVE_REDUCTION, &payload)
    }
}

// --- graph_feedback_arc_set ---

struct GraphFeedbackArcSetHandler;

#[async_trait]
impl ToolHandler for GraphFeedbackArcSetHandler {
    fn name(&self) -> &'static str {
        TOOL_GRAPH_FEEDBACK_ARC_SET
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "subgraph": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Root symbol id (required)" },
                        "depth": { "type": "integer", "description": "BFS depth (default 3)" },
                        "direction": { "type": "string", "enum": ["outgoing", "incoming", "both"] }
                    },
                    "required": ["root"]
                }
            },
            "required": ["subgraph"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: GraphFeedbackArcSetArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_GRAPH_FEEDBACK_ARC_SET,
                    "invalid_args",
                    &format!("{TOOL_GRAPH_FEEDBACK_ARC_SET}: invalid args: {e}"),
                );
            }
        };

        let g = match require_graph(ctx, TOOL_GRAPH_FEEDBACK_ARC_SET) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let root = match args.subgraph.root {
            Some(r) if !r.is_empty() => r,
            _ => {
                return err_envelope(
                    TOOL_GRAPH_FEEDBACK_ARC_SET,
                    "missing_required_arg",
                    "graph_feedback_arc_set: missing required arg `subgraph.root`",
                );
            }
        };

        let direction = parse_direction(args.subgraph.direction.as_deref());
        let depth = args.subgraph.depth.unwrap_or(DEFAULT_SUBGRAPH_DEPTH);

        let view = extract_subgraph_view(g, &root, direction, depth);

        if view.nodes.is_empty() {
            return ok_envelope(
                TOOL_GRAPH_FEEDBACK_ARC_SET,
                &serde_json::json!({ "edges": [] }),
            );
        }

        let sub_cg = build_subgraph_callgraph(&view);
        let fas = GraphAnalyticsService::feedback_arc_set(&sub_cg);

        let edges: Vec<_> = fas
            .into_iter()
            .map(|(src, dst)| {
                serde_json::json!({
                    "source_id": src.as_str().to_string(),
                    "target_id": dst.as_str().to_string()
                })
            })
            .collect();
        let payload = serde_json::json!({ "edges": edges });
        ok_envelope(TOOL_GRAPH_FEEDBACK_ARC_SET, &payload)
    }
}

// --- graph_all_simple_paths ---

struct GraphAllSimplePathsHandler;

#[async_trait]
impl ToolHandler for GraphAllSimplePathsHandler {
    fn name(&self) -> &'static str {
        TOOL_GRAPH_ALL_SIMPLE_PATHS
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "subgraph": {
                    "type": "object",
                    "properties": {
                        "root": { "type": "string", "description": "Root symbol id (required)" },
                        "depth": { "type": "integer", "description": "BFS depth (default 3)" },
                        "direction": { "type": "string", "enum": ["outgoing", "incoming", "both"] }
                    },
                    "required": ["root"]
                },
                "from": { "type": "string", "description": "Source symbol id (required)" },
                "to": { "type": "string", "description": "Target symbol id (required)" },
                "max_hops": { "type": "integer", "description": "Max hops (default 10)" }
            },
            "required": ["subgraph", "from", "to"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: GraphAllSimplePathsArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_GRAPH_ALL_SIMPLE_PATHS,
                    "invalid_args",
                    &format!("{TOOL_GRAPH_ALL_SIMPLE_PATHS}: invalid args: {e}"),
                );
            }
        };

        let g = match require_graph(ctx, TOOL_GRAPH_ALL_SIMPLE_PATHS) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let root = match args.subgraph.root {
            Some(r) if !r.is_empty() => r,
            _ => {
                return err_envelope(
                    TOOL_GRAPH_ALL_SIMPLE_PATHS,
                    "missing_required_arg",
                    "graph_all_simple_paths: missing required arg `subgraph.root`",
                );
            }
        };

        let from = match args.from {
            Some(f) if !f.is_empty() => f,
            _ => {
                return err_envelope(
                    TOOL_GRAPH_ALL_SIMPLE_PATHS,
                    "missing_required_arg",
                    "graph_all_simple_paths: missing required arg `from`",
                );
            }
        };

        let to = match args.to {
            Some(t) if !t.is_empty() => t,
            _ => {
                return err_envelope(
                    TOOL_GRAPH_ALL_SIMPLE_PATHS,
                    "missing_required_arg",
                    "graph_all_simple_paths: missing required arg `to`",
                );
            }
        };

        let direction = parse_direction(args.subgraph.direction.as_deref());
        let depth = args.subgraph.depth.unwrap_or(DEFAULT_SUBGRAPH_DEPTH);
        let max_hops = args.max_hops.unwrap_or(10);

        let view = extract_subgraph_view(g, &root, direction, depth);

        if view.nodes.is_empty() {
            return ok_envelope(
                TOOL_GRAPH_ALL_SIMPLE_PATHS,
                &serde_json::json!({ "paths": [] }),
            );
        }

        let sub_cg = build_subgraph_callgraph(&view);
        let paths = GraphAnalyticsService::all_simple_paths(
            &sub_cg,
            &SymbolId::new(from),
            &SymbolId::new(to),
            max_hops,
        );

        let paths_str: Vec<Vec<String>> = paths
            .into_iter()
            .map(|p| p.into_iter().map(|s| s.as_str().to_string()).collect())
            .collect();
        let payload = serde_json::json!({ "paths": paths_str });
        ok_envelope(TOOL_GRAPH_ALL_SIMPLE_PATHS, &payload)
    }
}

// ============================================================================
// Registry builder
// ============================================================================

/// Register all 8 graph-analysis handlers into the registry.
pub fn register_graph_analyze_handlers(registry: &mut crate::mcp::handler::ToolHandlerRegistry) {
    registry.register(GraphPagerankHandler);
    registry.register(GraphGodNodesHandler);
    registry.register(GraphCommunitiesHandler);
    registry.register(GraphCommunityGodNodesHandler);
    registry.register(GraphSurprisingConnectionsHandler);
    registry.register(GraphTransitiveReductionHandler);
    registry.register(GraphFeedbackArcSetHandler);
    registry.register(GraphAllSimplePathsHandler);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::Content;

    // ------------------------------------------------------------------------
    // Tool names
    // ------------------------------------------------------------------------

    #[test]
    fn graph_pagerank_handler_name() {
        let h = GraphPagerankHandler;
        assert_eq!(h.name(), TOOL_GRAPH_PAGERANK);
    }

    #[test]
    fn graph_god_nodes_handler_name() {
        let h = GraphGodNodesHandler;
        assert_eq!(h.name(), TOOL_GRAPH_GOD_NODES);
    }

    #[test]
    fn graph_communities_handler_name() {
        let h = GraphCommunitiesHandler;
        assert_eq!(h.name(), TOOL_GRAPH_COMMUNITIES);
    }

    #[test]
    fn graph_community_god_nodes_handler_name() {
        let h = GraphCommunityGodNodesHandler;
        assert_eq!(h.name(), TOOL_GRAPH_COMMUNITY_GOD_NODES);
    }

    #[test]
    fn graph_surprising_connections_handler_name() {
        let h = GraphSurprisingConnectionsHandler;
        assert_eq!(h.name(), TOOL_GRAPH_SURPRISING_CONNECTIONS);
    }

    #[test]
    fn graph_transitive_reduction_handler_name() {
        let h = GraphTransitiveReductionHandler;
        assert_eq!(h.name(), TOOL_GRAPH_TRANSITIVE_REDUCTION);
    }

    #[test]
    fn graph_feedback_arc_set_handler_name() {
        let h = GraphFeedbackArcSetHandler;
        assert_eq!(h.name(), TOOL_GRAPH_FEEDBACK_ARC_SET);
    }

    #[test]
    fn graph_all_simple_paths_handler_name() {
        let h = GraphAllSimplePathsHandler;
        assert_eq!(h.name(), TOOL_GRAPH_ALL_SIMPLE_PATHS);
    }

    // ------------------------------------------------------------------------
    // Arg schema is valid JSON
    // ------------------------------------------------------------------------

    fn is_valid_json_schema(v: &serde_json::Value) -> bool {
        v.is_object() && v.get("type").is_some()
    }

    #[test]
    fn graph_pagerank_arg_schema_valid() {
        let h = GraphPagerankHandler;
        assert!(is_valid_json_schema(&h.arg_schema()));
    }

    #[test]
    fn graph_god_nodes_arg_schema_valid() {
        let h = GraphGodNodesHandler;
        assert!(is_valid_json_schema(&h.arg_schema()));
    }

    #[test]
    fn graph_communities_arg_schema_valid() {
        let h = GraphCommunitiesHandler;
        assert!(is_valid_json_schema(&h.arg_schema()));
    }

    #[test]
    fn graph_community_god_nodes_arg_schema_valid() {
        let h = GraphCommunityGodNodesHandler;
        assert!(is_valid_json_schema(&h.arg_schema()));
    }

    #[test]
    fn graph_surprising_connections_arg_schema_valid() {
        let h = GraphSurprisingConnectionsHandler;
        assert!(is_valid_json_schema(&h.arg_schema()));
    }

    #[test]
    fn graph_transitive_reduction_arg_schema_valid() {
        let h = GraphTransitiveReductionHandler;
        assert!(is_valid_json_schema(&h.arg_schema()));
    }

    #[test]
    fn graph_feedback_arc_set_arg_schema_valid() {
        let h = GraphFeedbackArcSetHandler;
        assert!(is_valid_json_schema(&h.arg_schema()));
    }

    #[test]
    fn graph_all_simple_paths_arg_schema_valid() {
        let h = GraphAllSimplePathsHandler;
        assert!(is_valid_json_schema(&h.arg_schema()));
    }

    // ------------------------------------------------------------------------
    // Arg parsing — valid args deserialize without error
    // ------------------------------------------------------------------------

    #[test]
    fn graph_pagerank_args_parse() {
        let json = serde_json::json!({
            "subgraph": { "root": "sym:foo:1", "depth": 2, "direction": "outgoing" },
            "options": { "alpha": 0.9, "max_iterations": 50 }
        });
        let args: GraphPagerankArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.subgraph.root, Some("sym:foo:1".to_string()));
        assert_eq!(args.subgraph.depth, Some(2));
        assert_eq!(args.subgraph.direction, Some("outgoing".to_string()));
        assert_eq!(args.options.as_ref().unwrap().alpha, Some(0.9));
        assert_eq!(args.options.as_ref().unwrap().max_iterations, Some(50));
    }

    #[test]
    fn graph_god_nodes_args_parse() {
        let json = serde_json::json!({
            "subgraph": { "root": "sym:foo:1", "depth": 3 },
            "percentile": 0.99
        });
        let args: GraphGodNodesArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.subgraph.root, Some("sym:foo:1".to_string()));
        assert_eq!(args.percentile, Some(0.99));
    }

    #[test]
    fn graph_communities_args_parse() {
        let json = serde_json::json!({
            "subgraph": { "root": "sym:foo:1", "direction": "both" },
            "max_iterations": 50
        });
        let args: GraphCommunitiesArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.subgraph.direction, Some("both".to_string()));
        assert_eq!(args.max_iterations, Some(50));
    }

    #[test]
    fn graph_all_simple_paths_args_parse() {
        let json = serde_json::json!({
            "subgraph": { "root": "sym:a:1", "depth": 3 },
            "from": "sym:a:1",
            "to": "sym:c:1",
            "max_hops": 5
        });
        let args: GraphAllSimplePathsArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.from, Some("sym:a:1".to_string()));
        assert_eq!(args.to, Some("sym:c:1".to_string()));
        assert_eq!(args.max_hops, Some(5));
    }

    // ------------------------------------------------------------------------
    // Error: graph unavailable
    // ------------------------------------------------------------------------

    fn extract_error_code(result: &CallToolResult) -> Option<String> {
        let items = &result.content;
        if items.is_empty() {
            return None;
        }
        let Content {
            raw: rmcp::model::RawContent::Text(text),
            annotations: _,
        } = &items[0]
        else {
            return None;
        };
        let parsed: serde_json::Value = serde_json::from_str(&text.text).ok()?;
        parsed
            .get("payload")?
            .get("error_code")?
            .as_str()
            .map(String::from)
    }

    #[tokio::test]
    async fn graph_pagerank_requires_graph() {
        let h = GraphPagerankHandler;
        // McpContext with no graph
        let ctx = McpContext::builder().build();
        let result = h.handle(&ctx, serde_json::json!({})).await;
        assert!(
            result.is_error == Some(true),
            "must return error when graph unavailable"
        );
        assert_eq!(
            extract_error_code(&result).as_deref(),
            Some("graph_unavailable")
        );
    }

    #[tokio::test]
    async fn graph_god_nodes_requires_graph() {
        let h = GraphGodNodesHandler;
        let ctx = McpContext::builder().build();
        let result = h.handle(&ctx, serde_json::json!({})).await;
        assert!(result.is_error == Some(true));
        assert_eq!(
            extract_error_code(&result).as_deref(),
            Some("graph_unavailable")
        );
    }

    #[tokio::test]
    async fn graph_communities_requires_graph() {
        let h = GraphCommunitiesHandler;
        let ctx = McpContext::builder().build();
        let result = h.handle(&ctx, serde_json::json!({})).await;
        assert!(result.is_error == Some(true));
        assert_eq!(
            extract_error_code(&result).as_deref(),
            Some("graph_unavailable")
        );
    }

    #[tokio::test]
    async fn graph_community_god_nodes_requires_graph() {
        let h = GraphCommunityGodNodesHandler;
        let ctx = McpContext::builder().build();
        let result = h.handle(&ctx, serde_json::json!({})).await;
        assert!(result.is_error == Some(true));
        assert_eq!(
            extract_error_code(&result).as_deref(),
            Some("graph_unavailable")
        );
    }

    #[tokio::test]
    async fn graph_surprising_connections_requires_graph() {
        let h = GraphSurprisingConnectionsHandler;
        let ctx = McpContext::builder().build();
        let result = h.handle(&ctx, serde_json::json!({})).await;
        assert!(result.is_error == Some(true));
        assert_eq!(
            extract_error_code(&result).as_deref(),
            Some("graph_unavailable")
        );
    }

    #[tokio::test]
    async fn graph_transitive_reduction_requires_graph() {
        let h = GraphTransitiveReductionHandler;
        let ctx = McpContext::builder().build();
        let result = h.handle(&ctx, serde_json::json!({})).await;
        assert!(result.is_error == Some(true));
        assert_eq!(
            extract_error_code(&result).as_deref(),
            Some("graph_unavailable")
        );
    }

    #[tokio::test]
    async fn graph_feedback_arc_set_requires_graph() {
        let h = GraphFeedbackArcSetHandler;
        let ctx = McpContext::builder().build();
        let result = h.handle(&ctx, serde_json::json!({})).await;
        assert!(result.is_error == Some(true));
        assert_eq!(
            extract_error_code(&result).as_deref(),
            Some("graph_unavailable")
        );
    }

    #[tokio::test]
    async fn graph_all_simple_paths_requires_graph() {
        let h = GraphAllSimplePathsHandler;
        let ctx = McpContext::builder().build();
        let result = h.handle(&ctx, serde_json::json!({})).await;
        assert!(result.is_error == Some(true));
        assert_eq!(
            extract_error_code(&result).as_deref(),
            Some("graph_unavailable")
        );
    }

    // ------------------------------------------------------------------------
    // Error: missing required arg
    // ------------------------------------------------------------------------

    fn build_test_callgraph() -> CallGraph {
        let mut cg = CallGraph::new();
        let sym_a = Symbol::new("a", SymbolKind::Function, Location::new("test.rs", 1, 1));
        let sym_b = Symbol::new("b", SymbolKind::Function, Location::new("test.rs", 2, 1));
        cg.add_symbol(sym_a);
        cg.add_symbol(sym_b);
        let _ = cg.add_dependency_with_provenance(
            &SymbolId::new("test.rs:a:1"),
            &SymbolId::new("test.rs:b:1"),
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
        cg
    }

    fn ctx_with_graph() -> McpContext {
        let cg = Arc::new(build_test_callgraph());
        McpContext::builder().with_graph(Some(cg)).build()
    }

    #[tokio::test]
    async fn graph_pagerank_missing_root() {
        let h = GraphPagerankHandler;
        let ctx = ctx_with_graph();
        let result = h.handle(&ctx, serde_json::json!({ "subgraph": {} })).await;
        assert!(result.is_error == Some(true));
        assert_eq!(
            extract_error_code(&result).as_deref(),
            Some("missing_required_arg")
        );
    }

    #[tokio::test]
    async fn graph_god_nodes_missing_root() {
        let h = GraphGodNodesHandler;
        let ctx = ctx_with_graph();
        let result = h.handle(&ctx, serde_json::json!({ "subgraph": {} })).await;
        assert!(result.is_error == Some(true));
        assert_eq!(
            extract_error_code(&result).as_deref(),
            Some("missing_required_arg")
        );
    }

    #[tokio::test]
    async fn graph_all_simple_paths_missing_from() {
        let h = GraphAllSimplePathsHandler;
        let ctx = ctx_with_graph();
        let result = h
            .handle(
                &ctx,
                serde_json::json!({
                    "subgraph": { "root": "test.rs:a:1" },
                    "to": "test.rs:b:1"
                }),
            )
            .await;
        assert!(result.is_error == Some(true));
        assert_eq!(
            extract_error_code(&result).as_deref(),
            Some("missing_required_arg")
        );
    }

    #[tokio::test]
    async fn graph_all_simple_paths_missing_to() {
        let h = GraphAllSimplePathsHandler;
        let ctx = ctx_with_graph();
        let result = h
            .handle(
                &ctx,
                serde_json::json!({
                    "subgraph": { "root": "test.rs:a:1" },
                    "from": "test.rs:a:1"
                }),
            )
            .await;
        assert!(result.is_error == Some(true));
        assert_eq!(
            extract_error_code(&result).as_deref(),
            Some("missing_required_arg")
        );
    }

    // ------------------------------------------------------------------------
    // Happy path: valid subgraph args return success (empty result for unknown root)
    // ------------------------------------------------------------------------

    #[tokio::test]
    async fn graph_pagerank_unknown_root_returns_empty_scores() {
        let h = GraphPagerankHandler;
        let ctx = ctx_with_graph();
        let result = h
            .handle(
                &ctx,
                serde_json::json!({
                    "subgraph": { "root": "test.rs:unknown:1" }
                }),
            )
            .await;
        assert!(
            result.is_error == Some(false),
            "unknown root should not error, just return empty"
        );
        let items = &result.content;
        let Content {
            raw: rmcp::model::RawContent::Text(text),
            annotations: _,
        } = &items[0]
        else {
            panic!("expected Content::Text");
        };
        let parsed: serde_json::Value = serde_json::from_str(&text.text).unwrap();
        assert!(parsed.get("payload").is_some());
        let scores = parsed["payload"]["scores"].as_object().unwrap();
        assert!(scores.is_empty(), "unknown root should yield empty scores");
    }

    // ------------------------------------------------------------------------
    // SubgraphArgs direction parsing
    // ------------------------------------------------------------------------

    #[test]
    fn subgraph_direction_outgoing() {
        let args: SubgraphArgs = serde_json::from_value(serde_json::json!({
            "root": "sym:foo:1",
            "direction": "outgoing"
        }))
        .unwrap();
        assert_eq!(args.direction, Some("outgoing".to_string()));
    }

    #[test]
    fn subgraph_direction_incoming() {
        let args: SubgraphArgs = serde_json::from_value(serde_json::json!({
            "root": "sym:foo:1",
            "direction": "incoming"
        }))
        .unwrap();
        assert_eq!(args.direction, Some("incoming".to_string()));
    }

    #[test]
    fn subgraph_direction_both() {
        let args: SubgraphArgs = serde_json::from_value(serde_json::json!({
            "root": "sym:foo:1",
            "direction": "both"
        }))
        .unwrap();
        assert_eq!(args.direction, Some("both".to_string()));
    }

    // ------------------------------------------------------------------------
    // PagerankOptions defaults
    // ------------------------------------------------------------------------

    #[test]
    fn pagerank_options_optional() {
        let json = serde_json::json!({ "subgraph": { "root": "sym:foo:1" } });
        let args: GraphPagerankArgs = serde_json::from_value(json).unwrap();
        assert!(args.options.is_none());
    }

    #[test]
    fn pagerank_options_partial() {
        let json = serde_json::json!({
            "subgraph": { "root": "sym:foo:1" },
            "options": { "alpha": 0.9 }
        });
        let args: GraphPagerankArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.options.as_ref().unwrap().alpha, Some(0.9));
        assert!(args.options.as_ref().unwrap().max_iterations.is_none());
    }
}
