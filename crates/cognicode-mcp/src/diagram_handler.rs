//! Diagram-aware MCP handler wrapper
//!
//! Wraps `CogniCodeHandler` from `cognicode-core` and adds the `generate_c4_code` tool
//! from `cognicode-diagram`. This avoids circular dependencies by keeping diagram
//! integration in the binary crate.

use cognicode_core::interface::mcp::CogniCodeHandler;
use cognicode_diagram::mcp::tools::{GenerateC4CodeInput, handle_generate_c4_code};
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content,
    ListToolsResult, ServerInfo, Tool,
};
use rmcp::service::RoleServer;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Wrapper around `CogniCodeHandler` that adds diagram tools
#[derive(Debug)]
pub struct DiagramAwareHandler {
    inner: CogniCodeHandler,
    cancellation_token: Arc<AtomicBool>,
}

impl DiagramAwareHandler {
    pub fn new(project_root: PathBuf) -> Self {
        let cancellation_token = Arc::new(AtomicBool::new(false));
        let inner = CogniCodeHandler::new(project_root);
        Self { inner, cancellation_token }
    }

    pub fn with_graph_store(
        project_root: PathBuf,
        store: Box<dyn cognicode_core::domain::traits::GraphStore>,
    ) -> Self {
        let inner = CogniCodeHandler::with_graph_store(project_root, store);
        let cancellation_token = Arc::new(AtomicBool::new(false));
        Self { inner, cancellation_token }
    }
}

impl ServerHandler for DiagramAwareHandler {
    fn get_info(&self) -> ServerInfo {
        self.inner.get_info()
    }

    fn list_tools(
        &self,
        request: Option<rmcp::model::PaginatedRequestParams>,
        context: rmcp::service::RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, rmcp::ErrorData>> + Send + '_ {
        async move {
            // Get the base tools from CogniCodeHandler
            let mut result = self.inner.list_tools(request, context).await?;

            // Add diagram tool to the first page if there's room
            // or it will appear when the client paginates
            let diagram_tool = Tool::new(
                "generate_c4_code",
                "Generate C4 model code-level (L4) diagrams from code analysis. Infers classes, structs, enums, traits and their relationships from the call graph, then renders as a Mermaid class diagram.",
                Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "scope": {
                            "type": "string",
                            "description": "Module or path scope to analyze (e.g. 'src/domain', 'crates/my-crate')"
                        },
                        "max_depth": {
                            "type": "integer",
                            "description": "Maximum dependency traversal depth (default: 3)"
                        },
                        "format": {
                            "type": "string",
                            "description": "Output format: 'mermaid' (default). Future: 'plantuml', 'd2', 'svg'"
                        },
                        "show_methods": {
                            "type": "boolean",
                            "description": "Include methods in class diagrams (default: true)"
                        },
                        "show_attributes": {
                            "type": "boolean",
                            "description": "Include attributes in class diagrams (default: true)"
                        }
                    },
                    "required": ["scope"]
                }).as_object().cloned().unwrap()),
            );

            result.tools.push(diagram_tool);

            Ok(result)
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: rmcp::service::RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, rmcp::ErrorData>> + Send + '_ {
        async move {
            let tool_name = request.name.as_ref();

            if tool_name == "generate_c4_code" {
                // Handle diagram tool locally
                let arguments = request.arguments.unwrap_or_default();
                let result = self.handle_diagram_tool(serde_json::Value::Object(arguments)).await;
                match result {
                    Ok(output) => Ok(CallToolResult::success(vec![Content::text(output)])),
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
                }
            } else {
                // Delegate all other tools to the inner handler
                self.inner.call_tool(request, context).await
            }
        }
    }

    fn on_cancelled(
        &self,
        notification: rmcp::model::CancelledNotificationParam,
        context: rmcp::service::NotificationContext<RoleServer>,
    ) -> impl std::future::Future<Output = ()> + Send + '_ {
        self.cancellation_token.store(true, Ordering::SeqCst);
        self.inner.on_cancelled(notification, context)
    }
}

impl DiagramAwareHandler {
    async fn handle_diagram_tool(
        &self,
        arguments: serde_json::Value,
    ) -> anyhow::Result<String> {
        let input: GenerateC4CodeInput =
            serde_json::from_value(arguments)?;

        let call_graph = self.inner.get_call_graph()?;

        let output = handle_generate_c4_code(input, &call_graph)?;

        Ok(serde_json::to_string(&output)?)
    }
}
