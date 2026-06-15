//! Search and inspection tool handlers.
//!
//! Implements 2 MCP tools:
//! - `explorer_spotter_search` — search symbols by name with optional kind filter
//! - `explorer_inspect_object` — inspect an object by its MVP id

use std::sync::Arc;

use async_trait::async_trait;
use rmcp::model::{CallToolResult, Content};
use serde::Deserialize;
use serde_json::Value;

use crate::mcp::envelope::{err_envelope, ok_envelope};
use crate::mcp::handler::ToolHandler;
use crate::mcp::{McpContext, TOOL_INSPECT_OBJECT, TOOL_SPOTTER_SEARCH};

// ============================================================================
// Arg structs
// ============================================================================

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpotterArgs {
    query: Option<String>,
    kind: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct InspectArgs {
    object_id: Option<String>,
}

// ============================================================================
// ToolHandler implementations
// ============================================================================

// --- explorer_spotter_search ---

struct SpotterSearchHandler;

#[async_trait]
impl ToolHandler for SpotterSearchHandler {
    fn name(&self) -> &'static str {
        TOOL_SPOTTER_SEARCH
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query (required)."
                },
                "kind": {
                    "type": "string",
                    "description": "Optional kind filter (e.g. 'Function', 'Struct')."
                }
            },
            "required": ["query"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: SpotterArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return err_envelope(TOOL_SPOTTER_SEARCH, "invalid_args",
                &format!("{TOOL_SPOTTER_SEARCH}: invalid args: {e}")),
        };

        let query = match args.query {
            Some(q) if !q.is_empty() => q,
            _ => return err_envelope(TOOL_SPOTTER_SEARCH, "missing_required_arg",
                "explorer_spotter_search: missing required arg `query`"),
        };

        let result = ctx.search
            .as_ref()
            .unwrap()
            .spotter_search(&query, args.kind.as_deref())
            .await;
        match result {
            Ok(results) => ok_envelope(TOOL_SPOTTER_SEARCH, &results),
            Err(e) => err_envelope(TOOL_SPOTTER_SEARCH, "service_error", &e.to_string()),
        }
    }
}

// --- explorer_inspect_object ---

struct InspectObjectHandler;

#[async_trait]
impl ToolHandler for InspectObjectHandler {
    fn name(&self) -> &'static str {
        TOOL_INSPECT_OBJECT
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "object_id": {
                    "type": "string",
                    "description": "MVP id of the object to inspect (required)."
                }
            },
            "required": ["object_id"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: InspectArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return err_envelope(TOOL_INSPECT_OBJECT, "invalid_args",
                &format!("{TOOL_INSPECT_OBJECT}: invalid args: {e}")),
        };

        let object_id = match args.object_id {
            Some(id) if !id.is_empty() => id,
            _ => return err_envelope(TOOL_INSPECT_OBJECT, "missing_required_arg",
                "explorer_inspect_object: missing required arg `object_id`"),
        };

        let result = ctx.search
            .as_ref()
            .unwrap()
            .inspect_object(&object_id)
            .await;
        match result {
            Ok(summary) => ok_envelope(TOOL_INSPECT_OBJECT, &summary),
            Err(e) => err_envelope(TOOL_INSPECT_OBJECT, "service_error", &e.to_string()),
        }
    }
}

// ============================================================================
// Registry builder
// ============================================================================

/// Register both search-family handlers into the registry.
pub fn register_search_handlers(registry: &mut crate::mcp::handler::ToolHandlerRegistry) {
    registry.register(SpotterSearchHandler);
    registry.register(InspectObjectHandler);
}
