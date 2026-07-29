//! Analytics tool handlers exposing the AlgorithmRegistry via MCP.
//!
//! Implements 4 MCP tools:
//! - `analytics_run`              — run an admitted algorithm
//! - `analytics_catalog`          — list all admitted algorithm descriptors
//! - `analytics_lineage_list`     — list lineage records
//! - `analytics_lineage_get`     — get a specific lineage record by run ID

use async_trait::async_trait;
use rmcp::model::CallToolResult;
use serde::Deserialize;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::dto::{
    AnalyticsCatalogResponse, AnalyticsLineageDetailResponse, AnalyticsLineageResponse,
    AlgorithmDescriptorSummary, LineageEntry, RunAnalyticsRequest, RunAnalyticsResponse,
};
use crate::mcp::envelope::{err_envelope, ok_envelope};
use crate::mcp::handler::ToolHandler;
use crate::mcp::McpContext;

// ============================================================================
// Arg structs
// ============================================================================

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct AnalyticsRunArgs {
    algorithm_id: Option<String>,
    from_symbol: Option<String>,
    to_symbol: Option<String>,
    max_hops: Option<usize>,
    caller_capabilities: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct AnalyticsCatalogArgs {}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct AnalyticsLineageListArgs {
    limit: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct AnalyticsLineageGetArgs {
    run_id: Option<String>,
}

// ============================================================================
// Tool constants
// ============================================================================

const TOOL_ANALYTICS_RUN: &str = "analytics_run";
const TOOL_ANALYTICS_CATALOG: &str = "analytics_catalog";
const TOOL_ANALYTICS_LINEAGE_LIST: &str = "analytics_lineage_list";
const TOOL_ANALYTICS_LINEAGE_GET: &str = "analytics_lineage_get";

// ============================================================================
// AnalyticsRun handler
// ============================================================================

struct AnalyticsRunHandler;

#[async_trait]
impl ToolHandler for AnalyticsRunHandler {
    fn name(&self) -> &'static str {
        TOOL_ANALYTICS_RUN
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "algorithm_id": {
                    "type": "string",
                    "description": "Algorithm identifier (e.g. bounded_shortest_paths)"
                },
                "from_symbol": {
                    "type": "string",
                    "description": "Source symbol name for path-based algorithms"
                },
                "to_symbol": {
                    "type": "string",
                    "description": "Target symbol name for path-based algorithms"
                },
                "max_hops": {
                    "type": "integer",
                    "description": "Maximum hops for bounded shortest paths"
                },
                "caller_capabilities": {
                    "type": "string",
                    "description": "Caller authorization level (Internal, TrustedREST, ExternalMCP, Explorer)"
                }
            },
            "required": ["algorithm_id", "from_symbol", "to_symbol"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: AnalyticsRunArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_ANALYTICS_RUN,
                    "invalid_args",
                    &format!("{TOOL_ANALYTICS_RUN}: invalid args: {e}"),
                );
            }
        };

        let algorithm_id = match &args.algorithm_id {
            Some(id) if !id.is_empty() => id.clone(),
            _ => {
                return err_envelope(
                    TOOL_ANALYTICS_RUN,
                    "missing_required_arg",
                    "analytics_run: missing required arg `algorithm_id`",
                );
            }
        };

        let from_symbol = match &args.from_symbol {
            Some(s) if !s.is_empty() => s.clone(),
            _ => {
                return err_envelope(
                    TOOL_ANALYTICS_RUN,
                    "missing_required_arg",
                    "analytics_run: missing required arg `from_symbol`",
                );
            }
        };

        let to_symbol = match &args.to_symbol {
            Some(s) if !s.is_empty() => s.clone(),
            _ => {
                return err_envelope(
                    TOOL_ANALYTICS_RUN,
                    "missing_required_arg",
                    "analytics_run: missing required arg `to_symbol`",
                );
            }
        };

        // Validate algorithm_id format - from_string panics on empty string
        if algorithm_id.is_empty() {
            return err_envelope(
                TOOL_ANALYTICS_RUN,
                "unknown_algorithm",
                "analytics_run: algorithm_id cannot be empty",
            );
        }
        let _algorithm_id_parsed =
            cognicode_core::domain::analytics::descriptor::AlgorithmId::from_string(&algorithm_id);

        // Require graph for algorithm execution
        let graph = match ctx.graph.as_ref() {
            Some(g) => g,
            None => {
                return err_envelope(
                    TOOL_ANALYTICS_RUN,
                    "graph_not_available",
                    "analytics_run: no call graph loaded",
                );
            }
        };

        let max_hops = args.max_hops.unwrap_or(5);

        // Execute BSP using GraphAnalyticsService
        let from_id = cognicode_core::domain::aggregates::SymbolId::new(from_symbol.clone());
        let to_id = cognicode_core::domain::aggregates::SymbolId::new(to_symbol.clone());

        let paths = cognicode_core::application::services::graph_analytics::GraphAnalyticsService::all_simple_paths(
            graph,
            &from_id,
            &to_id,
            max_hops,
        );

        let path_summaries: Vec<_> = paths
            .iter()
            .take(100)
            .map(|path| {
                path.iter()
                    .map(|sid| sid.as_str().to_string())
                    .collect::<Vec<_>>()
            })
            .collect();

        let run_id = format!(
            "run_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let executed_at = chrono::Utc::now().to_rfc3339();

        let result = serde_json::json!({
            "algorithm_id": algorithm_id,
            "from_symbol": from_symbol,
            "to_symbol": to_symbol,
            "max_hops": max_hops,
            "paths_found": paths.len(),
            "paths": path_summaries,
        });

        let response = RunAnalyticsResponse {
            algorithm_id,
            run_id: run_id.clone(),
            executed_at,
            lineage_persisted: false,
            result,
        };

        ok_envelope(TOOL_ANALYTICS_RUN, &response)
    }
}

// ============================================================================
// AnalyticsCatalog handler
// ============================================================================

struct AnalyticsCatalogHandler;

#[async_trait]
impl ToolHandler for AnalyticsCatalogHandler {
    fn name(&self) -> &'static str {
        TOOL_ANALYTICS_CATALOG
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    async fn handle(&self, _ctx: &McpContext, _params: Value) -> CallToolResult {
        // Return stub catalog - in production this would query the AlgorithmRegistry
        let algorithms = vec![
            AlgorithmDescriptorSummary {
                id: "bounded_shortest_paths".to_string(),
                name: "Bounded Shortest Paths".to_string(),
                version: "1.0.0".to_string(),
                description: "Find all simple paths between two symbols bounded by max hops".to_string(),
                mode: "Stream".to_string(),
                categories: vec!["pathfinding".to_string(), "graph".to_string()],
            },
            AlgorithmDescriptorSummary {
                id: "page_rank".to_string(),
                name: "PageRank".to_string(),
                version: "1.0.0".to_string(),
                description: "Compute PageRank scores for all symbols in the call graph".to_string(),
                mode: "Stats".to_string(),
                categories: vec!["centrality".to_string(), "graph".to_string()],
            },
            AlgorithmDescriptorSummary {
                id: "scc".to_string(),
                name: "Strongly Connected Components".to_string(),
                version: "1.0.0".to_string(),
                description: "Find strongly connected components in the call graph".to_string(),
                mode: "Stats".to_string(),
                categories: vec!["clustering".to_string(), "graph".to_string()],
            },
            AlgorithmDescriptorSummary {
                id: "wcc".to_string(),
                name: "Weakly Connected Components".to_string(),
                version: "1.0.0".to_string(),
                description: "Find weakly connected components in the call graph".to_string(),
                mode: "Stats".to_string(),
                categories: vec!["clustering".to_string(), "graph".to_string()],
            },
        ];

        let total = algorithms.len();
        let response = AnalyticsCatalogResponse { algorithms, total };

        ok_envelope(TOOL_ANALYTICS_CATALOG, &response)
    }
}

// ============================================================================
// AnalyticsLineageList handler
// ============================================================================

struct AnalyticsLineageListHandler;

#[async_trait]
impl ToolHandler for AnalyticsLineageListHandler {
    fn name(&self) -> &'static str {
        TOOL_ANALYTICS_LINEAGE_LIST
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of records to return (default 100)"
                }
            },
            "additionalProperties": false
        })
    }

    async fn handle(&self, _ctx: &McpContext, params: Value) -> CallToolResult {
        let args: AnalyticsLineageListArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_ANALYTICS_LINEAGE_LIST,
                    "invalid_args",
                    &format!("{TOOL_ANALYTICS_LINEAGE_LIST}: invalid args: {e}"),
                );
            }
        };

        let _limit = args.limit.unwrap_or(100).min(100);

        // Stub lineage response - in production this would query the lineage store
        let runs: Vec<LineageEntry> = vec![];

        let response = AnalyticsLineageResponse {
            runs,
            total: 0,
        };

        ok_envelope(TOOL_ANALYTICS_LINEAGE_LIST, &response)
    }
}

// ============================================================================
// AnalyticsLineageGet handler
// ============================================================================

struct AnalyticsLineageGetHandler;

#[async_trait]
impl ToolHandler for AnalyticsLineageGetHandler {
    fn name(&self) -> &'static str {
        TOOL_ANALYTICS_LINEAGE_GET
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "run_id": {
                    "type": "string",
                    "description": "The unique run identifier"
                }
            },
            "required": ["run_id"]
        })
    }

    async fn handle(&self, _ctx: &McpContext, params: Value) -> CallToolResult {
        let args: AnalyticsLineageGetArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_ANALYTICS_LINEAGE_GET,
                    "invalid_args",
                    &format!("{TOOL_ANALYTICS_LINEAGE_GET}: invalid args: {e}"),
                );
            }
        };

        let run_id = match &args.run_id {
            Some(id) if !id.is_empty() => id.clone(),
            _ => {
                return err_envelope(
                    TOOL_ANALYTICS_LINEAGE_GET,
                    "missing_required_arg",
                    "analytics_lineage_get: missing required arg `run_id`",
                );
            }
        };

        // Stub - in production this would query the lineage store
        return err_envelope(
            TOOL_ANALYTICS_LINEAGE_GET,
            "not_implemented",
            &format!("analytics_lineage_get: run `{run_id}` not found (lineage store not wired in this build)"),
        );
    }
}

// ============================================================================
// Registration
// ============================================================================

/// Register all analytics tool handlers into the provided registry.
pub fn register_analytics_handlers(registry: &mut crate::mcp::handler::ToolHandlerRegistry) {
    registry.register(AnalyticsRunHandler);
    registry.register(AnalyticsCatalogHandler);
    registry.register(AnalyticsLineageListHandler);
    registry.register(AnalyticsLineageGetHandler);
}
