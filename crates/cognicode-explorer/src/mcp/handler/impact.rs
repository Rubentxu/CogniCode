//! Impact-analysis tool handlers.
//!
//! Implements 6 MCP tools for call-graph impact analysis:
//! - `impact_radius`          — predecessor (reverse) BFS from a root symbol
//! - `impact_forward_radius` — successor (forward) BFS from a root symbol
//! - `impact_has_path`       — check if a directed path exists between two symbols
//! - `impact_shortest_path`  — compute lowest-cost path between two symbols
//! - `impact_detect_cycles`  — find all non-trivial strongly connected components
//! - `impact_component`      — return the undirected connected component containing a symbol

use std::sync::Arc;

use async_trait::async_trait;
use cognicode_core::application::dto::SccDto;
use cognicode_core::application::services::impact_analysis::ImpactAnalysisService;
use cognicode_core::domain::aggregates::SymbolId;
use rmcp::model::{CallToolResult, Content};
use serde::Deserialize;
use serde_json::Value;

use crate::mcp::envelope::{err_envelope, ok_envelope};
use crate::mcp::handler::ToolHandler;
use crate::mcp::{
    McpContext, DEFAULT_IMPACT_RADIUS_DEPTH,
    TOOL_IMPACT_COMPONENT, TOOL_IMPACT_DETECT_CYCLES, TOOL_IMPACT_FORWARD_RADIUS,
    TOOL_IMPACT_HAS_PATH, TOOL_IMPACT_RADIUS, TOOL_IMPACT_SHORTEST_PATH,
};

// ============================================================================
// Arg structs
// ============================================================================

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ImpactRadiusArgs {
    root: Option<String>,
    max_depth: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ImpactForwardRadiusArgs {
    root: Option<String>,
    max_depth: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ImpactEndpointsArgs {
    from: Option<String>,
    to: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ImpactIdArgs {
    id: Option<String>,
}

// ============================================================================
// require_graph — shared guard for tools that need a loaded call graph
// ============================================================================

/// Guard: require a call graph or return a structured error envelope.
fn require_graph<'a>(
    ctx: &'a McpContext,
    tool: &str,
) -> Result<&'a Arc<cognicode_core::domain::aggregates::CallGraph>, CallToolResult> {
    ctx.graph.as_ref().ok_or_else(|| {
        err_envelope(
            tool,
            "graph_unavailable",
            &format!("{tool}: impact analysis unavailable — no call graph loaded"),
        )
    })
}

// ============================================================================
// ToolHandler implementations
// ============================================================================

// --- impact_radius ---

struct ImpactRadiusHandler;

#[async_trait]
impl ToolHandler for ImpactRadiusHandler {
    fn name(&self) -> &'static str {
        TOOL_IMPACT_RADIUS
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "root": {
                    "type": "string",
                    "description": "Symbol id to analyze (required). Use the `symbol:{file}:{name}:{line}` form."
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum reverse BFS depth. Omit to default to 5."
                }
            },
            "required": ["root"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: ImpactRadiusArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return err_envelope(TOOL_IMPACT_RADIUS, "invalid_args",
                &format!("{TOOL_IMPACT_RADIUS}: invalid args: {e}")),
        };

        let g = match require_graph(ctx, TOOL_IMPACT_RADIUS) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let root = match args.root {
            Some(r) if !r.is_empty() => r,
            _ => return err_envelope(TOOL_IMPACT_RADIUS, "missing_required_arg",
                "impact_radius: missing required arg `root`"),
        };

        let max_depth = args.max_depth.unwrap_or(DEFAULT_IMPACT_RADIUS_DEPTH);
        let svc = ImpactAnalysisService::new();
        let ids = svc.impact_radius(g, &SymbolId::new(root), max_depth);
        let strings: Vec<String> = ids.iter().map(|s| s.as_str().to_string()).collect();
        ok_envelope(TOOL_IMPACT_RADIUS, &strings)
    }
}

// --- impact_forward_radius ---

struct ImpactForwardRadiusHandler;

#[async_trait]
impl ToolHandler for ImpactForwardRadiusHandler {
    fn name(&self) -> &'static str {
        TOOL_IMPACT_FORWARD_RADIUS
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "root": {
                    "type": "string",
                    "description": "Symbol id to analyze (required)."
                },
                "max_depth": {
                    "type": "integer",
                    "description": "Maximum forward BFS depth. Omit to default to 5."
                }
            },
            "required": ["root"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: ImpactForwardRadiusArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return err_envelope(TOOL_IMPACT_FORWARD_RADIUS, "invalid_args",
                &format!("{TOOL_IMPACT_FORWARD_RADIUS}: invalid args: {e}")),
        };

        let g = match require_graph(ctx, TOOL_IMPACT_FORWARD_RADIUS) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let root = match args.root {
            Some(r) if !r.is_empty() => r,
            _ => return err_envelope(TOOL_IMPACT_FORWARD_RADIUS, "missing_required_arg",
                "impact_forward_radius: missing required arg `root`"),
        };

        let max_depth = args.max_depth.unwrap_or(DEFAULT_IMPACT_RADIUS_DEPTH);
        let svc = ImpactAnalysisService::new();
        let ids = svc.forward_radius(g, &SymbolId::new(root), max_depth);
        let strings: Vec<String> = ids.iter().map(|s| s.as_str().to_string()).collect();
        ok_envelope(TOOL_IMPACT_FORWARD_RADIUS, &strings)
    }
}

// --- impact_has_path ---

struct ImpactHasPathHandler;

#[async_trait]
impl ToolHandler for ImpactHasPathHandler {
    fn name(&self) -> &'static str {
        TOOL_IMPACT_HAS_PATH
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
        let args: ImpactEndpointsArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return err_envelope(TOOL_IMPACT_HAS_PATH, "invalid_args",
                &format!("{TOOL_IMPACT_HAS_PATH}: invalid args: {e}")),
        };

        let g = match require_graph(ctx, TOOL_IMPACT_HAS_PATH) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let from = match args.from {
            Some(v) if !v.is_empty() => v,
            _ => return err_envelope(TOOL_IMPACT_HAS_PATH, "missing_required_arg",
                "impact_has_path: missing required arg `from`"),
        };

        let to = match args.to {
            Some(v) if !v.is_empty() => v,
            _ => return err_envelope(TOOL_IMPACT_HAS_PATH, "missing_required_arg",
                "impact_has_path: missing required arg `to`"),
        };

        let svc = ImpactAnalysisService::new();
        let has_path = svc.has_path(g, &SymbolId::new(from.clone()), &SymbolId::new(to.clone()));
        let result = serde_json::json!({
            "from": from,
            "to": to,
            "has_path": has_path,
        });
        ok_envelope(TOOL_IMPACT_HAS_PATH, &result)
    }
}

// --- impact_shortest_path ---

struct ImpactShortestPathHandler;

#[async_trait]
impl ToolHandler for ImpactShortestPathHandler {
    fn name(&self) -> &'static str {
        TOOL_IMPACT_SHORTEST_PATH
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
        let args: ImpactEndpointsArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return err_envelope(TOOL_IMPACT_SHORTEST_PATH, "invalid_args",
                &format!("{TOOL_IMPACT_SHORTEST_PATH}: invalid args: {e}")),
        };

        let g = match require_graph(ctx, TOOL_IMPACT_SHORTEST_PATH) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let from = match args.from {
            Some(v) if !v.is_empty() => v,
            _ => return err_envelope(TOOL_IMPACT_SHORTEST_PATH, "missing_required_arg",
                "impact_shortest_path: missing required arg `from`"),
        };

        let to = match args.to {
            Some(v) if !v.is_empty() => v,
            _ => return err_envelope(TOOL_IMPACT_SHORTEST_PATH, "missing_required_arg",
                "impact_shortest_path: missing required arg `to`"),
        };

        let svc = ImpactAnalysisService::new();
        let result = svc.shortest_path(g, &SymbolId::new(from), &SymbolId::new(to));
        ok_envelope(TOOL_IMPACT_SHORTEST_PATH, &result)
    }
}

// --- impact_detect_cycles ---

struct ImpactDetectCyclesHandler;

#[async_trait]
impl ToolHandler for ImpactDetectCyclesHandler {
    fn name(&self) -> &'static str {
        TOOL_IMPACT_DETECT_CYCLES
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let _ = params; // empty-valid

        let g = match require_graph(ctx, TOOL_IMPACT_DETECT_CYCLES) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let svc = ImpactAnalysisService::new();
        let sccs = svc.detect_cycles(g);
        let dtos: Vec<SccDto> = sccs.into_iter().map(SccDto::from_scc).collect();
        ok_envelope(TOOL_IMPACT_DETECT_CYCLES, &dtos)
    }
}

// --- impact_component ---

struct ImpactComponentHandler;

#[async_trait]
impl ToolHandler for ImpactComponentHandler {
    fn name(&self) -> &'static str {
        TOOL_IMPACT_COMPONENT
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Symbol id whose undirected component to return (required)."
                }
            },
            "required": ["id"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: ImpactIdArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return err_envelope(TOOL_IMPACT_COMPONENT, "invalid_args",
                &format!("{TOOL_IMPACT_COMPONENT}: invalid args: {e}")),
        };

        let g = match require_graph(ctx, TOOL_IMPACT_COMPONENT) {
            Ok(g) => g,
            Err(e) => return e,
        };

        let id = match args.id {
            Some(v) if !v.is_empty() => v,
            _ => return err_envelope(TOOL_IMPACT_COMPONENT, "missing_required_arg",
                "impact_component: missing required arg `id`"),
        };

        let svc = ImpactAnalysisService::new();
        let component = svc.containing_component(g, &SymbolId::new(id));
        let as_strings: Option<Vec<String>> =
            component.map(|members| members.iter().map(|s| s.as_str().to_string()).collect());
        ok_envelope(TOOL_IMPACT_COMPONENT, &as_strings)
    }
}

// ============================================================================
// Registry builder
// ============================================================================

/// Register all 6 impact-family handlers into the registry.
pub fn register_impact_handlers(registry: &mut crate::mcp::handler::ToolHandlerRegistry) {
    registry.register(ImpactRadiusHandler);
    registry.register(ImpactForwardRadiusHandler);
    registry.register(ImpactHasPathHandler);
    registry.register(ImpactShortestPathHandler);
    registry.register(ImpactDetectCyclesHandler);
    registry.register(ImpactComponentHandler);
}
