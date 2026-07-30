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
use std::sync::Arc;

use cognicode_core::application::services::graph_analytics::{
    AlgorithmRegistry, CallerCapabilities, DefaultAnalyticsBoundaryGuard, RunRequest,
};
use cognicode_core::domain::analytics::lineage::{RunLineageFilter, RunLineageStore, Uuid};
use cognicode_core::domain::analytics::{AnalyticsMode, RunOutput};
use cognicode_core::domain::plan::limits::PlanLimits;

use crate::dto::{
    AlgorithmDescriptorSummary, AnalyticsCatalogResponse, AnalyticsLineageDetailResponse,
    AnalyticsLineageResponse, LineageEntry, RunAnalyticsResponse,
};
use crate::mcp::McpContext;
use crate::mcp::envelope::{err_envelope, ok_envelope};
use crate::mcp::handler::ToolHandler;

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

        // Require analytics_registry
        let registry = match ctx.analytics_registry.as_ref() {
            Some(r) => r,
            None => {
                return err_envelope(
                    TOOL_ANALYTICS_RUN,
                    "analytics_not_available",
                    "analytics_run: AlgorithmRegistry not wired in this context",
                );
            }
        };

        // Require lineage store
        let lineage = match ctx.analytics_lineage_store.as_ref() {
            Some(l) => l,
            None => {
                return err_envelope(
                    TOOL_ANALYTICS_RUN,
                    "analytics_not_available",
                    "analytics_run: AnalyticsLineageStore not wired in this context",
                );
            }
        };

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

        // Parse algorithm_id
        let algorithm_id_parsed =
            cognicode_core::domain::analytics::descriptor::AlgorithmId::from_string(&algorithm_id);

        // Build RunRequest
        let (workspace_id, revision_id) = ctx.current_pin();
        let workspace_id =
            cognicode_core::domain::value_objects::WorkspaceId::try_new(workspace_id)
                .unwrap_or_else(|_| {
                    cognicode_core::domain::value_objects::WorkspaceId::try_new("default").unwrap()
                });
        let revision_id = cognicode_core::domain::value_objects::RevisionId::new(revision_id);

        let max_hops = args.max_hops.unwrap_or(5);
        let params = serde_json::json!({
            "from_symbol": from_symbol,
            "to_symbol": to_symbol,
            "max_hops": max_hops,
        });

        let request = RunRequest {
            algorithm_id: algorithm_id_parsed,
            params,
            pin: (workspace_id, revision_id),
            mode: AnalyticsMode::Stats,
            caller_limits: PlanLimits::default(),
            seed: None,
            idempotency_key: None,
            caller: CallerCapabilities::ExternalMCP,
            graph: (*graph).as_ref().clone(),
        };

        // Execute via registry with ExternalMCP capability (Persist denied by default)
        let result = match registry.run(request).await {
            Ok(run_result) => {
                // Map RunResult to RunAnalyticsResponse
                let run_id = run_result.run_id.to_string();
                let executed_at = chrono::Utc::now().to_rfc3339();
                let output_json = match run_result.output {
                    RunOutput::PageRank(v) => v,
                    RunOutput::Scc(v) => v,
                    RunOutput::Wcc(v) => v,
                    RunOutput::BoundedShortestPaths(v) => v,
                    RunOutput::Dominators {
                        nodes,
                        immediate_dominators,
                        depths,
                    } => {
                        serde_json::json!({
                            "nodes": nodes,
                            "immediate_dominators": immediate_dominators,
                            "depths": depths,
                        })
                    }
                    RunOutput::ArticulationPoints {
                        nodes,
                        cut_vertices_counts,
                    } => {
                        serde_json::json!({
                            "nodes": nodes,
                            "cut_vertices_counts": cut_vertices_counts,
                        })
                    }
                    RunOutput::Bridges { edges } => {
                        serde_json::json!({ "edges": edges })
                    }
                    RunOutput::KCore {
                        nodes,
                        core_numbers,
                    } => {
                        serde_json::json!({
                            "nodes": nodes,
                            "core_numbers": core_numbers,
                        })
                    }
                    RunOutput::Conductance {
                        community_ids,
                        scores,
                    } => {
                        serde_json::json!({
                            "community_ids": community_ids,
                            "scores": scores,
                        })
                    }
                    RunOutput::Modularity { score, community_count } => {
                        serde_json::json!({
                            "score": score,
                            "community_count": community_count,
                        })
                    }
                };
                let lineage_persisted = matches!(
                    run_result.status,
                    cognicode_core::domain::analytics::lineage::RunStatus::Succeeded
                );

                RunAnalyticsResponse {
                    algorithm_id: algorithm_id.clone(),
                    run_id,
                    executed_at,
                    lineage_persisted,
                    result: output_json,
                }
            }
            Err(e) => {
                return err_envelope(
                    TOOL_ANALYTICS_RUN,
                    "analytics_error",
                    &format!("analytics_run: {e}"),
                );
            }
        };

        ok_envelope(TOOL_ANALYTICS_RUN, &result)
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

    async fn handle(&self, ctx: &McpContext, _params: Value) -> CallToolResult {
        // Get the registry from context
        let registry = match ctx.analytics_registry.as_ref() {
            Some(r) => r,
            None => {
                return err_envelope(
                    TOOL_ANALYTICS_CATALOG,
                    "analytics_not_available",
                    "analytics_catalog: AlgorithmRegistry not wired in this context",
                );
            }
        };

        // Query admitted algorithms from the registry
        let algorithms: Vec<AlgorithmDescriptorSummary> = registry
            .admitted()
            .map(|d| {
                let identity = d.identity();
                // Build a readable name from the algorithm id (e.g., "bounded_shortest_paths" -> "Bounded Shortest Paths")
                let name = identity
                    .id
                    .as_str()
                    .split('_')
                    .map(|word| {
                        let mut chars = word.chars();
                        match chars.next() {
                            None => String::new(),
                            Some(c) => c.to_uppercase().chain(chars).collect(),
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");

                AlgorithmDescriptorSummary {
                    id: identity.id.as_str().to_string(),
                    name,
                    version: identity.version.as_str().to_string(),
                    description: "No description available".to_string(),
                    mode: format!(
                        "{:?}",
                        d.supported_modes().first().unwrap_or(&AnalyticsMode::Stats)
                    ),
                    categories: vec!["analytics".to_string()],
                }
            })
            .collect();

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

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
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

        let limit = args.limit.unwrap_or(100).min(100);

        // Require lineage store
        let lineage = match ctx.analytics_lineage_store.as_ref() {
            Some(l) => l,
            None => {
                return err_envelope(
                    TOOL_ANALYTICS_LINEAGE_LIST,
                    "analytics_not_available",
                    "analytics_lineage_list: AnalyticsLineageStore not wired in this context",
                );
            }
        };

        // Build filter from current context
        let (workspace_id_str, revision_id) = ctx.current_pin();
        let workspace_id =
            cognicode_core::domain::value_objects::WorkspaceId::try_new(workspace_id_str.clone())
                .unwrap_or_else(|_| {
                    cognicode_core::domain::value_objects::WorkspaceId::try_new("default").unwrap()
                });
        let revision_id = cognicode_core::domain::value_objects::RevisionId::new(revision_id);

        let filter = RunLineageFilter {
            workspace_id: Some(workspace_id),
            revision_id: Some(revision_id),
            algorithm_id: None,
            status: None,
        };

        // Query lineage store
        let lineage_records = match lineage.query(filter, Some(limit as u64)).await {
            Ok(records) => records,
            Err(e) => {
                return err_envelope(
                    TOOL_ANALYTICS_LINEAGE_LIST,
                    "lineage_error",
                    &format!("analytics_lineage_list: {e}"),
                );
            }
        };

        let runs: Vec<LineageEntry> = lineage_records
            .iter()
            .map(|r| LineageEntry {
                run_id: r.run_id.to_string(),
                algorithm_id: r.algorithm_id.as_str().to_string(),
                executed_at: r.started_at.to_rfc3339(),
                parameters: r.params.clone(),
                result_summary: serde_json::json!({
                    "status": format!("{}", r.status),
                    "row_count": r.row_count,
                }),
                mode: format!("{:?}", r.mode),
            })
            .collect();

        let total = runs.len();
        let response = AnalyticsLineageResponse { runs, total };

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

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
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

        // Require lineage store
        let lineage = match ctx.analytics_lineage_store.as_ref() {
            Some(l) => l,
            None => {
                return err_envelope(
                    TOOL_ANALYTICS_LINEAGE_GET,
                    "analytics_not_available",
                    "analytics_lineage_get: AnalyticsLineageStore not wired in this context",
                );
            }
        };

        // Parse run_id and query the store
        let run_uuid = Uuid::from_string(run_id.clone());
        let record = match lineage.get(run_uuid).await {
            Ok(r) => r,
            Err(e) => {
                return err_envelope(
                    TOOL_ANALYTICS_LINEAGE_GET,
                    "lineage_not_found",
                    &format!("analytics_lineage_get: run `{run_id}` not found: {e}"),
                );
            }
        };

        let response = AnalyticsLineageDetailResponse {
            run_id: record.run_id.to_string(),
            algorithm_id: record.algorithm_id.as_str().to_string(),
            executed_at: record.started_at.to_rfc3339(),
            parameters: record.params,
            result_summary: serde_json::json!({
                "status": format!("{}", record.status),
                "row_count": record.row_count,
                "finished_at": record.finished_at.map(|t| t.to_rfc3339()),
            }),
            mode: format!("{:?}", record.mode),
        };

        ok_envelope(TOOL_ANALYTICS_LINEAGE_GET, &response)
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
