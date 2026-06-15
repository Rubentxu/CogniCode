//! Workspace tool handler.
//!
//! Implements 1 MCP tool:
//! - `explorer_open_workspace` — open a workspace by path or return the current one

use std::sync::Arc;

use async_trait::async_trait;
use rmcp::model::{CallToolResult, Content};
use serde::Deserialize;
use serde_json::Value;

use crate::dto::OpenWorkspaceRequest;
use crate::mcp::envelope::{err_envelope, ok_envelope};
use crate::mcp::handler::ToolHandler;
use crate::mcp::{McpContext, TOOL_OPEN_WORKSPACE};

// ============================================================================
// Arg struct
// ============================================================================

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OpenWorkspaceArgs {
    root_path: Option<String>,
}

// ============================================================================
// ToolHandler implementation
// ============================================================================

struct OpenWorkspaceHandler;

#[async_trait]
impl ToolHandler for OpenWorkspaceHandler {
    fn name(&self) -> &'static str {
        TOOL_OPEN_WORKSPACE
    }

    fn arg_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "root_path": {
                    "type": "string",
                    "description": "Filesystem path to the workspace root. Optional — when omitted, the workspace bound at handler construction is returned."
                }
            },
            "required": []
        })
    }

    async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
        let args: OpenWorkspaceArgs = match serde_json::from_value(params) {
            Ok(a) => a,
            Err(e) => return err_envelope(TOOL_OPEN_WORKSPACE, "invalid_args",
                &format!("{TOOL_OPEN_WORKSPACE}: invalid args: {e}")),
        };

        // Use the workspace facade directly (PR 1 migration — fallback removed).
        let workspace_svc = match ctx.workspace.as_ref() {
            Some(ws) => ws,
            None => {
                return err_envelope(TOOL_OPEN_WORKSPACE, "facade_unavailable",
                    "workspace service not wired");
            }
        };

        let result = match args.root_path {
            Some(root_path) => {
                workspace_svc
                    .open_workspace(OpenWorkspaceRequest { root_path })
                    .await
            }
            None => workspace_svc.current_workspace(),
        };

        match result {
            Ok(workspace) => ok_envelope(TOOL_OPEN_WORKSPACE, &workspace),
            Err(e) => err_envelope(TOOL_OPEN_WORKSPACE, "service_error", &e.to_string()),
        }
    }
}

// ============================================================================
// Registry builder
// ============================================================================

/// Register the workspace-family handler into the registry.
pub fn register_workspace_handlers(registry: &mut crate::mcp::handler::ToolHandlerRegistry) {
    registry.register(OpenWorkspaceHandler);
}
