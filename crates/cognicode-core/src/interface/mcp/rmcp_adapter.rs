//! RMCP Adapter - Bridge between rmcp SDK and CogniCode handlers
//!
//! This module provides the CogniCodeHandler which implements the rmcp ServerHandler trait,
//! allowing the CogniCode MCP server to use the official rmcp SDK for transport.

use crate::interface::mcp::handlers::HandlerContext;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content,
    ListToolsResult, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RoleServer;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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
    /// Creates a new CogniCodeHandler
    pub fn new(project_root: PathBuf) -> Self {
        // Canonicalize to absolute path to avoid issues with relative paths
        // containing "./" or other components that cause validation mismatches
        let canonical_root = std::fs::canonicalize(&project_root)
            .unwrap_or_else(|_| {
                // If canonicalize fails (e.g., path doesn't exist yet), use the original
                project_root.clone()
            });
        let cancellation_token = Arc::new(AtomicBool::new(false));
        let mut ctx = HandlerContext::new(canonical_root);
        ctx.cancellation_token = cancellation_token.clone();
        Self {
            ctx: Arc::new(ctx),
            cancellation_token,
        }
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
        .with_server_info(rmcp::model::Implementation::new("cognicode", env!("CARGO_PKG_VERSION")))
        .with_protocol_version(rmcp::model::ProtocolVersion::V_2025_03_26)
    }

    fn list_tools(
        &self,
        request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, rmcp::ErrorData>> + Send + '_ {
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
            ];

            // Paginate
            let total = all_tools.len();
            let page_tools: Vec<_> = all_tools
                .into_iter()
                .skip(cursor_offset)
                .take(PAGE_SIZE)
                .collect();

            let next_cursor = if cursor_offset + PAGE_SIZE < total {
                Some(base64::engine::general_purpose::STANDARD.encode(
                    (cursor_offset + PAGE_SIZE).to_string(),
                ))
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
    ) -> impl std::future::Future<Output = Result<CallToolResult, rmcp::ErrorData>> + Send + '_ {
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
) -> anyhow::Result<String> {
    let tool_name = request.name.as_ref();
    let arguments = request.arguments.unwrap_or_default();

    match tool_name {
        "get_file_symbols" => {
            let input: crate::interface::mcp::schemas::GetFileSymbolsInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_get_file_symbols(ctx, input).await?;
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
            let output = crate::interface::mcp::handlers::refactor_handlers::handle_safe_refactor(ctx, input).await?;
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
            let output = crate::interface::mcp::handlers::handle_get_all_symbols(ctx, input).await?;
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
            let output = crate::interface::mcp::handlers::handle_get_module_dependencies(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "build_lightweight_index" => {
            let input: crate::interface::mcp::schemas::BuildIndexInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::handle_build_lightweight_index(ctx, input).await?;
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
            let output = crate::interface::mcp::handlers::handle_get_symbol_code(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "semantic_search" => {
            let input: crate::interface::mcp::schemas::SemanticSearchInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::handle_semantic_search(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "find_usages_with_context" => {
            let input: crate::interface::mcp::schemas::FindUsagesWithContextInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::handle_find_usages_with_context(ctx, input)
                    .await?;
            Ok(serde_json::to_string(&output)?)
        }
        "go_to_definition" => {
            let input: crate::interface::mcp::schemas::GoToDefinitionInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::lsp_handlers::handle_go_to_definition(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "hover" => {
            let input: crate::interface::mcp::schemas::HoverInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::lsp_handlers::handle_hover(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "find_references" => {
            let input: crate::interface::mcp::schemas::FindReferencesInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::lsp_handlers::handle_find_references(ctx, input).await?;
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
            let output =
                crate::interface::mcp::handlers::handle_search_content(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "list_files" => {
            let input: crate::interface::mcp::schemas::ListFilesInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_list_files(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        // AIX-1: Smart Overview & Ranked Symbols
        "smart_overview" => {
            let input: crate::interface::mcp::schemas::SmartOverviewInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::aix_handlers::handle_smart_overview(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "ranked_symbols" => {
            let input: crate::interface::mcp::schemas::RankedSymbolsInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::aix_handlers::handle_ranked_symbols(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        // AIX-2: Onboarding Plan & Auto Diagnose & Refactor Plan
        "suggest_onboarding_plan" => {
            let input: crate::interface::mcp::schemas::OnboardingPlanInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::aix_handlers::handle_suggest_onboarding_plan(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "auto_diagnose" => {
            let input: crate::interface::mcp::schemas::AutoDiagnoseInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::aix_handlers::handle_auto_diagnose(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "suggest_refactor_plan" => {
            let input: crate::interface::mcp::schemas::SuggestRefactorPlanInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::aix_handlers::handle_suggest_refactor_plan(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        // AIX-3: NL to Symbol & Ask About Code & Find Pattern
        "nl_to_symbol" => {
            let input: crate::interface::mcp::schemas::NlToSymbolInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::aix_handlers::handle_nl_to_symbol(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "ask_about_code" => {
            let input: crate::interface::mcp::schemas::AskAboutCodeInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::aix_handlers::handle_ask_about_code(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "find_pattern_by_intent" => {
            let input: crate::interface::mcp::schemas::FindPatternByIntentInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::aix_handlers::handle_find_pattern_by_intent(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        // AIX-4: Compare Call Graphs & Detect API Breaks
        "compare_call_graphs" => {
            let input: crate::interface::mcp::schemas::CompareCallGraphsInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::aix_handlers::handle_compare_call_graphs(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "detect_api_breaks" => {
            let input: crate::interface::mcp::schemas::DetectApiBreaksInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::aix_handlers::handle_detect_api_breaks(ctx, input).await?;
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
            let output = crate::interface::mcp::handlers::aix_handlers::handle_detect_god_functions(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        "detect_long_parameter_lists" => {
            let input: crate::interface::mcp::schemas::DetectLongParamsInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::aix_handlers::handle_detect_long_parameter_lists(ctx, input).await?;
            Ok(serde_json::to_string(&output)?)
        }
        _ => anyhow::bail!("Unknown tool: {}", tool_name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
