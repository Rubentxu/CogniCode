//! C4 Mermaid export tool handler.
//!
//! Implements 1 MCP tool:
//! - `export_c4_mermaid` — render a C4-level architecture as a Mermaid C4 diagram

use async_trait::async_trait;
use rmcp::model::{CallToolResult, Content};
use serde_json::Value;

use crate::dto::SubgraphResponse;
use crate::domain::c4_mermaid::{c4_to_mermaid, C4Level};
use crate::mcp::envelope::{err_envelope, ok_envelope};
use crate::mcp::handler::ToolHandler;
use crate::mcp::{McpContext, TOOL_EXPORT_C4_MERMAID};

// ============================================================================
// ToolHandler implementation
// ============================================================================

struct ExportC4MermaidHandler;

#[async_trait]
impl ToolHandler for ExportC4MermaidHandler {
    fn name(&self) -> &'static str {
        TOOL_EXPORT_C4_MERMAID
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "level": {
                    "type": "string",
                    "enum": ["context", "container", "component"],
                    "description": "C4 diagram level (context | container | component)"
                }
            },
            "required": ["level"]
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "lowercase")]
        struct Args {
            level: String,
        }

        let args: Args = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_EXPORT_C4_MERMAID,
                    "invalid_args",
                    &format!("{TOOL_EXPORT_C4_MERMAID}: invalid args: {e}"),
                );
            }
        };

        let c4_level = match C4Level::parse(&args.level) {
            Ok(l) => l,
            Err(e) => {
                return err_envelope(
                    TOOL_EXPORT_C4_MERMAID,
                    "invalid_level",
                    &e.to_string(),
                );
            }
        };

        let graph_svc = match ctx.graph_service.as_ref() {
            Some(gs) => gs,
            None => {
                return err_envelope(
                    TOOL_EXPORT_C4_MERMAID,
                    "facade_unavailable",
                    "graph service not wired",
                );
            }
        };

        let workspace_svc = match ctx.workspace.as_ref() {
            Some(ws) => ws,
            None => {
                return err_envelope(
                    TOOL_EXPORT_C4_MERMAID,
                    "facade_unavailable",
                    "workspace service not wired",
                );
            }
        };

        let workspace = match workspace_svc.current_workspace() {
            Ok(ws) => ws,
            Err(e) => {
                return err_envelope(
                    TOOL_EXPORT_C4_MERMAID,
                    "workspace_error",
                    &e.to_string(),
                );
            }
        };

        let architecture: SubgraphResponse = match graph_svc
            .build_architecture(&workspace.root_path)
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                return err_envelope(
                    TOOL_EXPORT_C4_MERMAID,
                    "service_error",
                    &e.to_string(),
                );
            }
        };

        let mermaid = c4_to_mermaid(&architecture.nodes, &architecture.edges, c4_level);
        // Return the raw mermaid string as a JSON payload
        ok_envelope(TOOL_EXPORT_C4_MERMAID, &mermaid)
    }
}

// ============================================================================
// Registry builder
// ============================================================================

/// Register the export-family handler into the registry.
pub fn register_export_handlers(registry: &mut crate::mcp::handler::ToolHandlerRegistry) {
    registry.register(ExportC4MermaidHandler);
}
