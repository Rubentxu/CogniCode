//! Graph-primitives tool handlers.
//!
//! Implements 3 MCP tools for call-graph structural queries:
//! - `graph_subgraph` — extract a bounded neighborhood subgraph
//! - `graph_cluster`  — cluster the graph by SCC or connected components
//! - `graph_explain`  — explain the lowest-cost path between two symbols

use std::sync::Arc;

use async_trait::async_trait;
use cognicode_core::application::services::impact_analysis::ImpactAnalysisService;
use cognicode_core::domain::aggregates::SymbolId;
use cognicode_core::infrastructure::graph::SubgraphDirection;
use rmcp::model::{CallToolResult, Content};
use serde::Deserialize;
use serde_json::Value;

use crate::mcp::handler::ToolHandler;
use crate::mcp::{
    McpContext, DEFAULT_SUBGRAPH_DEPTH, TOOL_GRAPH_CLUSTER, TOOL_GRAPH_EXPLAIN,
    TOOL_GRAPH_SUBGRAPH,
};

// ============================================================================
// Arg structs
// ============================================================================

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GraphSubgraphArgs {
    root: Option<String>,
    direction: Option<String>,
    max_depth: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GraphClusterArgs {
    method: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GraphExplainArgs {
    from: Option<String>,
    to: Option<String>,
}

// ============================================================================
// Envelope helpers
// ============================================================================

fn ok_envelope<T: serde::Serialize>(tool_name: &str, value: &T) -> CallToolResult {
    let envelope = serde_json::json!({
        "tool_name": tool_name,
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "provenance": serde_json::Value::Null,
        "payload": value,
        "suggested_follow_ups": serde_json::Value::Array(Vec::new()),
    });
    let pretty = serde_json::to_string_pretty(&envelope)
        .unwrap_or_else(|e| format!("failed to serialize envelope: {e}"));
    CallToolResult::success(vec![Content::text(pretty)])
}

fn err_envelope(tool_name: &str, code: &str, message: &str) -> CallToolResult {
    let envelope = serde_json::json!({
        "tool_name": tool_name,
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "provenance": serde_json::Value::Null,
        "payload": {
            "error_code": code,
            "error": message,
        },
        "suggested_follow_ups": serde_json::Value::Array(Vec::new()),
    });
    let pretty = serde_json::to_string_pretty(&envelope)
        .unwrap_or_else(|e| format!("failed to serialize envelope: {e}"));
    CallToolResult::error(vec![Content::text(pretty)])
}

fn plain_err(message: String) -> CallToolResult {
    CallToolResult::error(vec![Content::text(message)])
}

/// Guard: require a call graph or return a structured error.
fn require_graph<'a>(ctx: &'a McpContext, tool: &str) -> Result<&'a Arc<cognicode_core::domain::aggregates::CallGraph>, CallToolResult> {
    ctx.graph.as_ref().ok_or_else(|| {
        plain_err(format!(
            "{tool}: impact analysis unavailable — no call graph loaded"
        ))
    })
}

// ============================================================================
// ToolHandler implementations
// ============================================================================

// --- graph_subgraph ---

struct GraphSubgraphHandler;

#[async_trait]
impl ToolHandler for GraphSubgraphHandler {
    fn name(&self) -> &'static str {
        TOOL_GRAPH_SUBGRAPH
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "root": {
                    "type": "string",
                    "description": "Symbol id of the subgraph root (required)."
                },
                "direction": {
                    "type": "string",
                    "enum": ["incoming", "outgoing", "both"],
                    "description": "Edge direction to walk. Omit to default to `both`."
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum BFS depth. Omit to default to 3."
                }
            },
            "required": ["root"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: GraphSubgraphArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return err_envelope(TOOL_GRAPH_SUBGRAPH, "invalid_args",
                &format!("{TOOL_GRAPH_SUBGRAPH}: invalid args: {e}")),
        };

        let g = match require_graph(ctx, TOOL_GRAPH_SUBGRAPH) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let root = match args.root {
            Some(r) if !r.is_empty() => r,
            _ => return err_envelope(TOOL_GRAPH_SUBGRAPH, "missing_required_arg",
                "graph_subgraph: missing required arg `root`"),
        };

        let direction_str = args.direction.as_deref().unwrap_or("both");
        let direction = match direction_str {
            "outgoing" => SubgraphDirection::Outgoing,
            "incoming" => SubgraphDirection::Incoming,
            "both" => SubgraphDirection::Both,
            other => return err_envelope(TOOL_GRAPH_SUBGRAPH, "invalid_input",
                &format!("graph_subgraph: invalid `direction` `{other}` (expected one of: outgoing, incoming, both)")),
        };

        let max_depth = args.max_depth.unwrap_or(DEFAULT_SUBGRAPH_DEPTH);
        let svc = ImpactAnalysisService::new();
        let dto = svc.subgraph(g, &SymbolId::new(root), direction, max_depth);
        ok_envelope(TOOL_GRAPH_SUBGRAPH, &dto)
    }
}

// --- graph_cluster ---

struct GraphClusterHandler;

#[async_trait]
impl ToolHandler for GraphClusterHandler {
    fn name(&self) -> &'static str {
        TOOL_GRAPH_CLUSTER
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "method": {
                    "type": "string",
                    "enum": ["scc", "connected"],
                    "description": "Cluster method. Omit to default to `scc`."
                }
            },
            "required": []
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: GraphClusterArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return err_envelope(TOOL_GRAPH_CLUSTER, "invalid_args",
                &format!("{TOOL_GRAPH_CLUSTER}: invalid args: {e}")),
        };

        let g = match require_graph(ctx, TOOL_GRAPH_CLUSTER) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let method = args.method.as_deref().unwrap_or("scc");
        if method != "scc" && method != "connected" {
            return err_envelope(TOOL_GRAPH_CLUSTER, "invalid_input",
                &format!("graph_cluster: invalid `method` `{method}` (expected one of: scc, connected)"));
        }

        let svc = ImpactAnalysisService::new();
        let dto = svc.cluster_components(g, method);
        ok_envelope(TOOL_GRAPH_CLUSTER, &dto)
    }
}

// --- graph_explain ---

struct GraphExplainHandler;

#[async_trait]
impl ToolHandler for GraphExplainHandler {
    fn name(&self) -> &'static str {
        TOOL_GRAPH_EXPLAIN
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "from": {
                    "type": "string",
                    "description": "Source symbol id (required)."
                },
                "to": {
                    "type": "string",
                    "description": "Target symbol id (required)."
                }
            },
            "required": ["from", "to"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: GraphExplainArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return err_envelope(TOOL_GRAPH_EXPLAIN, "invalid_args",
                &format!("{TOOL_GRAPH_EXPLAIN}: invalid args: {e}")),
        };

        let g = match require_graph(ctx, TOOL_GRAPH_EXPLAIN) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let from = match args.from {
            Some(v) if !v.is_empty() => v,
            _ => return err_envelope(TOOL_GRAPH_EXPLAIN, "missing_required_arg",
                "graph_explain: missing required arg `from`"),
        };

        let to = match args.to {
            Some(v) if !v.is_empty() => v,
            _ => return err_envelope(TOOL_GRAPH_EXPLAIN, "missing_required_arg",
                "graph_explain: missing required arg `to`"),
        };

        let svc = ImpactAnalysisService::new();
        // Service guarantees Some (wraps None as found:false).
        let dto = svc
            .explain_path(g, &SymbolId::new(from), &SymbolId::new(to))
            .expect("service.explain_path always returns Some");
        ok_envelope(TOOL_GRAPH_EXPLAIN, &dto)
    }
}

// ============================================================================
// Registry builder
// ============================================================================

/// Register all 3 graph-primitive handlers into the registry.
pub fn register_graph_handlers(registry: &mut crate::mcp::handler::ToolHandlerRegistry) {
    registry.register(GraphSubgraphHandler);
    registry.register(GraphClusterHandler);
    registry.register(GraphExplainHandler);
}
