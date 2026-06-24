//! Architecture-drift tool handler.
//!
//! Implements 1 MCP tool:
//! - `detect_architecture_drift` — compare inferred C4 architecture against expected

use async_trait::async_trait;
use rmcp::model::{CallToolResult, Content};
use serde_json::Value;

use crate::dto::DriftReport;
use crate::mcp::envelope::{err_envelope, ok_envelope};
use crate::mcp::handler::ToolHandler;
use crate::mcp::{McpContext, TOOL_DETECT_ARCHITECTURE_DRIFT};

// ============================================================================
// ToolHandler implementation
// ============================================================================

struct DetectArchitectureDriftHandler;

#[async_trait]
impl ToolHandler for DetectArchitectureDriftHandler {
    fn name(&self) -> &'static str {
        TOOL_DETECT_ARCHITECTURE_DRIFT
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let _args: () = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => {
                return err_envelope(
                    TOOL_DETECT_ARCHITECTURE_DRIFT,
                    "invalid_args",
                    &format!("{TOOL_DETECT_ARCHITECTURE_DRIFT}: invalid args: {e}"),
                );
            }
        };

        let graph_svc = match ctx.graph_service.as_ref() {
            Some(gs) => gs,
            None => {
                return err_envelope(
                    TOOL_DETECT_ARCHITECTURE_DRIFT,
                    "facade_unavailable",
                    "graph service not wired",
                );
            }
        };

        let workspace_svc = match ctx.workspace.as_ref() {
            Some(ws) => ws,
            None => {
                return err_envelope(
                    TOOL_DETECT_ARCHITECTURE_DRIFT,
                    "facade_unavailable",
                    "workspace service not wired",
                );
            }
        };

        let workspace = match workspace_svc.current_workspace() {
            Ok(ws) => ws,
            Err(e) => {
                return err_envelope(
                    TOOL_DETECT_ARCHITECTURE_DRIFT,
                    "workspace_error",
                    &e.to_string(),
                );
            }
        };

        let root_path = std::path::PathBuf::from(&workspace.root_path);
        match graph_svc
            .compare_architecture(root_path.to_string_lossy().as_ref())
            .await
        {
            Ok(report) => {
                let report: DriftReport = report;
                ok_envelope(TOOL_DETECT_ARCHITECTURE_DRIFT, &report)
            }
            Err(e) => err_envelope(
                TOOL_DETECT_ARCHITECTURE_DRIFT,
                "service_error",
                &e.to_string(),
            ),
        }
    }
}

// ============================================================================
// Registry builder
// ============================================================================

/// Register the drift-family handler into the registry.
pub fn register_drift_handlers(registry: &mut crate::mcp::handler::ToolHandlerRegistry) {
    registry.register(DetectArchitectureDriftHandler);
}
