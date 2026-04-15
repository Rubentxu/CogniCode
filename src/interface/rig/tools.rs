//! CogniCode Tools for rig-core
//!
//! Implements the Tool trait from rig-core for all CogniCode operations.

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[cfg(feature = "rig")]
use rig::completion::ToolDefinition;
#[cfg(feature = "rig")]
use rig::tool::Tool;
#[cfg(feature = "rig")]
use rig::tool::ToolDyn;

use crate::application::workspace_session::WorkspaceSession;
use crate::application::dto::{
    ReadFileRequest, ReadMode,
    WriteFileRequest,
    EditFileRequest, FileEdit,
    SearchContentRequest,
    ListFilesRequest,
};

// =============================================================================
// Tool Error
// =============================================================================

/// Error type for tool operations
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Tool call failed: {0}")]
    ToolCallError(String),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Workspace error: {0}")]
    WorkspaceError(String),
}

impl From<crate::application::workspace_session::WorkspaceError> for ToolError {
    fn from(e: crate::application::workspace_session::WorkspaceError) -> Self {
        ToolError::WorkspaceError(e.to_string())
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Build a parameter schema for tool definitions
fn param(name: &str, type_str: &str, description: &str) -> JsonValue {
    serde_json::json!({
        "type": type_str,
        "description": description
    })
}

/// Build the full parameters object for tool definitions
fn build_parameters(required: &[&str], properties: JsonValue) -> JsonValue {
    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required
    })
}

/// Create a tool definition
#[cfg(feature = "rig")]
fn create_tool_definition(
    name: &str,
    description: &str,
    parameters: JsonValue,
) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: description.to_string(),
        parameters,
    }
}

// =============================================================================
// File Operation Tools
// =============================================================================

// -----------------------------------------------------------------------------
// ReadFileTool
// -----------------------------------------------------------------------------

pub struct ReadFileTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReadFileArgs {
    pub path: String,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub start_line: Option<u32>,
    #[serde(default)]
    pub end_line: Option<u32>,
}

impl ReadFileTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for ReadFileTool {
    const NAME: &'static str = "read_file";
    type Error = ToolError;
    type Args = ReadFileArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            let properties = serde_json::json!({
                "path": param("path", "string", "Path to the file to read"),
                "mode": param("mode", "string", "Read mode: raw, outline, symbols, or compressed"),
                "start_line": param("start_line", "number", "Starting line number (1-indexed)"),
                "end_line": param("end_line", "number", "Ending line number (inclusive)")
            });
            create_tool_definition(
                Self::NAME,
                "Read contents of a file with optional line range and mode",
                build_parameters(&["path"], properties),
            )
        }
    }

    fn call(&self, args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let read_mode = args.mode.as_deref().unwrap_or("raw");
            let mode = match read_mode {
                "outline" => ReadMode::Outline,
                "symbols" => ReadMode::Symbols,
                "compressed" => ReadMode::Compressed,
                _ => ReadMode::Raw,
            };

            let request = ReadFileRequest {
                path: args.path,
                mode: Some(mode.to_string()),
                start_line: args.start_line,
                end_line: args.end_line,
                chunk_size: None,
                continuation_token: None,
            };

            let result = self.session.read_file(request).await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

// -----------------------------------------------------------------------------
// SearchContentTool
// -----------------------------------------------------------------------------

pub struct SearchContentTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchContentArgs {
    pub pattern: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub file_glob: Option<String>,
    #[serde(default)]
    pub max_results: Option<usize>,
}

impl SearchContentTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for SearchContentTool {
    const NAME: &'static str = "search_content";
    type Error = ToolError;
    type Args = SearchContentArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            let properties = serde_json::json!({
                "pattern": param("pattern", "string", "Search pattern or regex"),
                "path": param("path", "string", "Directory path to search in"),
                "file_glob": param("file_glob", "string", "Glob pattern for files to search"),
                "max_results": param("max_results", "number", "Maximum number of results")
            });
            create_tool_definition(
                Self::NAME,
                "Search for content within files using pattern matching",
                build_parameters(&["pattern"], properties),
            )
        }
    }

    fn call(&self, args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let request = SearchContentRequest {
                pattern: args.pattern,
                path: args.path,
                file_glob: args.file_glob,
                regex: None,
                case_insensitive: None,
                max_results: args.max_results,
                context_lines: None,
            };

            let result = self.session.search_content(request).await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

// -----------------------------------------------------------------------------
// ListFilesTool
// -----------------------------------------------------------------------------

pub struct ListFilesTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListFilesArgs {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub glob: Option<String>,
    #[serde(default)]
    pub recursive: Option<bool>,
}

impl ListFilesTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for ListFilesTool {
    const NAME: &'static str = "list_files";
    type Error = ToolError;
    type Args = ListFilesArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            let properties = serde_json::json!({
                "path": param("path", "string", "Directory path to list files from"),
                "glob": param("glob", "string", "Glob pattern for filtering files"),
                "recursive": param("recursive", "boolean", "Whether to list recursively")
            });
            create_tool_definition(
                Self::NAME,
                "List files in a directory with optional filtering",
                build_parameters(&[], properties),
            )
        }
    }

    fn call(&self, args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let request = ListFilesRequest {
                path: args.path,
                glob: args.glob,
                offset: None,
                limit: None,
                recursive: args.recursive,
                max_depth: None,
            };

            let result = self.session.list_files(request).await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

// -----------------------------------------------------------------------------
// WriteFileTool
// -----------------------------------------------------------------------------

pub struct WriteFileTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WriteFileArgs {
    pub path: String,
    pub content: String,
    #[serde(default)]
    pub create_dirs: Option<bool>,
}

impl WriteFileTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for WriteFileTool {
    const NAME: &'static str = "write_file";
    type Error = ToolError;
    type Args = WriteFileArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            let properties = serde_json::json!({
                "path": param("path", "string", "Path to the file to write"),
                "content": param("content", "string", "Content to write to the file"),
                "create_dirs": param("create_dirs", "boolean", "Whether to create parent directories")
            });
            create_tool_definition(
                Self::NAME,
                "Write content to a file atomically",
                build_parameters(&["path", "content"], properties),
            )
        }
    }

    fn call(&self, args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let request = WriteFileRequest {
                path: args.path,
                content: args.content,
                create_dirs: args.create_dirs,
            };

            let result = self.session.write_file(request).await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

// -----------------------------------------------------------------------------
// EditFileTool
// -----------------------------------------------------------------------------

pub struct EditFileTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileEditArg {
    pub old_text: String,
    pub new_text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EditFileArgs {
    pub path: String,
    pub edits: Vec<FileEditArg>,
}

impl EditFileTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for EditFileTool {
    const NAME: &'static str = "edit_file";
    type Error = ToolError;
    type Args = EditFileArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            let properties = serde_json::json!({
                "path": param("path", "string", "Path to the file to edit"),
                "edits": {
                    "type": "array",
                    "description": "List of edit operations",
                    "items": {
                        "type": "object",
                        "properties": {
                            "old_text": param("old_text", "string", "Exact text to replace"),
                            "new_text": param("new_text", "string", "Replacement text")
                        }
                    }
                }
            });
            create_tool_definition(
                Self::NAME,
                "Apply string-replacement edits to a file",
                build_parameters(&["path", "edits"], properties),
            )
        }
    }

    fn call(&self, args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let edits: Vec<FileEdit> = args.edits.into_iter().map(|e| FileEdit {
                old_string: e.old_text,
                new_string: e.new_text,
            }).collect();

            let request = EditFileRequest {
                path: args.path,
                edits,
            };

            let result = self.session.edit_file(request).await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

// =============================================================================
// Code Analysis Tools
// =============================================================================

// -----------------------------------------------------------------------------
// GetFileSymbolsTool
// -----------------------------------------------------------------------------

pub struct GetFileSymbolsTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetFileSymbolsArgs {
    pub file_path: String,
}

impl GetFileSymbolsTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for GetFileSymbolsTool {
    const NAME: &'static str = "get_file_symbols";
    type Error = ToolError;
    type Args = GetFileSymbolsArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            let properties = serde_json::json!({
                "file_path": param("file_path", "string", "Path to the source file")
            });
            create_tool_definition(
                Self::NAME,
                "Get all symbols (functions, classes, etc.) from a file",
                build_parameters(&["file_path"], properties),
            )
        }
    }

    fn call(&self, args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let result = self.session.get_file_symbols(&args.file_path).await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

// -----------------------------------------------------------------------------
// GetOutlineTool
// -----------------------------------------------------------------------------

pub struct GetOutlineTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetOutlineArgs {
    pub file_path: String,
    #[serde(default)]
    pub include_private: Option<bool>,
}

impl GetOutlineTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for GetOutlineTool {
    const NAME: &'static str = "get_outline";
    type Error = ToolError;
    type Args = GetOutlineArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            let properties = serde_json::json!({
                "file_path": param("file_path", "string", "Path to the source file"),
                "include_private": param("include_private", "boolean", "Include private symbols starting with _")
            });
            create_tool_definition(
                Self::NAME,
                "Get an outline (hierarchical structure) of a file showing symbols and their relationships",
                build_parameters(&["file_path"], properties),
            )
        }
    }

    fn call(&self, args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let include_private = args.include_private.unwrap_or(false);
            let result = self.session.get_outline(&args.file_path, include_private).await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(result)
        }
    }
}

// -----------------------------------------------------------------------------
// GetComplexityTool
// -----------------------------------------------------------------------------

pub struct GetComplexityTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetComplexityArgs {
    pub file_path: String,
    #[serde(default)]
    pub function_name: Option<String>,
}

impl GetComplexityTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for GetComplexityTool {
    const NAME: &'static str = "get_complexity";
    type Error = ToolError;
    type Args = GetComplexityArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            let properties = serde_json::json!({
                "file_path": param("file_path", "string", "Path to the source file"),
                "function_name": param("function_name", "string", "Optional specific function to analyze")
            });
            create_tool_definition(
                Self::NAME,
                "Get complexity metrics for a file or specific function (cyclomatic, cognitive, lines of code)",
                build_parameters(&["file_path"], properties),
            )
        }
    }

    fn call(&self, args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let result = self.session.get_complexity(&args.file_path, args.function_name.as_deref()).await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

// -----------------------------------------------------------------------------
// SemanticSearchTool
// -----------------------------------------------------------------------------

pub struct SemanticSearchTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SemanticSearchArgs {
    pub query: String,
    #[serde(default)]
    pub max_results: Option<usize>,
}

impl SemanticSearchTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for SemanticSearchTool {
    const NAME: &'static str = "semantic_search";
    type Error = ToolError;
    type Args = SemanticSearchArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            let properties = serde_json::json!({
                "query": param("query", "string", "Semantic search query"),
                "max_results": param("max_results", "number", "Maximum number of results to return")
            });
            create_tool_definition(
                Self::NAME,
                "Perform semantic search for symbols using natural language query",
                build_parameters(&["query"], properties),
            )
        }
    }

    fn call(&self, args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let max_results = args.max_results.unwrap_or(10);
            let result = self.session.semantic_search(&args.query, max_results).await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

// =============================================================================
// Graph Operation Tools
// =============================================================================

// -----------------------------------------------------------------------------
// BuildGraphTool
// -----------------------------------------------------------------------------

pub struct BuildGraphTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildGraphArgs {
    #[serde(default)]
    pub strategy: Option<String>,
}

impl BuildGraphTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for BuildGraphTool {
    const NAME: &'static str = "build_graph";
    type Error = ToolError;
    type Args = BuildGraphArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            let properties = serde_json::json!({
                "strategy": param("strategy", "string", "Build strategy: lightweight, on_demand, per_file, or full")
            });
            create_tool_definition(
                Self::NAME,
                "Build the call graph using the specified strategy",
                build_parameters(&[], properties),
            )
        }
    }

    fn call(&self, args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let strategy = args.strategy.unwrap_or_else(|| "full".to_string());
            self.session.build_graph(&strategy).await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(format!("Graph built successfully with strategy: {}", strategy))
        }
    }
}

// -----------------------------------------------------------------------------
// GetCallHierarchyTool
// -----------------------------------------------------------------------------

pub struct GetCallHierarchyTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetCallHierarchyArgs {
    pub symbol: String,
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default)]
    pub depth: Option<usize>,
}

impl GetCallHierarchyTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for GetCallHierarchyTool {
    const NAME: &'static str = "get_call_hierarchy";
    type Error = ToolError;
    type Args = GetCallHierarchyArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            let properties = serde_json::json!({
                "symbol": param("symbol", "string", "Symbol name to get call hierarchy for"),
                "direction": param("direction", "string", "Direction: incoming (callers), outgoing (callees), or both"),
                "depth": param("depth", "number", "Maximum traversal depth")
            });
            create_tool_definition(
                Self::NAME,
                "Get call hierarchy for a symbol showing callers and/or callees",
                build_parameters(&["symbol"], properties),
            )
        }
    }

    fn call(&self, args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let direction = args.direction.unwrap_or_else(|| "both".to_string());
            let depth = args.depth.unwrap_or(3);

            let result = self.session.get_call_hierarchy(&args.symbol, &direction, depth).await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

// -----------------------------------------------------------------------------
// AnalyzeImpactTool
// -----------------------------------------------------------------------------

pub struct AnalyzeImpactTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnalyzeImpactArgs {
    pub symbol: String,
}

impl AnalyzeImpactTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for AnalyzeImpactTool {
    const NAME: &'static str = "analyze_impact";
    type Error = ToolError;
    type Args = AnalyzeImpactArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            let properties = serde_json::json!({
                "symbol": param("symbol", "string", "Symbol name to analyze impact for")
            });
            create_tool_definition(
                Self::NAME,
                "Analyze the impact of changing a symbol - what other symbols and files would be affected",
                build_parameters(&["symbol"], properties),
            )
        }
    }

    fn call(&self, args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let result = self.session.analyze_impact(&args.symbol).await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

// -----------------------------------------------------------------------------
// GetEntryPointsTool
// -----------------------------------------------------------------------------

pub struct GetEntryPointsTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetEntryPointsArgs {
    // Empty - no required args
}

impl GetEntryPointsTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for GetEntryPointsTool {
    const NAME: &'static str = "get_entry_points";
    type Error = ToolError;
    type Args = GetEntryPointsArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            create_tool_definition(
                Self::NAME,
                "Get entry points - symbols with no incoming edges (likely main functions, exports)",
                serde_json::json!({
                    "type": "object",
                    "properties": serde_json::json!({}),
                    "required": []
                }),
            )
        }
    }

    fn call(&self, _args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let result = self.session.get_entry_points().await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

// -----------------------------------------------------------------------------
// GetLeafFunctionsTool
// -----------------------------------------------------------------------------

pub struct GetLeafFunctionsTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetLeafFunctionsArgs {
    // Empty - no required args
}

impl GetLeafFunctionsTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for GetLeafFunctionsTool {
    const NAME: &'static str = "get_leaf_functions";
    type Error = ToolError;
    type Args = GetLeafFunctionsArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            create_tool_definition(
                Self::NAME,
                "Get leaf functions - symbols with no outgoing edges (likely utility functions, leaf nodes)",
                serde_json::json!({
                    "type": "object",
                    "properties": serde_json::json!({}),
                    "required": []
                }),
            )
        }
    }

    fn call(&self, _args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let result = self.session.get_leaf_functions().await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

// -----------------------------------------------------------------------------
// TracePathTool
// -----------------------------------------------------------------------------

pub struct TracePathTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TracePathArgs {
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub max_depth: Option<usize>,
}

impl TracePathTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for TracePathTool {
    const NAME: &'static str = "trace_path";
    type Error = ToolError;
    type Args = TracePathArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            let properties = serde_json::json!({
                "source": param("source", "string", "Source symbol name"),
                "target": param("target", "string", "Target symbol name"),
                "max_depth": param("max_depth", "number", "Maximum path length to search")
            });
            create_tool_definition(
                Self::NAME,
                "Trace execution path between two symbols through the call graph",
                build_parameters(&["source", "target"], properties),
            )
        }
    }

    fn call(&self, args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let max_depth = args.max_depth.unwrap_or(10);
            let result = self.session.trace_path(&args.source, &args.target, max_depth).await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

// -----------------------------------------------------------------------------
// ExportMermaidTool
// -----------------------------------------------------------------------------

pub struct ExportMermaidTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExportMermaidArgs {
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub root_symbol: Option<String>,
}

impl ExportMermaidTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for ExportMermaidTool {
    const NAME: &'static str = "export_mermaid";
    type Error = ToolError;
    type Args = ExportMermaidArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            let properties = serde_json::json!({
                "format": param("format", "string", "Output format (currently only 'code' is supported)"),
                "theme": param("theme", "string", "Mermaid theme for rendering"),
                "root_symbol": param("root_symbol", "string", "Optional root symbol to center the graph around")
            });
            create_tool_definition(
                Self::NAME,
                "Export the call graph as a Mermaid diagram",
                build_parameters(&[], properties),
            )
        }
    }

    fn call(&self, args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let format = args.format.as_deref().unwrap_or("code");
            let result = self.session.export_mermaid(format, args.theme.as_deref(), args.root_symbol.as_deref()).await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(result)
        }
    }
}

// -----------------------------------------------------------------------------
// CheckArchitectureTool
// -----------------------------------------------------------------------------

pub struct CheckArchitectureTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CheckArchitectureArgs {
    #[serde(default)]
    pub scope: Option<String>,
}

impl CheckArchitectureTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for CheckArchitectureTool {
    const NAME: &'static str = "check_architecture";
    type Error = ToolError;
    type Args = CheckArchitectureArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            let properties = serde_json::json!({
                "scope": param("scope", "string", "Optional scope to check (e.g., module name)")
            });
            create_tool_definition(
                Self::NAME,
                "Check architecture for cycles and violations in the call graph",
                build_parameters(&[], properties),
            )
        }
    }

    fn call(&self, args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let result = self.session.check_architecture(args.scope.as_deref()).await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

// =============================================================================
// Navigation Tools
// =============================================================================

// -----------------------------------------------------------------------------
// FindUsagesTool
// -----------------------------------------------------------------------------

pub struct FindUsagesTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FindUsagesArgs {
    pub file: String,
    pub line: u32,
    pub column: u32,
    #[serde(default)]
    pub include_declaration: Option<bool>,
}

impl FindUsagesTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for FindUsagesTool {
    const NAME: &'static str = "find_usages";
    type Error = ToolError;
    type Args = FindUsagesArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            let properties = serde_json::json!({
                "file": param("file", "string", "File path where the symbol is located"),
                "line": param("line", "number", "Line number of the symbol (1-indexed)"),
                "column": param("column", "number", "Column number of the symbol (1-indexed)"),
                "include_declaration": param("include_declaration", "boolean", "Whether to include the declaration itself")
            });
            create_tool_definition(
                Self::NAME,
                "Find all usages of a symbol at a given location",
                build_parameters(&["file", "line", "column"], properties),
            )
        }
    }

    fn call(&self, args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let include_decl = args.include_declaration.unwrap_or(true);
            let result = self.session.find_references(&args.file, args.line, args.column, include_decl).await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

// -----------------------------------------------------------------------------
// GetSymbolCodeTool
// -----------------------------------------------------------------------------

pub struct GetSymbolCodeTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetSymbolCodeArgs {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

impl GetSymbolCodeTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for GetSymbolCodeTool {
    const NAME: &'static str = "get_symbol_code";
    type Error = ToolError;
    type Args = GetSymbolCodeArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            let properties = serde_json::json!({
                "file_path": param("file_path", "string", "Path to the source file"),
                "line": param("line", "number", "Line number of the symbol (1-indexed)"),
                "column": param("column", "number", "Column number of the symbol (1-indexed)")
            });
            create_tool_definition(
                Self::NAME,
                "Get the full source code of a symbol at a specific location, including its docstring",
                build_parameters(&["file_path", "line", "column"], properties),
            )
        }
    }

    fn call(&self, args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let result = self.session.get_symbol_code(&args.file_path, args.line, args.column).await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(result)
        }
    }
}

// =============================================================================
// Refactor Tools
// =============================================================================

// -----------------------------------------------------------------------------
// SafeRefactorTool
// -----------------------------------------------------------------------------

pub struct SafeRefactorTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SafeRefactorArgs {
    pub symbol: String,
    pub new_name: String,
    pub file: String,
}

impl SafeRefactorTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for SafeRefactorTool {
    const NAME: &'static str = "safe_refactor";
    type Error = ToolError;
    type Args = SafeRefactorArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            let properties = serde_json::json!({
                "symbol": param("symbol", "string", "Current symbol name to rename"),
                "new_name": param("new_name", "string", "New name for the symbol"),
                "file": param("file", "string", "File path where the symbol is defined")
            });
            create_tool_definition(
                Self::NAME,
                "Safely rename a symbol across the entire codebase with preview and validation",
                build_parameters(&["symbol", "new_name", "file"], properties),
            )
        }
    }

    fn call(&self, args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let result = self.session.rename_symbol(&args.symbol, &args.new_name, &args.file).await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

// -----------------------------------------------------------------------------
// ValidateSyntaxTool
// -----------------------------------------------------------------------------

pub struct ValidateSyntaxTool {
    session: Arc<WorkspaceSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ValidateSyntaxArgs {
    pub file_path: String,
}

impl ValidateSyntaxTool {
    pub fn new(session: Arc<WorkspaceSession>) -> Self {
        Self { session }
    }
}

#[cfg(feature = "rig")]
impl Tool for ValidateSyntaxTool {
    const NAME: &'static str = "validate_syntax";
    type Error = ToolError;
    type Args = ValidateSyntaxArgs;
    type Output = String;

    fn definition(&self, _prompt: String) -> impl std::future::Future<Output = ToolDefinition> + Send + Sync {
        async move {
            let properties = serde_json::json!({
                "file_path": param("file_path", "string", "Path to the file to validate")
            });
            create_tool_definition(
                Self::NAME,
                "Validate the syntax of a file and report any errors or warnings",
                build_parameters(&["file_path"], properties),
            )
        }
    }

    fn call(&self, args: Self::Args) -> impl std::future::Future<Output = Result<Self::Output, Self::Error>> + Send {
        async move {
            let result = self.session.validate_syntax(&args.file_path).await
                .map_err(|e| ToolError::ToolCallError(e.to_string()))?;

            Ok(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
    }
}

// =============================================================================
// Convenience Function
// =============================================================================

/// Create all CogniCode tools with the given session
#[cfg(feature = "rig")]
pub fn create_all_tools(session: Arc<WorkspaceSession>) -> Vec<Box<dyn ToolDyn>> {
    vec![
        Box::new(ReadFileTool::new(session.clone())),
        Box::new(SearchContentTool::new(session.clone())),
        Box::new(ListFilesTool::new(session.clone())),
        Box::new(WriteFileTool::new(session.clone())),
        Box::new(EditFileTool::new(session.clone())),
        Box::new(GetFileSymbolsTool::new(session.clone())),
        Box::new(GetOutlineTool::new(session.clone())),
        Box::new(GetComplexityTool::new(session.clone())),
        Box::new(SemanticSearchTool::new(session.clone())),
        Box::new(BuildGraphTool::new(session.clone())),
        Box::new(GetCallHierarchyTool::new(session.clone())),
        Box::new(AnalyzeImpactTool::new(session.clone())),
        Box::new(GetEntryPointsTool::new(session.clone())),
        Box::new(GetLeafFunctionsTool::new(session.clone())),
        Box::new(TracePathTool::new(session.clone())),
        Box::new(ExportMermaidTool::new(session.clone())),
        Box::new(CheckArchitectureTool::new(session.clone())),
        Box::new(FindUsagesTool::new(session.clone())),
        Box::new(GetSymbolCodeTool::new(session.clone())),
        Box::new(SafeRefactorTool::new(session.clone())),
        Box::new(ValidateSyntaxTool::new(session.clone())),
    ]
}
