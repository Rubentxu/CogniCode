//! RMCP Adapter - Bridge between rmcp SDK and CogniCode handlers
//!
//! This module provides the CogniCodeHandler which implements the rmcp ServerHandler trait,
//! allowing the CogniCode MCP server to use the official rmcp SDK for transport.

use crate::application::services::file_operations::FileOperationsService;
use crate::infrastructure::verification::RustVerifier;
use crate::interface::mcp::error::{InterfaceError, InterfaceResult};
use crate::interface::mcp::handlers::HandlerContext;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ListToolsResult, ServerCapabilities,
    ServerInfo, Tool,
};
use rmcp::service::RoleServer;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// CogniCodeHandler implements the rmcp ServerHandler trait
///
/// This handler bridges the rmcp SDK with the existing CogniCode handler functions.
/// It maintains a persistent HandlerContext that survives across requests to avoid
/// rebuilding the analysis graph, plus a cancellation flag.
#[derive(Debug)]
pub struct CogniCodeHandler {
    /// Persistent handler context - created once and shared across all requests
    ctx: Arc<HandlerContext>,
    /// Cancellation token for handling cancelled requests
    cancellation_token: Arc<AtomicBool>,
}

impl CogniCodeHandler {
    /// Creates a new CogniCodeHandler with InMemoryGraphStore (no persistence)
    pub fn new(project_root: PathBuf) -> Self {
        let cancellation_token = Arc::new(AtomicBool::new(false));
        let mut ctx = Self::build_ctx(project_root);
        ctx.cancellation_token = cancellation_token.clone();
        Self {
            ctx: Arc::new(ctx),
            cancellation_token,
        }
    }

    /// Creates a new CogniCodeHandler with a custom GraphStore (SQLite for persistence)
    pub fn with_graph_store(
        project_root: PathBuf,
        store: Arc<dyn crate::domain::traits::GraphStore>,
    ) -> Self {
        let cancellation_token = Arc::new(AtomicBool::new(false));
        let mut ctx = HandlerContext::with_graph_store(project_root, store);
        ctx.cancellation_token = cancellation_token.clone();
        Self {
            ctx: Arc::new(ctx),
            cancellation_token,
        }
    }

    /// Creates a new CogniCodeHandler with a `GraphRepository`
    /// wired into the handler context. Used by the MCP binary
    /// when the user passes `--database-url` — the binary
    /// builds a `PgGraphRepository` from a `sqlx::PgPool` and
    /// hands it in here so the `graph_search` / `docs_ingest`
    /// tools can route to PG.
    ///
    /// Gated behind the `multimodal` Cargo feature: callers
    /// that don't enable `multimodal` see no method.
    #[cfg(feature = "multimodal")]
    pub fn with_graph_repository(
        project_root: PathBuf,
        repo: std::sync::Arc<dyn crate::domain::GraphRepository>,
    ) -> Self {
        let cancellation_token = Arc::new(AtomicBool::new(false));
        let mut ctx = HandlerContext::with_graph_repository(project_root, repo);
        ctx.cancellation_token = cancellation_token.clone();
        Self {
            ctx: Arc::new(ctx),
            cancellation_token,
        }
    }

    fn build_ctx(project_root: PathBuf) -> HandlerContext {
        let canonical_root =
            std::fs::canonicalize(&project_root).unwrap_or_else(|_| project_root.clone());

        // Create validator and FileOperationsService for shared use across handlers
        let validator = Arc::new(
            crate::interface::mcp::security::InputValidator::new()
                .with_workspace(vec![canonical_root.clone()]),
        );
        let file_ops_service = Arc::new(FileOperationsService::new(
            canonical_root.to_string_lossy().as_ref(),
            validator,
            Arc::new(RustVerifier::new()),
        ));

        HandlerContext::builder()
            .with_working_dir(canonical_root)
            .with_file_ops_service(file_ops_service)
            .build()
    }

    /// Get the current CallGraph from the store
    pub fn get_call_graph(
        &self,
    ) -> anyhow::Result<crate::domain::aggregates::call_graph::CallGraph> {
        self.ctx
            .get_graph_store()
            .load_graph()
            .map_err(|e| anyhow::anyhow!("Graph store error: {}", e))?
            .ok_or_else(|| anyhow::anyhow!("No call graph available. Run build_graph first."))
    }
}

impl ServerHandler for CogniCodeHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(rmcp::model::Implementation::new(
            "cognicode",
            env!("CARGO_PKG_VERSION"),
        ))
        .with_protocol_version(rmcp::model::ProtocolVersion::V_2025_03_26)
    }

    fn list_tools(
        &self,
        request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, rmcp::ErrorData>> + Send + '_
    {
        async move {
            use base64::Engine;

            // Parse cursor for pagination (base64 encoded offset)
            let cursor_offset = request
                .as_ref()
                .and_then(|p| p.cursor.as_ref())
                .and_then(|c| base64::engine::general_purpose::STANDARD.decode(c).ok())
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0);

            const PAGE_SIZE: usize = 20;

            // All tools with annotations - same as server.rs handle_tools_list
            let all_tools = vec![
                Tool::new(
                    "build_graph",
                    "Build the call graph for a project directory. Must be called before get_call_hierarchy, analyze_impact, or check_architecture.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "directory": {
                                "type": "string",
                                "description": "Path to project directory to analyze (default: current working directory)"
                            }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "get_file_symbols",
                    "Extract symbols (functions, classes, variables) from a source file. Set compressed=true for natural language summary.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "file_path": { "type": "string", "description": "Path to the source file" },
                            "compressed": { "type": "boolean", "description": "Return compressed natural language summary instead of JSON (default: false)" }
                        },
                        "required": ["file_path"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "get_call_hierarchy",
                    "Traverse call graph to find callers (incoming) or callees (outgoing). Requires build_graph first.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "symbol_name": { "type": "string", "description": "Fully qualified symbol name" },
                            "direction": { "type": "string", "enum": ["incoming", "outgoing"], "description": "Traverse direction" },
                            "depth": { "type": "integer", "description": "Traversal depth (default: 1)" },
                            "compressed": { "type": "boolean", "description": "Return compressed summary" }
                        },
                        "required": ["symbol_name", "direction"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "analyze_impact",
                    "Analyze the impact of changing a symbol. Returns impacted files and risk level.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "symbol_name": { "type": "string", "description": "Symbol to analyze" },
                            "compressed": { "type": "boolean", "description": "Return compressed summary" }
                        },
                        "required": ["symbol_name"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "check_architecture",
                    "Detect cycles and architecture violations using Tarjan SCC algorithm.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "scope": { "type": "string", "description": "Optional scope to check" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "find_usages",
                    "Find all usages of a symbol across the project.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "symbol_name": { "type": "string", "description": "Symbol to search" },
                            "include_declaration": { "type": "boolean", "description": "Include definition (default: true)" }
                        },
                        "required": ["symbol_name"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "get_complexity",
                    "Calculate code complexity metrics (cyclomatic, cognitive, nesting).",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "file_path": { "type": "string", "description": "Path to source file" },
                            "function_name": { "type": "string", "description": "Optional specific function" }
                        },
                        "required": ["file_path"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "get_entry_points",
                    "Find symbols with no incoming edges (entry points in the call graph).",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "compressed": { "type": "boolean", "description": "Return compressed natural language summary instead of JSON (default: false)" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "get_leaf_functions",
                    "Find symbols with no outgoing edges (leaf functions in the call graph).",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "compressed": { "type": "boolean", "description": "Return compressed natural language summary instead of JSON (default: false)" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "trace_path",
                    "Find execution path between two symbols using BFS.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "source": { "type": "string", "description": "Source symbol name (function or method)" },
                            "target": { "type": "string", "description": "Target symbol name (function or method)" },
                            "max_depth": { "type": "integer", "description": "Maximum depth for path search (default: 10)" }
                        },
                        "required": ["source", "target"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "export_mermaid",
                    "Export call graph or subgraph as Mermaid flowchart. Optionally render to SVG with a theme.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "root_symbol": { "type": "string", "description": "Subgraph root symbol (optional - if not provided, exports entire graph)" },
                            "max_depth": { "type": "integer", "description": "Maximum depth for traversal (default: 3)" },
                            "include_external": { "type": "boolean", "description": "Include external dependencies (default: false)" },
                            "theme": { "type": "string", "description": "Theme for SVG rendering" },
                            "format": { "type": "string", "enum": ["code", "svg"], "description": "Output format" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "get_hot_paths",
                    "Find functions with highest fan-in (most called functions).",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "limit": { "type": "integer", "description": "Number of hot paths to return (default: 10)" },
                            "min_fan_in": { "type": "integer", "description": "Minimum fan-in threshold (default: 2)" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "build_lightweight_index",
                    "Build a lightweight symbol index for fast lookups.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "directory": { "type": "string", "description": "Directory to build the index for (default: current working directory)" },
                            "strategy": { "type": "string", "enum": ["lightweight", "on_demand", "per_file", "full"], "description": "Index strategy to use (default: lightweight)" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "query_symbol_index",
                    "Query the symbol index to find locations of a symbol by name (case-insensitive).",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "symbol_name": { "type": "string", "description": "Symbol name to query (case-insensitive)" },
                            "directory": { "type": "string", "description": "Directory to search in (default: current working directory)" }
                        },
                        "required": ["symbol_name"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "build_call_subgraph",
                    "Build an on-demand call subgraph centered on a symbol.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "symbol_name": { "type": "string", "description": "Symbol name to build subgraph around" },
                            "depth": { "type": "integer", "description": "Traversal depth (default: 3)" },
                            "direction": { "type": "string", "enum": ["in", "out", "both"], "description": "Traversal direction (default: both)" },
                            "directory": { "type": "string", "description": "Directory to search in (default: current working directory)" }
                        },
                        "required": ["symbol_name"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "get_per_file_graph",
                    "Get the call graph for a specific file.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "file_path": { "type": "string", "description": "File path to get graph for" }
                        },
                        "required": ["file_path"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "merge_file_graphs",
                    "Merge call graphs from multiple files into a single graph.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "file_paths": { "type": "array", "items": { "type": "string" }, "description": "List of file paths to merge" }
                        },
                        "required": ["file_paths"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "get_outline",
                    "Get a hierarchical outline of symbols in a source file.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "file_path": { "type": "string", "description": "Path to the source file" },
                            "include_private": { "type": "boolean", "description": "Include private symbols starting with _ (default: true)" },
                            "include_tests": { "type": "boolean", "description": "Include test symbols starting with test_ (default: true)" }
                        },
                        "required": ["file_path"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "get_symbol_code",
                    "Get the full source code of a symbol at a given location, including docstrings.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "file": { "type": "string", "description": "Path to the source file" },
                            "line": { "type": "integer", "description": "Line number (1-indexed)" },
                            "col": { "type": "integer", "description": "Column number (0-indexed)" }
                        },
                        "required": ["file", "line", "col"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "semantic_search",
                    "Search for symbols with fuzzy matching and kind filtering.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Search query string" },
                            "kinds": { "type": "array", "items": { "type": "string" }, "description": "Filter by symbol kinds" },
                            "max_results": { "type": "integer", "description": "Maximum results to return (default: 50)" }
                        },
                        "required": ["query"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "find_usages_with_context",
                    "Find all usages of a symbol with surrounding context lines.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "symbol": { "type": "string", "description": "Symbol name to search" },
                            "context_lines": { "type": "integer", "description": "Number of context lines around each reference (default: 3)" },
                            "include_declaration": { "type": "boolean", "description": "Include the declaration (default: true)" }
                        },
                        "required": ["symbol"]
                    }).as_object().cloned().unwrap()),
                ),
                // LSP Navigation tools
                Tool::new(
                    "go_to_definition",
                    "Navigate to the definition of a symbol using LSP.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "file_path": { "type": "string" },
                            "line": { "type": "integer" },
                            "column": { "type": "integer" }
                        },
                        "required": ["file_path", "line", "column"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "hover",
                    "Get type information and documentation for a symbol at a position using LSP.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "file_path": { "type": "string" },
                            "line": { "type": "integer" },
                            "column": { "type": "integer" }
                        },
                        "required": ["file_path", "line", "column"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "find_references",
                    "Find all references to a symbol using LSP.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "file_path": { "type": "string" },
                            "line": { "type": "integer" },
                            "column": { "type": "integer" },
                            "include_declaration": { "type": "boolean", "default": true }
                        },
                        "required": ["file_path", "line", "column"]
                    }).as_object().cloned().unwrap()),
                ),
                // File operation tools
                Tool::new(
                    "read_file",
                    "Smart file reader with semantic modes.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Path to the file to read (required)" },
                            "start_line": { "type": "integer", "description": "Start line for partial read (1-indexed, default: 1)" },
                            "end_line": { "type": "integer", "description": "End line for partial read (1-indexed, default: last line)" },
                            "mode": { "type": "string", "enum": ["raw", "outline", "symbols", "compressed"], "description": "Read mode" },
                            "chunk_size": { "type": "integer", "description": "Chunk size for streaming reads (optional)" },
                            "continuation_token": { "type": "string", "description": "Continuation token for pagination (optional)" }
                        },
                        "required": ["path"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "search_content",
                    "Search file contents with .gitignore awareness.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "pattern": { "type": "string", "description": "Search pattern (required)" },
                            "path": { "type": "string", "description": "Path to search within (optional, defaults to workspace root)" },
                            "file_glob": { "type": "string", "description": "Glob pattern to filter files (e.g., '*.rs')" },
                            "regex": { "type": "boolean", "description": "Whether to treat pattern as regex (default: true)" },
                            "case_insensitive": { "type": "boolean", "description": "Case insensitive search (default: false)" },
                            "max_results": { "type": "integer", "description": "Maximum number of results to return (default: 50)" },
                            "context_lines": { "type": "integer", "description": "Number of context lines around matches (default: 2)" }
                        },
                        "required": ["pattern"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "list_files",
                    "List project files with .gitignore awareness.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Path to list (optional, defaults to workspace root)" },
                            "glob": { "type": "string", "description": "Glob pattern to filter results (e.g., '**/*.rs')" },
                            "offset": { "type": "integer", "description": "Pagination offset (default: 0)" },
                            "limit": { "type": "integer", "description": "Maximum number of results (default: 100)" },
                            "recursive": { "type": "boolean", "description": "Whether to list files recursively (default: true)" },
                            "max_depth": { "type": "integer", "description": "Maximum depth for recursive traversal" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "retrieve_and_verify",
                    "Search for code matching a query and verify Rust files via sandboxed rustc compilation. Combines lexical search with compile-check verification.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Search query string (required)" },
                            "language": { "type": "string", "description": "Language filter (reserved for future use, defaults to 'rust')" },
                            "max_results": { "type": "integer", "description": "Maximum number of results to return (default: 20)" },
                            "verify": { "type": "boolean", "description": "Whether to verify Rust files via rustc compilation (default: true)" }
                        },
                        "required": ["query"]
                    }).as_object().cloned().unwrap()),
                ),
                // Modification tools (destructive)
                Tool::new(
                    "write_file",
                    "Create or overwrite files within the workspace.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Path to the file to write (required)" },
                            "content": { "type": "string", "description": "Content to write (required)" },
                            "create_dirs": { "type": "boolean", "description": "Whether to create parent directories if they don't exist (default: false)" }
                        },
                        "required": ["path", "content"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "edit_file",
                    "Edit files with syntax validation.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "path": { "type": "string", "description": "Path to the file to edit (required)" },
                            "edits": {
                                "type": "array",
                                "description": "Edits to apply (required)",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "old_string": { "type": "string", "description": "The exact text to replace (required)" },
                                        "new_string": { "type": "string", "description": "The replacement text (required)" }
                                    },
                                    "required": ["old_string", "new_string"]
                                }
                            }
                        },
                        "required": ["path", "edits"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "safe_refactor",
                    "Perform safe refactoring with validation and preview.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "action": { "type": "string", "enum": ["rename", "extract", "inline", "move", "change_signature"], "description": "Refactor action" },
                            "target": { "type": "string", "description": "Target symbol name" },
                            "params": { "type": "object", "description": "Action-specific parameters" }
                        },
                        "required": ["action", "target"]
                    }).as_object().cloned().unwrap()),
                ),
                // AIX-1: Smart Overview & Ranked Symbols
                Tool::new(
                    "smart_overview",
                    "Get a comprehensive project overview with architecture score, hot paths, and recommended first reads for AI agents.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "detail": { "type": "string", "enum": ["quick", "medium", "detailed"], "description": "Detail level: quick (~100 tokens), medium (~400 tokens), detailed (~800 tokens)" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "ranked_symbols",
                    "Get AI-relevance ranked symbols based on a search query, considering fan-in, complexity, and documentation.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Search query string" },
                            "limit": { "type": "integer", "description": "Maximum number of results to return (default: 50)" }
                        },
                        "required": ["query"]
                    }).as_object().cloned().unwrap()),
                ),
                // AIX-2: Onboarding Plan & Auto Diagnose & Refactor Plan
                Tool::new(
                    "suggest_onboarding_plan",
                    "Generate a step-by-step onboarding plan to understand, refactor, debug, or extend a codebase.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "goal": { "type": "string", "enum": ["understand", "refactor", "debug", "add_feature", "review"], "description": "Goal for the onboarding plan" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "auto_diagnose",
                    "Automatically diagnose project health issues including architecture problems, dead code, and complexity hotspots.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "target": { "type": "string", "description": "Optional target directory to diagnose" },
                            "min_severity": { "type": "string", "enum": ["info", "warning", "important", "critical"], "description": "Minimum severity level to report" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "suggest_refactor_plan",
                    "Analyze a symbol and suggest a concrete refactoring plan with risk assessment.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "symbol": { "type": "string", "description": "Target symbol to refactor" },
                            "goal": { "type": "string", "description": "Goal for refactoring (default: reduce_complexity)" },
                            "max_steps": { "type": "integer", "description": "Maximum number of steps in the plan" }
                        },
                        "required": ["symbol"]
                    }).as_object().cloned().unwrap()),
                ),
                // AIX-3: NL to Symbol & Ask About Code & Find Pattern
                Tool::new(
                    "nl_to_symbol",
                    "Convert natural language descriptions to precise symbol matches using keyword extraction and semantic search.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Natural language query" },
                            "limit": { "type": "integer", "description": "Maximum number of results (default: 20)" }
                        },
                        "required": ["query"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "ask_about_code",
                    "Answer questions about code flow by tracing execution paths between symbols.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "question": { "type": "string", "description": "Question about code flow" },
                            "limit": { "type": "integer", "description": "Maximum number of answers (default: 10)" }
                        },
                        "required": ["question"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "find_pattern_by_intent",
                    "Match natural language intent descriptions to known code patterns.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "intent": { "type": "string", "description": "Natural language intent description" },
                            "list_patterns": { "type": "boolean", "description": "Whether to list all available patterns" }
                        },
                        "required": ["intent"]
                    }).as_object().cloned().unwrap()),
                ),
                // AIX-4: Compare Call Graphs & Detect API Breaks
                Tool::new(
                    "compare_call_graphs",
                    "Compare the current call graph against a baseline to detect structural changes.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "baseline_dir": { "type": "string", "description": "Optional baseline directory to compare against" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "detect_api_breaks",
                    "Detect breaking changes in the public API by comparing entry points between current and baseline graphs.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "baseline_dir": { "type": "string", "description": "Optional baseline directory to compare against" },
                            "min_severity": { "type": "string", "enum": ["patch", "minor", "major"], "description": "Minimum severity to report" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "evaluate_refactor_quality",
                    "Evaluate whether a refactoring was beneficial by comparing current graph state vs persisted baseline.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {}
                    }).as_object().cloned().unwrap()),
                ),
                // AIX-5: System Prompt Context & God Functions & Long Params
                Tool::new(
                    "generate_system_prompt_context",
                    "Generate a structured context block for LLM system prompts in XML, JSON, or Markdown format.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "format": { "type": "string", "enum": ["xml", "json", "markdown"], "description": "Output format" },
                            "include_architecture": { "type": "boolean", "description": "Whether to include architecture info" },
                            "include_hot_paths": { "type": "boolean", "description": "Whether to include hot paths" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "detect_god_functions",
                    "Find overly large or complex functions (god functions) that should be refactored.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "min_lines": { "type": "integer", "description": "Minimum lines of code threshold (default: 50)" },
                            "min_complexity": { "type": "integer", "description": "Minimum cyclomatic complexity threshold (default: 15)" },
                            "min_fan_in": { "type": "integer", "description": "Minimum fan-in threshold (default: 5)" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "detect_long_parameter_lists",
                    "Find functions with too many parameters that should be consolidated into structs.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "max_params": { "type": "integer", "description": "Maximum number of parameters allowed (default: 5)" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                // PL3: Symbol Hotness Tracking
                Tool::new(
                    "get_hot_symbols",
                    "Get the most frequently accessed symbols (AI query hotness tracking).",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "limit": { "type": "integer", "description": "Maximum number of hot symbols to return (default: 20)" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                // AVC: Agent-Verifiable Context tools
                Tool::new(
                    "generate_contract",
                    "Generate an AVC truth contract from an existing function. Returns syntax, semantic, and safety constraints.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "function_name": { "type": "string", "description": "Name of the function to generate a contract for" },
                            "file_path": { "type": "string", "description": "Path to the source file containing the function" }
                        },
                        "required": ["function_name", "file_path"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "validate_contract",
                    "Validate generated code against an AVC truth contract. Returns pass/fail with violations and fix suggestions.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "contract_id": { "type": "string", "description": "ID of the contract to validate against" },
                            "generated_code": { "type": "string", "description": "The code to validate" }
                        },
                        "required": ["contract_id", "generated_code"]
                    }).as_object().cloned().unwrap()),
                ),
                // Phase 4b: Graph Analytics (PageRank, paths, condensation, god nodes, reduction, FAS)
                // These tools operate on the in-memory call graph that
                // `build_graph` populates, so they all require a prior
                // build. They are always available (not feature-gated)
                // because the underlying petgraph algorithms are pure.
                Tool::new(
                    "graph_pagerank",
                    "Compute PageRank importance scores for all symbols in the call graph. Returns a ranked list of symbols by dependency importance. High-scoring symbols are 'god nodes' (heavily depended-upon). Requires build_graph first.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "alpha": { "type": "number", "description": "Damping factor (default: 0.85). Must be in (0.0, 1.0]." },
                            "max_iterations": { "type": "integer", "description": "Max fixed-point iterations (default: 100)" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "graph_all_paths",
                    "Find all simple paths between two symbols in the call graph (no repeated nodes). Useful for enumerating every call chain that connects two functions. Requires build_graph first.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "from_symbol": { "type": "string", "description": "Source symbol name (substring match, case-insensitive)" },
                            "to_symbol": { "type": "string", "description": "Target symbol name (substring match, case-insensitive)" },
                            "max_hops": { "type": "integer", "description": "Maximum number of intermediate nodes (default: 5)" }
                        },
                        "required": ["from_symbol", "to_symbol"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "graph_condensed",
                    "Compute the SCC condensation of the call graph: every strongly connected component is collapsed into a single node, producing an acyclic condensation DAG. Use to spot circular dependency clusters. Requires build_graph first.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {}
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "graph_god_nodes",
                    "Find god nodes — symbols with unusually high PageRank (above the supplied percentile). These are symbols that too many things depend on and are prime refactoring candidates. Requires build_graph first.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "percentile": { "type": "number", "description": "Percentile threshold in [0.0, 1.0] (default: 0.95). Symbols at or above this PageRank percentile are returned." }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "graph_reduced",
                    "Compute the transitive reduction of the call graph — the minimal set of dependency edges that preserves reachability. Redundant edges (implied by longer paths) are dropped. Requires build_graph first.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {}
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "graph_feedback_arcs",
                    "Find a feedback arc set — edges whose removal would make the call graph acyclic. The greedy heuristic is not optimal but fast; use the result as a starting point when breaking circular dependencies. Requires build_graph first.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {}
                    }).as_object().cloned().unwrap()),
                ),
                // Phase 5: Community Detection (Label Propagation).
                //
                // `graph_communities` runs Label Propagation over the
                // in-memory call graph and returns deterministic
                // community labels. `graph_community_detail` drills
                // into a single community, and `graph_surprising_
                // connections` highlights edges that cross community
                // boundaries (often a sign of unwanted coupling).
                Tool::new(
                    "graph_communities",
                    "Detect code communities using Label Propagation. Groups symbols that are tightly coupled into clusters. Returns communities with cohesion scores. Requires build_graph first.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "max_iterations": { "type": "integer", "description": "Max label propagation iterations (default: 100)" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "graph_community_detail",
                    "Get details for a specific community detected by graph_communities (members, internal/external edge counts, cohesion score, and top god nodes within the community). Requires build_graph first.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "community_id": { "type": "integer", "description": "Sequential community id from graph_communities output" },
                            "max_iterations": { "type": "integer", "description": "Max label propagation iterations used to re-detect communities (default: 100)" }
                        },
                        "required": ["community_id"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "graph_surprising_connections",
                    "Find surprising cross-community connections. These are edges between symbols in different communities, indicating unexpected coupling. Requires build_graph first.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "top_n": { "type": "integer", "description": "Max connections to return (default: 20)" },
                            "max_iterations": { "type": "integer", "description": "Max label propagation iterations (default: 100)" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                // Phase 6: IDF-weighted Search & Unified Insights.
                //
                // `graph_search_idf` ranks symbols by an information-
                // retrieval-style score (rare tokens count more) and
                // includes a hub-bypass step that demotes the
                // 95th-percentile-degree nodes. The remaining two
                // tools, `graph_insights` and `graph_suggest_questions`,
                // consolidate god-nodes + cycles + communities +
                // cross-community edges + a 0-100 health score into a
                // single payload.
                Tool::new(
                    "graph_search_idf",
                    "Search symbols ranked by IDF (Inverse Document Frequency) importance. Rare terms score higher. Includes hub bypass for cleaner results. Requires build_graph first.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "query": { "type": "string", "description": "Search query (symbol name or partial)" },
                            "max_results": { "type": "integer", "description": "Max results (default: 20)" }
                        },
                        "required": ["query"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "graph_insights",
                    "Get a complete architecture health report: god nodes, circular dependencies, community overview, surprising cross-module connections, and a health score (0-100). Requires build_graph first.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {}
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "graph_suggest_questions",
                    "Generate intelligent questions about the codebase architecture based on graph analysis. Helps identify areas that need attention. Requires build_graph first.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {}
                    }).as_object().cloned().unwrap()),
                ),
                // Sprint 2: Graphify-style tools (ADR-026)
                Tool::new(
                    "graph_query",
                    "Natural language graph topology query. Ask 'what connects X to Y?' and get a subgraph with provenance. Requires build_graph first.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "question": { "type": "string", "description": "Natural language question about the graph topology" },
                            "max_depth": { "type": "integer", "description": "Maximum BFS depth from seed nodes (default: 3)" },
                            "budget": { "type": "integer", "description": "Maximum nodes to collect (default: 1500)" }
                        },
                        "required": ["question"]
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "graph_explain",
                    "Composite deep-dive on a symbol: callers, callees, fan-in/out, complexity. Saves multiple tool calls. Requires build_graph first.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "symbol": { "type": "string", "description": "Symbol name to explain" },
                            "depth": { "type": "integer", "description": "Neighbor depth (default: 2)" }
                        },
                        "required": ["symbol"]
                    }).as_object().cloned().unwrap()),
                ),
                // Phase 3A: Proactive Tools
                Tool::new(
                    "suggest_context",
                    "Zero-query proactive context suggestion. Returns ranked files/symbols relevant to an agent's current task without explicit search queries. Uses FTS5 search and call-graph hot-path analysis.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "limit": { "type": "integer", "description": "Maximum number of results to return (default: 10, max: 50)" },
                            "project_path": { "type": "string", "description": "Project path to search within (optional, defaults to workspace root)" }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                #[cfg(feature = "persistence")]
                Tool::new(
                    "reparse_on_edit",
                    "MCP-triggered incremental reindex of changed files. Accepts explicit file paths and optional edit ranges to inform the reindex scope.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "file_paths": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "File paths that were edited (required)"
                            },
                            "edit_ranges": {
                                "type": "array",
                                "description": "Optional edit ranges for more precise reindexing",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "file": { "type": "string" },
                                        "start_line": { "type": "integer" },
                                        "end_line": { "type": "integer" }
                                    }
                                }
                            }
                        },
                        "required": ["file_paths"]
                    }).as_object().cloned().unwrap()),
                ),
                // Detect Drift tool (S7000-S7003 intent drift detection)
                Tool::new(
                    "detect_drift",
                    "Analyze a source file for intent drift (S7000: docstring-body mismatch), AVC violations (S7001: unsafe/panic/unwrap), obsolete patterns (S7002: try! macro), and forbidden terms (S7003). Persists high-drift findings to the drift_events store.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "file_path": {
                                "type": "string",
                                "description": "Path to the source file to analyze (required)"
                            },
                            "threshold": {
                                "type": "number",
                                "description": "Minimum drift score threshold (default: 0.5). Only findings with drift_score >= threshold are included."
                            },
                            "function_name": {
                                "type": "string",
                                "description": "Optional function name to scope analysis to a single function"
                            }
                        },
                        "required": ["file_path"]
                    }).as_object().cloned().unwrap()),
                ),
                // Batch D: Agent Task Tools (bidirectional interaction)
                Tool::new(
                    "poll_tasks",
                    "Poll for pending agent tasks and claim them for execution. Returns up to `limit` tasks with status changed to 'in_progress'.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "limit": {
                                "type": "integer",
                                "description": "Maximum number of tasks to claim (default: 10, max: 100)"
                            }
                        }
                    }).as_object().cloned().unwrap()),
                ),
                Tool::new(
                    "complete_task",
                    "Mark an agent task as completed or failed with optional result data.",
                    Arc::new(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "task_id": {
                                "type": "integer",
                                "description": "Task ID to complete (required)"
                            },
                            "status": {
                                "type": "string",
                                "description": "Completion status: 'completed' or 'failed' (required)"
                            },
                            "result_json": {
                                "type": "string",
                                "description": "Optional JSON result data"
                            },
                            "error_message": {
                                "type": "string",
                                "description": "Optional error message (for failed tasks)"
                            }
                        },
                         "required": ["task_id", "status"]
                     }).as_object().cloned().unwrap()),
                 ),
            ];

            // Paginate
            let total = all_tools.len();
            let page_tools: Vec<_> = all_tools
                .into_iter()
                .skip(cursor_offset)
                .take(PAGE_SIZE)
                .collect();

            let next_cursor = if cursor_offset + PAGE_SIZE < total {
                Some(
                    base64::engine::general_purpose::STANDARD
                        .encode((cursor_offset + PAGE_SIZE).to_string()),
                )
            } else {
                None
            };

            Ok(ListToolsResult {
                meta: None,
                tools: page_tools,
                next_cursor,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, rmcp::ErrorData>> + Send + '_
    {
        let ctx = self.ctx.clone();
        async move {
            let result = call_tool_handler(&ctx, request).await;

            match result {
                Ok(output) => Ok(CallToolResult::success(vec![Content::text(output)])),
                Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
            }
        }
    }

    fn on_cancelled(
        &self,
        _notification: rmcp::model::CancelledNotificationParam,
        _context: rmcp::service::NotificationContext<RoleServer>,
    ) -> impl std::future::Future<Output = ()> + Send + '_ {
        self.cancellation_token.store(true, Ordering::SeqCst);
        std::future::ready(())
    }
}

/// Handles the tool call by dispatching to the appropriate handler
async fn call_tool_handler(
    ctx: &HandlerContext,
    request: CallToolRequestParams,
) -> InterfaceResult<String> {
    let tool_name = request.name.as_ref();
    let arguments = request.arguments.unwrap_or_default();

    match tool_name {
        "get_file_symbols" => {
            let input: crate::interface::mcp::schemas::GetFileSymbolsInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::handle_get_file_symbols(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "build_graph" => {
            let input: crate::interface::mcp::handlers::BuildGraphInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_build_graph(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "get_call_hierarchy" => {
            let input: crate::interface::mcp::schemas::GetCallHierarchyInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::handle_get_call_hierarchy(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "analyze_impact" => {
            let input: crate::interface::mcp::schemas::AnalyzeImpactInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_analyze_impact(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "check_architecture" => {
            let input: crate::interface::mcp::schemas::CheckArchitectureInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::handle_check_architecture(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "safe_refactor" => {
            let input: crate::interface::mcp::schemas::SafeRefactorInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::refactor_handlers::handle_safe_refactor(
                ctx, input,
            )
            .await?;
            Ok(serde_json::to_string(&output)?)
        }
        "find_usages" => {
            let input: crate::interface::mcp::schemas::FindUsagesInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_find_usages(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "get_complexity" => {
            let input: crate::interface::mcp::schemas::GetComplexityInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_get_complexity(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "get_entry_points" => {
            let input: crate::interface::mcp::schemas::GetEntryPointsInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::handle_get_entry_points(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "get_leaf_functions" => {
            let input: crate::interface::mcp::schemas::GetLeafFunctionsInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::handle_get_leaf_functions(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "trace_path" => {
            let input: crate::interface::mcp::schemas::TracePathInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_trace_path(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "export_mermaid" => {
            let input: crate::interface::mcp::schemas::ExportMermaidInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_export_mermaid(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "get_hot_paths" => {
            let input: crate::interface::mcp::schemas::GetHotPathsInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_get_hot_paths(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "get_all_symbols" => {
            let input: crate::interface::mcp::schemas::GetAllSymbolsInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::handle_get_all_symbols(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "find_dead_code" => {
            let input: crate::interface::mcp::schemas::FindDeadCodeInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_find_dead_code(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "get_module_dependencies" => {
            let input: crate::interface::mcp::schemas::GetModuleDependenciesInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::handle_get_module_dependencies(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "build_lightweight_index" => {
            let input: crate::interface::mcp::schemas::BuildIndexInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::handle_build_lightweight_index(ctx, input)
                    .await?;
            Ok(serde_json::to_string(&output)?)
        }
        "query_symbol_index" => {
            let input: crate::interface::mcp::schemas::QuerySymbolInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::handle_query_symbol_index(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "build_call_subgraph" => {
            let input: crate::interface::mcp::schemas::BuildSubgraphInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::handle_build_call_subgraph(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "get_per_file_graph" => {
            let input: crate::interface::mcp::schemas::GetPerFileGraphInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::handle_get_per_file_graph(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "merge_file_graphs" => {
            let input: crate::interface::mcp::schemas::MergeGraphsInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_merge_graphs(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "get_outline" => {
            let input: crate::interface::mcp::schemas::OutlineInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_get_outline(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "get_symbol_code" => {
            let input: crate::interface::mcp::schemas::SymbolCodeInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_get_symbol_code(
                ctx.symbol_code.clone(),
                ctx.validator.clone(),
                ctx.working_dir.clone(),
                input,
            )
            .await?;
            Ok(serde_json::to_string(&output)?)
        }
        "semantic_search" => {
            let input: crate::interface::mcp::schemas::SemanticSearchInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_semantic_search(
                ctx.semantic_search.clone(),
                ctx.working_dir.clone(),
                input,
            )
            .await?;
            Ok(serde_json::to_string(&output)?)
        }
        "find_usages_with_context" => {
            let input: crate::interface::mcp::schemas::FindUsagesWithContextInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_find_usages_with_context(
                ctx.validator.clone(),
                ctx.working_dir.clone(),
                input,
            )
            .await?;
            Ok(serde_json::to_string(&output)?)
        }
        "go_to_definition" => {
            let input: crate::interface::mcp::schemas::GoToDefinitionInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::lsp_handlers::handle_go_to_definition(ctx, input)
                    .await?;
            Ok(serde_json::to_string(&output)?)
        }
        "hover" => {
            let input: crate::interface::mcp::schemas::HoverInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::lsp_handlers::handle_hover(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "find_references" => {
            let input: crate::interface::mcp::schemas::FindReferencesInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::lsp_handlers::handle_find_references(ctx, input)
                    .await?;
            Ok(serde_json::to_string(&output)?)
        }
        "read_file" => {
            let input: crate::interface::mcp::schemas::ReadFileInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_read_file(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "write_file" => {
            let input: crate::interface::mcp::schemas::WriteFileInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_write_file(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "edit_file" => {
            let input: crate::interface::mcp::schemas::EditFileInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_edit_file(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "search_content" => {
            let input: crate::interface::mcp::schemas::SearchContentInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_search_content(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "list_files" => {
            let input: crate::interface::mcp::schemas::ListFilesInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_list_files(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "retrieve_and_verify" => {
            let input: crate::interface::mcp::schemas::RetrieveAndVerifyInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::handle_retrieve_and_verify(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        // AIX-1: Smart Overview & Ranked Symbols
        "smart_overview" => {
            let input: crate::interface::mcp::schemas::SmartOverviewInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_smart_overview(ctx, input)
                    .await?;
            Ok(serde_json::to_string(&output)?)
        }
        "ranked_symbols" => {
            let input: crate::interface::mcp::schemas::RankedSymbolsInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_ranked_symbols(ctx, input)
                    .await?;
            Ok(serde_json::to_string(&output)?)
        }
        // AIX-2: Onboarding Plan & Auto Diagnose & Refactor Plan
        "suggest_onboarding_plan" => {
            let input: crate::interface::mcp::schemas::OnboardingPlanInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_suggest_onboarding_plan(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string(&output)?)
        }
        "auto_diagnose" => {
            let input: crate::interface::mcp::schemas::AutoDiagnoseInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_auto_diagnose(ctx, input)
                    .await?;
            Ok(serde_json::to_string(&output)?)
        }
        "suggest_refactor_plan" => {
            let input: crate::interface::mcp::schemas::SuggestRefactorPlanInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_suggest_refactor_plan(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string(&output)?)
        }
        // AIX-3: NL to Symbol & Ask About Code & Find Pattern
        "nl_to_symbol" => {
            let input: crate::interface::mcp::schemas::NlToSymbolInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_nl_to_symbol(ctx, input)
                    .await?;
            Ok(serde_json::to_string(&output)?)
        }
        "ask_about_code" => {
            let input: crate::interface::mcp::schemas::AskAboutCodeInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_ask_about_code(ctx, input)
                    .await?;
            Ok(serde_json::to_string(&output)?)
        }
        "find_pattern_by_intent" => {
            let input: crate::interface::mcp::schemas::FindPatternByIntentInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_find_pattern_by_intent(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string(&output)?)
        }
        // AIX-4: Compare Call Graphs & Detect API Breaks
        "compare_call_graphs" => {
            let input: crate::interface::mcp::schemas::CompareCallGraphsInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::aix_handlers::handle_compare_call_graphs(
                ctx, input,
            )
            .await?;
            Ok(serde_json::to_string(&output)?)
        }
        "detect_api_breaks" => {
            let input: crate::interface::mcp::schemas::DetectApiBreaksInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_detect_api_breaks(ctx, input)
                    .await?;
            Ok(serde_json::to_string(&output)?)
        }
        "evaluate_refactor_quality" => {
            let input: crate::interface::mcp::schemas::EvaluateRefactorQualityInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_evaluate_refactor_quality(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string(&output)?)
        }
        // AIX-5: System Prompt Context & God Functions & Long Params
        "generate_system_prompt_context" => {
            let input: crate::interface::mcp::schemas::SystemPromptContextInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::aix_handlers::handle_generate_system_prompt_context(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "detect_god_functions" => {
            let input: crate::interface::mcp::schemas::DetectGodFunctionsInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_detect_god_functions(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string(&output)?)
        }
        "detect_long_parameter_lists" => {
            let input: crate::interface::mcp::schemas::DetectLongParamsInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_detect_long_parameter_lists(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string(&output)?)
        }
        // PL3: Symbol Hotness Tracking
        "get_hot_symbols" => {
            let input: crate::interface::mcp::schemas::GetHotSymbolsInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::handle_get_hot_symbols(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        // AVC: Agent-Verifiable Context tools
        "generate_contract" => {
            let input: crate::interface::mcp::schemas::GenerateContractInput =
                serde_json::from_value(arguments.into())?;
            let start = std::time::Instant::now();
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_generate_contract(ctx, input)
                    .await?;
            let duration_ms = start.elapsed().as_millis() as f64;
            // Best-effort telemetry recording
            ctx.record_tool_usage(
                "generate_contract",
                &serde_json::to_string(&output).unwrap_or_default(),
                duration_ms,
                Some(&output.contract_id),
            );
            Ok(serde_json::to_string(&output)?)
        }
        "validate_contract" => {
            let input: crate::interface::mcp::schemas::ValidateContractInput =
                serde_json::from_value(arguments.into())?;
            let contract_id = input.contract_id.clone();
            let start = std::time::Instant::now();
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_validate_contract(ctx, input)
                    .await?;
            let duration_ms = start.elapsed().as_millis() as f64;
            // Best-effort telemetry recording
            ctx.record_tool_usage(
                "validate_contract",
                &serde_json::to_string(&output).unwrap_or_default(),
                duration_ms,
                Some(&contract_id),
            );
            Ok(serde_json::to_string(&output)?)
        }
        // Phase 3A: Proactive Tools
        "suggest_context" => {
            let input: crate::interface::mcp::schemas::SuggestContextInput =
                serde_json::from_value(arguments.into())?;
            let start = std::time::Instant::now();
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_suggest_context(ctx, input)
                    .await?;
            let duration_ms = start.elapsed().as_millis() as f64;
            // Best-effort telemetry recording
            ctx.record_tool_usage(
                "suggest_context",
                &serde_json::to_string(&output).unwrap_or_default(),
                duration_ms,
                None,
            );
            Ok(serde_json::to_string(&output)?)
        }
        #[cfg(feature = "persistence")]
        "reparse_on_edit" => {
            let input: crate::interface::mcp::schemas::ReparseOnEditInput =
                serde_json::from_value(arguments.into())?;
            let start = std::time::Instant::now();
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_reparse_on_edit(ctx, input)
                    .await?;
            let duration_ms = start.elapsed().as_millis() as f64;
            // Best-effort telemetry recording
            ctx.record_tool_usage(
                "reparse_on_edit",
                &serde_json::to_string(&output).unwrap_or_default(),
                duration_ms,
                None,
            );
            Ok(serde_json::to_string(&output)?)
        }
        // Detect Drift tool (S7000-S7003)
        "detect_drift" => {
            let input: crate::interface::mcp::schemas::DetectDriftInput =
                serde_json::from_value(arguments.into())?;
            let start = std::time::Instant::now();
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_detect_drift(ctx, input)
                    .await?;
            let duration_ms = start.elapsed().as_millis() as f64;
            // Best-effort telemetry recording
            ctx.record_tool_usage(
                "detect_drift",
                &serde_json::to_string(&output).unwrap_or_default(),
                duration_ms,
                None,
            );
            Ok(serde_json::to_string(&output)?)
        }
        // Batch D: Agent Task Tools (bidirectional interaction)
        "poll_tasks" => {
            let input: crate::interface::mcp::schemas::PollTasksInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_poll_tasks(ctx, input)
                    .await?;
            Ok(serde_json::to_string(&output)?)
        }
        "complete_task" => {
            let input: crate::interface::mcp::schemas::CompleteTaskInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_complete_task(ctx, input)
                    .await?;
            Ok(serde_json::to_string(&output)?)
        }
        // Phase 4b: Graph analytics tools (extracted to graph_handlers.rs)
        "graph_pagerank" => {
            let input: crate::interface::mcp::schemas::GraphPageRankInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_handlers::handle_graph_pagerank(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_all_paths" => {
            let input: crate::interface::mcp::schemas::GraphAllPathsInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_handlers::handle_graph_all_paths(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_condensed" => {
            let input: crate::interface::mcp::schemas::GraphCondensedInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_handlers::handle_graph_condensed(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_god_nodes" => {
            let input: crate::interface::mcp::schemas::GraphGodNodesInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_handlers::handle_graph_god_nodes(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_reduced" => {
            let input: crate::interface::mcp::schemas::GraphReducedInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_handlers::handle_graph_reduced(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_feedback_arcs" => {
            let input: crate::interface::mcp::schemas::GraphFeedbackArcsInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_handlers::handle_graph_feedback_arcs(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        // Phase 5: Community Detection handlers (extracted to graph_handlers.rs)
        "graph_communities" => {
            let input: crate::interface::mcp::schemas::GraphCommunitiesInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_handlers::handle_graph_communities(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_community_detail" => {
            let input: crate::interface::mcp::schemas::GraphCommunityDetailInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_handlers::handle_graph_community_detail(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_surprising_connections" => {
            let input: crate::interface::mcp::schemas::GraphSurprisingConnectionsInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_handlers::handle_graph_surprising_connections(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        // Phase 6: IDF-weighted Search & Unified Insights (extracted to graph_handlers.rs)
        "graph_search_idf" => {
            let input: crate::interface::mcp::schemas::GraphSearchIdfInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_handlers::handle_graph_search_idf(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_insights" => {
            let input: crate::interface::mcp::schemas::GraphInsightsInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_handlers::handle_graph_insights(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_suggest_questions" => {
            let input: crate::interface::mcp::schemas::GraphSuggestQuestionsInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_handlers::handle_graph_suggest_questions(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        // Sprint 2: Graphify-style tools (ADR-026)
        "graph_query" => {
            let input: crate::interface::mcp::handlers::graph_query_handlers::GraphQueryInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_query_handlers::handle_graph_query(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_explain" => {
            let input: crate::interface::mcp::handlers::graph_query_handlers::GraphExplainInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_query_handlers::handle_graph_explain(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        // Edge-type query tools (ADR-026)
        "get_type_references" => {
            let input: crate::interface::mcp::handlers::graph_query_handlers::GetTypeRefsInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_query_handlers::handle_get_type_references(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "get_imports" => {
            let input: crate::interface::mcp::handlers::graph_query_handlers::GetImportsInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_query_handlers::handle_get_imports(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "get_implementors" => {
            let input: crate::interface::mcp::handlers::graph_query_handlers::GetImplementorsInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_query_handlers::handle_get_implementors(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "get_members" => {
            let input: crate::interface::mcp::handlers::graph_query_handlers::GetMembersInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_query_handlers::handle_get_members(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_query_filtered" => {
            let input: crate::interface::mcp::handlers::graph_query_handlers::GraphQueryFilteredInput = serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_query_handlers::handle_graph_query_filtered(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "export_callflow" => {
            let input: crate::interface::mcp::handlers::graph_query_handlers::ExportCallflowInput = serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_query_handlers::handle_export_callflow(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        _ => return Err(InterfaceError::ToolNotFound(tool_name.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    use std::sync::Arc;
    
    

    // ============================================================================
    // Concurrent Request Tests
    // ============================================================================
    // NOTE: Tests using RequestContext::default() are marked as #[ignore]
    // because rmcp's RequestContext requires internal APIs (Peer::new is pub(crate))
    // to construct a valid context. These tests need to be moved to an integration
    // test within the rmcp crate or rewritten to use a test helper from rmcp.

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "requires rmcp internals to create RequestContext"]
    async fn test_concurrent_list_tools_requests() {
        // TODO: Rewrite using proper rmcp context creation when test utilities are available
        unimplemented!("requires rmcp::service::Peer::new (pub(crate)) to create RequestContext")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_concurrent_handler_creation() {
        // Test that multiple handlers can be created concurrently without issues
        let handlers: Vec<CogniCodeHandler> = (0..10)
            .map(|i| CogniCodeHandler::new(PathBuf::from(&format!("/tmp/test_{}", i))))
            .collect();

        // All handlers should have valid state
        for handler in handlers {
            assert!(handler.ctx.working_dir.to_string_lossy().contains("test"));
            let info = handler.get_info();
            assert_eq!(info.server_info.name, "cognicode");
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_concurrent_get_info_calls() {
        let handler = Arc::new(CogniCodeHandler::new(PathBuf::from("/tmp/test")));

        let mut handles = vec![];
        for _ in 0..100 {
            let handler = handler.clone();
            handles.push(tokio::spawn(async move { handler.get_info() }));
        }

        let results = futures_util::future::join_all(handles).await;

        // All calls should return consistent info
        for result in results {
            let info = result.unwrap();
            assert_eq!(info.server_info.name, "cognicode");
            assert!(info.capabilities.tools.is_some());
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "requires rmcp internals to create RequestContext"]
    async fn test_concurrent_shared_handler() {
        // TODO: Rewrite using proper rmcp context creation when test utilities are available
        unimplemented!("requires rmcp::service::Peer::new (pub(crate)) to create RequestContext")
    }

    // ============================================================================
    // Request Cancellation Tests
    // ============================================================================

    #[tokio::test]
    #[ignore = "requires rmcp internals to create NotificationContext"]
    async fn test_cancellation_token_set() {
        unimplemented!("requires rmcp::service::NotificationContext::default() which doesn't exist")
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "requires rmcp internals to create NotificationContext"]
    async fn test_concurrent_cancellation_notifications() {
        unimplemented!("requires rmcp::service::NotificationContext::default() which doesn't exist")
    }

    #[tokio::test]
    #[ignore = "requires rmcp internals to create NotificationContext"]
    async fn test_cancellation_token_reset_on_new_handler() {
        unimplemented!("requires rmcp::service::NotificationContext::default() which doesn't exist")
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "requires rmcp internals to create NotificationContext"]
    async fn test_multiple_cancellation_tokens_independent() {
        unimplemented!("requires rmcp::service::NotificationContext::default() which doesn't exist")
    }

    // ============================================================================
    // Adapter State Management Tests
    // ============================================================================

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "requires rmcp internals to create RequestContext"]
    async fn test_handler_state_preserved_across_concurrent_requests() {
        unimplemented!("requires rmcp::service::Peer::new (pub(crate)) to create RequestContext")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_capability_exchange_concurrent() {
        let handler = Arc::new(CogniCodeHandler::new(PathBuf::from("/tmp/test")));

        let mut handles = vec![];
        for _ in 0..20 {
            let handler = handler.clone();
            handles.push(tokio::spawn(async move {
                let info = handler.get_info();
                // Verify capabilities are consistent
                (info.server_info.name.clone(), info.capabilities.clone())
            }));
        }

        let results = futures_util::future::join_all(handles).await;

        // All should return same consistent capabilities
        let first = results.first().unwrap().as_ref().unwrap();
        let first_tools = first.1.tools.clone();
        for result in results {
            let (name, caps) = result.unwrap();
            assert_eq!(name, "cognicode");
            assert_eq!(caps.tools, first_tools);
        }
    }

    #[tokio::test]
    async fn test_version_negotiation_returns_correct_version() {
        let handler = CogniCodeHandler::new(PathBuf::from("/tmp/test"));
        let info = handler.get_info();

        // Verify protocol version is set correctly
        assert_eq!(
            info.protocol_version,
            rmcp::model::ProtocolVersion::V_2025_03_26
        );
    }

    // ============================================================================
    // Error Handling Under Load Tests
    // ============================================================================

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "requires rmcp internals to create RequestContext"]
    async fn test_pagination_consistent_under_concurrent_access() {
        // TODO: Rewrite using proper rmcp context creation when test utilities are available
        unimplemented!("requires rmcp::service::Peer::new (pub(crate)) to create RequestContext")
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "requires rmcp internals to create RequestContext"]
    async fn test_high_concurrency_stress() {
        // TODO: Rewrite using proper rmcp context creation when test utilities are available
        unimplemented!("requires rmcp::service::Peer::new (pub(crate)) to create RequestContext")
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "requires rmcp internals to create RequestContext"]
    async fn test_concurrent_requests_with_different_pagination() {
        // TODO: Rewrite using proper rmcp context creation when test utilities are available
        unimplemented!("requires rmcp::service::Peer::new (pub(crate)) to create RequestContext")
    }

    // ============================================================================
    // Original Basic Tests (preserved)
    // ============================================================================

    #[test]
    fn test_cognicode_handler_creation() {
        let handler = CogniCodeHandler::new(PathBuf::from("/tmp/test"));
        // working_dir is canonicalized so may differ from input path
        assert!(handler.ctx.working_dir.to_string_lossy().ends_with("test"));
    }

    #[test]
    fn test_server_info() {
        let handler = CogniCodeHandler::new(PathBuf::from("/tmp/test"));
        let info = handler.get_info();
        assert_eq!(info.server_info.name, "cognicode");
        assert!(info.capabilities.tools.is_some());
    }
}
