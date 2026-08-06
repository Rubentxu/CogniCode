//! View-family tool handlers.
//!
//! Implements 7 MCP tools for view and lens operations:
//! - `explorer_get_views`  — list available views for an object
//! - `explorer_get_view`   — build a specific contextual view
//! - `explorer_get_lenses` — list available lenses for an object
//! - `explorer_apply_lens` — apply a lens to an object
//! - `explorer_query_moldql` — execute a MoldQL query
//! - `moldql_pattern_query` — execute a Pattern Profile query (T7)
//! - `moldql_pattern_capabilities` — return v1 supported-feature matrix (T7)
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(deprecated, unused_imports)]

use std::sync::Arc;
use std::sync::atomic::Ordering;

use async_trait::async_trait;
use rmcp::model::{CallToolResult, Content};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mcp::envelope::{err_envelope, ok_envelope};
use crate::mcp::handler::ToolHandler;
use crate::mcp::{
    McpContext, ProvenanceMetadata, TOOL_APPLY_LENS, TOOL_GET_LENSES, TOOL_GET_VIEW,
    TOOL_GET_VIEWS, TOOL_PATTERN_CAPABILITIES, TOOL_PATTERN_QUERY, TOOL_QUERY_MOLDQL,
};

// ============================================================================
// Arg structs
// ============================================================================

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct InspectArgs {
    object_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GetViewArgs {
    object_id: Option<String>,
    view_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ApplyLensArgs {
    object_id: Option<String>,
    lens_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct QueryMoldQLArgs {
    query: Option<String>,
    target: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct PatternQueryArgs {
    query: Option<String>,
    workspace_id: Option<String>,
    revision_id: Option<u64>,
}

// ============================================================================
// ToolHandler implementations — one struct per tool
// ============================================================================

// --- explorer_get_views ---

struct GetViewsHandler;

#[async_trait]
impl ToolHandler for GetViewsHandler {
    fn name(&self) -> &'static str {
        TOOL_GET_VIEWS
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "object_id": {
                    "type": "string",
                    "description": "The object id to query views for (required)."
                }
            },
            "required": ["object_id"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: InspectArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_GET_VIEWS,
                    "invalid_args",
                    &format!("{TOOL_GET_VIEWS}: invalid args: {e}"),
                );
            }
        };

        let object_id = match args.object_id {
            Some(id) if !id.is_empty() => id,
            _ => {
                return err_envelope(
                    TOOL_GET_VIEWS,
                    "missing_required_arg",
                    "explorer_get_views: missing required arg `object_id`",
                );
            }
        };

        let view_service = match ctx.view.as_ref() {
            Some(v) => v,
            None => {
                return err_envelope(
                    TOOL_GET_VIEWS,
                    "facade_unavailable",
                    "view service not wired",
                );
            }
        };

        let result = view_service.available_views(&object_id).await;
        match result {
            Ok(views) => {
                let payload = serde_json::to_value(views).unwrap_or(Value::Null);
                ok_envelope(TOOL_GET_VIEWS, &payload)
            }
            Err(e) => err_envelope(TOOL_GET_VIEWS, "service_error", &e.to_string()),
        }
    }
}

// --- explorer_get_view ---

struct GetViewHandler;

#[async_trait]
impl ToolHandler for GetViewHandler {
    fn name(&self) -> &'static str {
        TOOL_GET_VIEW
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "object_id": {
                    "type": "string",
                    "description": "The object id to get the view for (required)."
                },
                "view_id": {
                    "type": "string",
                    "description": "The view id to build (required)."
                }
            },
            "required": ["object_id", "view_id"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: GetViewArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_GET_VIEW,
                    "invalid_args",
                    &format!("{TOOL_GET_VIEW}: invalid args: {e}"),
                );
            }
        };

        let object_id = match args.object_id {
            Some(id) if !id.is_empty() => id,
            _ => {
                return err_envelope(
                    TOOL_GET_VIEW,
                    "missing_required_arg",
                    "explorer_get_view: missing required arg `object_id`",
                );
            }
        };

        let view_id = match args.view_id {
            Some(v) if !v.is_empty() => v,
            _ => {
                return err_envelope(
                    TOOL_GET_VIEW,
                    "missing_required_arg",
                    "explorer_get_view: missing required arg `view_id`",
                );
            }
        };

        let view_service = match ctx.view.as_ref() {
            Some(v) => v,
            None => {
                return err_envelope(
                    TOOL_GET_VIEW,
                    "facade_unavailable",
                    "view service not wired",
                );
            }
        };

        let result = view_service.contextual_view(&object_id, &view_id).await;
        match result {
            Ok(view) => {
                let payload = serde_json::to_value(view).unwrap_or(Value::Null);
                ok_envelope(TOOL_GET_VIEW, &payload)
            }
            Err(e) => err_envelope(TOOL_GET_VIEW, "service_error", &e.to_string()),
        }
    }
}

// --- explorer_get_lenses ---

struct GetLensesHandler;

#[async_trait]
impl ToolHandler for GetLensesHandler {
    fn name(&self) -> &'static str {
        TOOL_GET_LENSES
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "object_id": {
                    "type": "string",
                    "description": "The object id to query lenses for (required)."
                }
            },
            "required": ["object_id"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: InspectArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_GET_LENSES,
                    "invalid_args",
                    &format!("{TOOL_GET_LENSES}: invalid args: {e}"),
                );
            }
        };

        let object_id = match args.object_id {
            Some(id) if !id.is_empty() => id,
            _ => {
                return err_envelope(
                    TOOL_GET_LENSES,
                    "missing_required_arg",
                    "explorer_get_lenses: missing required arg `object_id`",
                );
            }
        };

        let view_service = match ctx.view.as_ref() {
            Some(v) => v,
            None => {
                return err_envelope(
                    TOOL_GET_LENSES,
                    "facade_unavailable",
                    "view service not wired",
                );
            }
        };

        let result = view_service.available_lenses(&object_id).await;
        match result {
            Ok(lenses) => {
                let payload = serde_json::to_value(lenses).unwrap_or(Value::Null);
                ok_envelope(TOOL_GET_LENSES, &payload)
            }
            Err(e) => err_envelope(TOOL_GET_LENSES, "service_error", &e.to_string()),
        }
    }
}

// --- explorer_apply_lens ---

struct ApplyLensHandler;

#[async_trait]
impl ToolHandler for ApplyLensHandler {
    fn name(&self) -> &'static str {
        TOOL_APPLY_LENS
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "object_id": {
                    "type": "string",
                    "description": "The object id to apply the lens to (required)."
                },
                "lens_id": {
                    "type": "string",
                    "description": "The lens id to apply (required)."
                }
            },
            "required": ["object_id", "lens_id"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: ApplyLensArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_APPLY_LENS,
                    "invalid_args",
                    &format!("{TOOL_APPLY_LENS}: invalid args: {e}"),
                );
            }
        };

        let object_id = match args.object_id {
            Some(id) if !id.is_empty() => id,
            _ => {
                return err_envelope(
                    TOOL_APPLY_LENS,
                    "missing_required_arg",
                    "explorer_apply_lens: missing required arg `object_id`",
                );
            }
        };

        let lens_id = match args.lens_id {
            Some(l) if !l.is_empty() => l,
            _ => {
                return err_envelope(
                    TOOL_APPLY_LENS,
                    "missing_required_arg",
                    "explorer_apply_lens: missing required arg `lens_id`",
                );
            }
        };

        let view_service = match ctx.view.as_ref() {
            Some(v) => v,
            None => {
                return err_envelope(
                    TOOL_APPLY_LENS,
                    "facade_unavailable",
                    "view service not wired",
                );
            }
        };

        let result = view_service.apply_lens(&object_id, &lens_id).await;
        match result {
            Ok(lens_result) => {
                let payload = serde_json::to_value(lens_result).unwrap_or(Value::Null);
                ok_envelope(TOOL_APPLY_LENS, &payload)
            }
            Err(e) => err_envelope(TOOL_APPLY_LENS, "service_error", &e.to_string()),
        }
    }
}

// --- explorer_query_moldql ---

struct QueryMoldQLHandler;

#[async_trait]
impl ToolHandler for QueryMoldQLHandler {
    fn name(&self) -> &'static str {
        TOOL_QUERY_MOLDQL
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The MoldQL query to execute (required)."
                },
                "target": {
                    "type": "string",
                    "description": "Compile target: 'pg' | 'petgraph' | 'auto' (default: 'auto')."
                }
            },
            "required": ["query"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: QueryMoldQLArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_QUERY_MOLDQL,
                    "invalid_args",
                    &format!("{TOOL_QUERY_MOLDQL}: invalid args: {e}"),
                );
            }
        };

        let query = match args.query {
            Some(q) if !q.is_empty() => q,
            _ => {
                return err_envelope(
                    TOOL_QUERY_MOLDQL,
                    "missing_required_arg",
                    "explorer_query_moldql: missing required arg `query`",
                );
            }
        };

        // Optional `target` argument: "pg" | "petgraph" | "auto".
        let target = match args.target.as_deref() {
            None | Some("auto") => None,
            Some("pg") => Some(crate::moldql::compile::CompileTarget::Postgres),
            Some("petgraph") => Some(crate::moldql::compile::CompileTarget::Petgraph),
            Some(other) => {
                return err_envelope(
                    TOOL_QUERY_MOLDQL,
                    "invalid_target",
                    &format!(
                        "explorer_query_moldql: invalid `target` `{other}` \
                         (expected one of: pg, petgraph, auto)"
                    ),
                );
            }
        };

        // Use the MoldQL facade directly (PR 1 migration — fallback removed).
        let moldql_service = match ctx.moldql.as_ref() {
            Some(ms) => ms,
            None => {
                return err_envelope(
                    TOOL_QUERY_MOLDQL,
                    "facade_unavailable",
                    "moldql service not wired",
                );
            }
        };

        let result: Result<crate::dto::MoldQLResultDto, _> = match target {
            None => moldql_service
                .execute_query(&query)
                .await
                .map(crate::dto::MoldQLResultDto::from),
            Some(tgt) => moldql_service
                .execute_query_with_target(&query, tgt)
                .await
                .map(crate::dto::MoldQLResultDto::from),
        };

        match result {
            Ok(dto) => {
                let payload = serde_json::to_value(dto).unwrap_or(Value::Null);
                ok_envelope(TOOL_QUERY_MOLDQL, &payload)
            }
            Err(e) => err_envelope(TOOL_QUERY_MOLDQL, "service_error", &e.to_string()),
        }
    }
}

// --- moldql_pattern_query ---

struct PatternQueryHandler;

#[async_trait]
impl ToolHandler for PatternQueryHandler {
    fn name(&self) -> &'static str {
        TOOL_PATTERN_QUERY
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The Pattern Profile query to execute (required). Example: MATCH (f:Function) RETURN PATH(f)"
                },
                "workspace_id": {
                    "type": "string",
                    "description": "Optional workspace scope (defaults to current workspace)."
                },
                "revision_id": {
                    "type": "integer",
                    "description": "Optional revision pin (defaults to workspace HEAD)."
                }
            },
            "required": ["query"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: PatternQueryArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_PATTERN_QUERY,
                    "invalid_args",
                    &format!("{TOOL_PATTERN_QUERY}: invalid args: {e}"),
                );
            }
        };

        let query = match args.query {
            Some(q) if !q.is_empty() => q,
            _ => {
                return err_envelope(
                    TOOL_PATTERN_QUERY,
                    "missing_required_arg",
                    "moldql_pattern_query: missing required arg `query`",
                );
            }
        };

        // Use the MoldQL facade (same implementation as explorer_query_moldql).
        let moldql_service = match ctx.moldql.as_ref() {
            Some(ms) => ms,
            None => {
                return err_envelope(
                    TOOL_PATTERN_QUERY,
                    "facade_unavailable",
                    "moldql service not wired",
                );
            }
        };

        // Resolve pin: explicit args > ctx.current_pin() > default fallback.
        let (ws_id, rev_id) = match (&args.workspace_id, args.revision_id) {
            (Some(ws), Some(rev)) => (ws.clone(), rev),
            (Some(ws), None) => (
                ws.clone(),
                ctx.revision_tracker
                    .as_ref()
                    .map(|t| t.load(Ordering::SeqCst))
                    .unwrap_or(1),
            ),
            (None, Some(rev)) => {
                let (ws, _) = ctx.current_pin();
                (ws, rev)
            }
            (None, None) => ctx.current_pin(),
        };
        let result: Result<crate::dto::MoldQLResultDto, _> = moldql_service
            .execute_query_pinned(&query, ws_id, rev_id)
            .await
            .map(crate::dto::MoldQLResultDto::from);

        match result {
            Ok(dto) => {
                let payload = serde_json::to_value(dto).unwrap_or(Value::Null);
                ok_envelope(TOOL_PATTERN_QUERY, &payload)
            }
            Err(e) => err_envelope(TOOL_PATTERN_QUERY, "service_error", &e.to_string()),
        }
    }
}

// --- moldql_pattern_capabilities ---

struct PatternCapabilitiesHandler;

#[async_trait]
impl ToolHandler for PatternCapabilitiesHandler {
    fn name(&self) -> &'static str {
        TOOL_PATTERN_CAPABILITIES
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn handle(&self, _ctx: &McpContext, _params: Value) -> CallToolResult {
        // Hard-coded v1 supported-feature matrix.
        // PR4 will refine the matrix with actual runtime detection.
        let matrix = serde_json::json!({
            "version": "1.0",
            "profile": "Pattern Profile",
            "features": [
                {"construct": "MATCH (node:Label)", "status": "supported", "notes": "Typed node patterns with label"},
                {"construct": "MATCH (a)-[e:EdgeType]->(b)", "status": "supported", "notes": "Directed edge patterns"},
                {"construct": "MATCH (a)-[e:EdgeType*1..N]->(b)", "status": "supported", "notes": "Bounded path quantifier; N must be finite"},
                {"construct": "MATCH (a)-[e?]->(b)", "status": "supported", "notes": "Zero-or-one quantifier maps to 0..1"},
                {"construct": "MATCH (a)-[e+]->(b)", "status": "supported", "notes": "One-or-more quantifier maps to 1..profile_max_hops"},
                {"construct": "RETURN PATH(a,b)", "status": "supported", "notes": "Path result shape with bindings"},
                {"construct": "RETURN COUNT(e)", "status": "supported", "notes": "Aggregation with ordering and limit"},
                {"construct": "SHORTEST path", "status": "supported", "notes": "Bounded shortest path selection"},
                {"construct": "CREATE/DELETE/SET/MERGE", "status": "unsupported", "notes": "Pattern Profile is read-only; mutations rejected as UnsupportedConstruct"}
            ],
            "compatibility_claims": {
                "cypher": "not_claimed",
                "opencypher": "not_claimed",
                "iso_gql": "not_claimed"
            }
        });

        ok_envelope(TOOL_PATTERN_CAPABILITIES, &matrix)
    }
}

// ============================================================================
// Registry builder
// ============================================================================

/// Register all 7 view-family handlers into the registry.
pub fn register_view_handlers(registry: &mut crate::mcp::handler::ToolHandlerRegistry) {
    registry.register(GetViewsHandler);
    registry.register(GetViewHandler);
    registry.register(GetLensesHandler);
    registry.register(ApplyLensHandler);
    registry.register(QueryMoldQLHandler);
    registry.register(PatternQueryHandler);
    registry.register(PatternCapabilitiesHandler);
}
