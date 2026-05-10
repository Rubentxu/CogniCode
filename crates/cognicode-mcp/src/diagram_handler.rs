//! Diagram-aware MCP handler wrapper
//!
//! Wraps `CogniCodeHandler` from `cognicode-core` and adds the `generate_c4_code` tool
//! from `cognicode-diagram`. This avoids circular dependencies by keeping diagram
//! integration in the binary crate.

use cognicode_core::interface::mcp::CogniCodeHandler;
use cognicode_diagram::mcp::tools::{
    GenerateC4CodeInput, GenerateC4ContainersInput, GenerateC4ComponentsInput,
    handle_generate_c4_code, handle_generate_c4_containers, handle_generate_c4_components,
};
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content,
    ListToolsResult, ServerInfo, Tool,
};
use rmcp::service::RoleServer;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Wrapper around `CogniCodeHandler` that adds diagram tools
#[derive(Debug)]
pub struct DiagramAwareHandler {
    inner: CogniCodeHandler,
    cancellation_token: Arc<AtomicBool>,
    project_root: PathBuf,
}

impl DiagramAwareHandler {
    pub fn new(project_root: PathBuf) -> Self {
        let cancellation_token = Arc::new(AtomicBool::new(false));
        let inner = CogniCodeHandler::new(project_root.clone());
        Self { inner, cancellation_token, project_root }
    }

    pub fn with_graph_store(
        project_root: PathBuf,
        store: Box<dyn cognicode_core::domain::traits::GraphStore>,
    ) -> Self {
        let inner = CogniCodeHandler::with_graph_store(project_root.clone(), store);
        let cancellation_token = Arc::new(AtomicBool::new(false));
        Self { inner, cancellation_token, project_root }
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

            // Add diagram tools
            result.tools.push(Tool::new(
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
                            "description": "Output format: 'mermaid' (default)"
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
            ));

            result.tools.push(Tool::new(
                "generate_c4_containers",
                "Generate C4 Container (L2) diagrams. Parses Cargo.toml, package.json, or pyproject.toml to infer containers (bins, libs, services) and their dependencies.",
                Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "directory": {
                            "type": "string",
                            "description": "Project directory to analyze (default: '.')"
                        },
                        "format": {
                            "type": "string",
                            "description": "Output format: 'mermaid' (default)"
                        },
                        "show_coupling": {
                            "type": "boolean",
                            "description": "Show coupling scores between containers"
                        },
                        "show_technology": {
                            "type": "boolean",
                            "description": "Show technology stack labels (default: true)"
                        }
                    },
                    "required": []
                }).as_object().cloned().unwrap()),
            ));

            result.tools.push(Tool::new(
                "generate_c4_components",
                "Generate C4 Component (L3) diagrams. Groups symbols by module/directory to infer components within a container.",
                Arc::new(serde_json::json!({
                    "type": "object",
                    "properties": {
                        "scope": {
                            "type": "string",
                            "description": "Module or path scope to analyze (e.g. 'src/domain')"
                        },
                        "container_name": {
                            "type": "string",
                            "description": "Name for the container grouping components"
                        },
                        "format": {
                            "type": "string",
                            "description": "Output format: 'mermaid' (default)"
                        },
                        "detail_level": {
                            "type": "string",
                            "description": "Detail level: 'high' (default) shows methods/fields"
                        }
                    },
                    "required": ["scope"]
                }).as_object().cloned().unwrap()),
            ));

            Ok(result)
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: rmcp::service::RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, rmcp::ErrorData>> + Send + '_ {
        let project_root = self.project_root.clone();
        async move {
            let tool_name = request.name.as_ref();

            match tool_name {
                "generate_c4_code" => {
                    let arguments = request.arguments.unwrap_or_default();
                    let result = self.handle_generate_c4_code(serde_json::Value::Object(arguments));
                    match result {
                        Ok(output) => Ok(CallToolResult::success(vec![Content::text(output)])),
                        Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
                    }
                }
                "generate_c4_containers" => {
                    let arguments = request.arguments.unwrap_or_default();
                    let result = Self::handle_generate_c4_containers(serde_json::Value::Object(arguments), &project_root);
                    match result {
                        Ok(output) => Ok(CallToolResult::success(vec![Content::text(output)])),
                        Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
                    }
                }
                "generate_c4_components" => {
                    let arguments = request.arguments.unwrap_or_default();
                    let result = self.handle_generate_c4_components(serde_json::Value::Object(arguments));
                    match result {
                        Ok(output) => Ok(CallToolResult::success(vec![Content::text(output)])),
                        Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
                    }
                }
                _ => {
                    // Delegate all other tools to the inner handler
                    self.inner.call_tool(request, context).await
                }
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
    fn handle_generate_c4_code(&self, arguments: serde_json::Value) -> anyhow::Result<String> {
        let input: GenerateC4CodeInput = serde_json::from_value(arguments)?;
        let call_graph = self.inner.get_call_graph()?;
        let output = handle_generate_c4_code(input, &call_graph)?;
        Ok(serde_json::to_string(&output)?)
    }

    fn handle_generate_c4_containers(
        arguments: serde_json::Value,
        project_root: &Path,
    ) -> anyhow::Result<String> {
        let input: GenerateC4ContainersInput = serde_json::from_value(arguments)?;
        let directory = input.directory.as_ref()
            .map(|d| project_root.join(d))
            .unwrap_or_else(|| project_root.to_path_buf());

        // Containers can be parsed without a CallGraph (just from config files)
        let output = handle_generate_c4_containers(input, &directory, None)?;
        Ok(serde_json::to_string(&output)?)
    }

    fn handle_generate_c4_components(&self, arguments: serde_json::Value) -> anyhow::Result<String> {
        let input: GenerateC4ComponentsInput = serde_json::from_value(arguments)?;
        let call_graph = self.inner.get_call_graph()?;
        let output = handle_generate_c4_components(input, &call_graph)?;
        Ok(serde_json::to_string(&output)?)
    }
}
