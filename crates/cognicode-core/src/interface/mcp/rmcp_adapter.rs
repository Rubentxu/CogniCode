//! RMCP Adapter - Bridge between rmcp SDK and CogniCode handlers
//!
//! This module provides the CogniCodeHandler which implements the rmcp ServerHandler trait,
//! allowing the CogniCode MCP server to use the official rmcp SDK for transport.

use crate::application::services::file_operations::FileOperationsService;
use crate::infrastructure::telemetry::get_global_metrics;
use crate::infrastructure::verification::RustVerifier;
use crate::interface::mcp::error::{InterfaceError, InterfaceResult};
use crate::interface::mcp::handlers::HandlerContext;
use opentelemetry::KeyValue;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ListToolsResult, Meta, ServerCapabilities,
    ServerInfo, Tool,
};
use rmcp::service::RoleServer;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use regex::Regex;

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

    /// M3.1: Creates a CogniCodeHandler wrapping a pre-built, shared
    /// `Arc<HandlerContext>`. Used by the HTTP server (cognicode-mcp)
    /// to share the same `graph_loaded` flag between the MCP dispatch
    /// and the `/ready` HTTP handler.
    pub fn from_ctx(ctx: Arc<HandlerContext>) -> Self {
        let cancellation_token = ctx.cancellation_token.clone();
        Self {
            ctx,
            cancellation_token,
        }
    }

    /// Creates a new CogniCodeHandler with a custom GraphStore (SQLite for persistence)
    pub fn with_graph_store(
        project_root: PathBuf,
        store: Arc<dyn crate::domain::traits::GraphStore>,
    ) -> Self {
        let cancellation_token = Arc::new(AtomicBool::new(false));
        let mut ctx = HandlerContext::builder()
            .with_working_dir(project_root)
            .with_graph_store_arc(store)
            .build();
        ctx.cancellation_token = cancellation_token.clone();
        Self {
            ctx: Arc::new(ctx),
            cancellation_token,
        }
    }

    /// M3.1: Creates a CogniCodeHandler with an IacRepository wired from pg_repo.
    ///
    /// When `iac_repo` is `Some`, the handler will use it for `iac_query`
    /// instead of falling back to the in-memory graph. This is the preferred
    /// constructor when running with PostgreSQL persistence.
    #[cfg(feature = "postgres")]
    pub fn with_iac_repo(
        project_root: PathBuf,
        store: Arc<dyn crate::domain::traits::GraphStore>,
        iac_repo: Arc<dyn crate::domain::traits::iac_repository::IacRepository>,
    ) -> Self {
        let cancellation_token = Arc::new(AtomicBool::new(false));
        let mut ctx = HandlerContext::builder()
            .with_working_dir(project_root)
            .with_graph_store_arc(store)
            .with_iac_repo(iac_repo)
            .build();
        ctx.cancellation_token = cancellation_token.clone();
        Self {
            ctx: Arc::new(ctx),
            cancellation_token,
        }
    }

    /// Fallback for no-cfg builds
    #[cfg(not(feature = "postgres"))]
    pub fn with_iac_repo(
        project_root: PathBuf,
        store: Arc<dyn crate::domain::traits::GraphStore>,
        _iac_repo: Option<Arc<dyn crate::domain::traits::iac_repository::IacRepository>>,
    ) -> Self {
        let cancellation_token = Arc::new(AtomicBool::new(false));
        let mut ctx = HandlerContext::builder()
            .with_working_dir(project_root)
            .with_graph_store_arc(store)
            .build();
        ctx.cancellation_token = cancellation_token.clone();
        Self {
            ctx: Arc::new(ctx),
            cancellation_token,
        }
    }

    /// Mode B: Creates a CogniCodeHandler with PostgreSQL-backed repositories.
    ///
    /// Wires both `postgres_repo` (for graph_diff, graph_timeline) and `iac_repo`
    /// (for iac_query) from a single PG connection pool. This is the preferred
    /// constructor when `--postgres <URL>` is provided to `cognicode-mcp`.
    ///
    /// Returns an error if the PostgreSQL connection fails.
    #[cfg(feature = "postgres")]
    pub async fn with_pg(project_root: PathBuf, pg_url: &str) -> Result<Self, String> {
        use crate::infrastructure::persistence::{PostgresIacRepository, PostgresRepository};
        use sqlx::PgPool;

        // Connect to PostgreSQL
        let pool = PgPool::connect(pg_url)
            .await
            .map_err(|e| format!("Failed to connect to PostgreSQL: {}", e))?;

        // Wrap in PostgresRepository (provides run_migrations if needed)
        let pg_repo = Arc::new(
            PostgresRepository::from_pool(pool.clone())
        );

        // Create IacRepository from the same pool
        let iac_repo: Arc<dyn crate::domain::traits::iac_repository::IacRepository> =
            Arc::new(PostgresIacRepository::new(pool));

        // Build HandlerContext with PG-backed repos (mirrors build_ctx structure)
        let cancellation_token = Arc::new(AtomicBool::new(false));
        let canonical_root =
            std::fs::canonicalize(&project_root).unwrap_or_else(|_| project_root.clone());

        let validator = Arc::new(
            crate::interface::mcp::security::InputValidator::new()
                .with_workspace(vec![canonical_root.clone()]),
        );
        let file_ops_service = Arc::new(FileOperationsService::new(
            canonical_root.to_string_lossy().as_ref(),
            validator,
            Arc::new(RustVerifier::new()),
        ));

        let mut ctx = HandlerContext::builder()
            .with_working_dir(canonical_root)
            .with_file_ops_service(file_ops_service)
            .with_postgres_repo(pg_repo)
            .with_iac_repo(iac_repo)
            .build();
        ctx.cancellation_token = cancellation_token.clone();

        Ok(Self {
            ctx: Arc::new(ctx),
            cancellation_token,
        })
    }

    /// Fallback `with_pg` for builds without the `postgres` feature.
    /// Mirrors the pattern used by `with_iac_repo`: callers can reference
    /// `CogniCodeHandler::with_pg(...)` unconditionally, but the build
    /// without `postgres` will short-circuit with a clear error message
    /// at runtime instead of failing at link time.
    #[cfg(not(feature = "postgres"))]
    pub async fn with_pg(_project_root: PathBuf, _pg_url: &str) -> Result<Self, String> {
        Err("with_pg requires the `postgres` feature on cognicode-core".to_string())
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

/// Helper to build cognicode metadata annotation for a tool.
/// Returns a Meta object with cognicode-specific fields:
/// - stability: "gated" | "stable" | "experimental"
/// - category: "graph" | "composite" | "navigation" | "file_ops" | etc.
/// - requires_graph: whether the tool needs a built call graph
/// - requires_persistence: whether the tool needs PG-backed persistence
/// - estimated_latency_ms: expected execution time in milliseconds
fn cognicode_meta(
    stability: &str,
    category: &str,
    requires_graph: bool,
    requires_persistence: bool,
    estimated_latency_ms: u32,
) -> Meta {
    let mut meta = Meta::new();
    meta.insert(
        "cognicode".to_string(),
        serde_json::json!({
            "stability": stability,
            "category": category,
            "requires_graph": requires_graph,
            "requires_persistence": requires_persistence,
            "estimated_latency_ms": estimated_latency_ms
        }),
    );
    meta
}

/// M3.2: Returns the dispatch timeout for a given tool category.
/// Categories with heavy graph algorithms (PageRank, IDF, communities)
/// get 60s; LSP-backed navigation gets 45s; in-memory search gets a
/// tight 500ms (search is expected to be sub-100ms); everything else
/// gets a 30s default. Unknown categories fall through to the default.
fn timeout_for_category(category: &str) -> Duration {
    match category {
        "graph" => Duration::from_secs(60),
        "navigation" => Duration::from_secs(45),
        "search" => Duration::from_millis(500),
        _ => Duration::from_secs(30),
    }
}

/// M3.3: Categories that use a stricter rate-limit key namespace.
/// Each tool in these categories gets its own `strict:<tool_name>`
/// bucket, separate from the regular `tool:<tool_name>` budget, so
/// heavy categories (graph analytics, LSP navigation, AIX NL search)
/// cannot exhaust the regular dispatcher budget.
const STRICT_RATE_LIMIT_CATEGORIES: &[&str] = &["graph", "navigation", "aix"];

/// Returns `true` if `category` should be rate-limited with a stricter
/// key namespace. See [`STRICT_RATE_LIMIT_CATEGORIES`].
fn is_strict_rate_limit_category(category: &str) -> bool {
    STRICT_RATE_LIMIT_CATEGORIES.contains(&category)
}

/// M3.2 / M3.3: Lazily-built map from tool name to its
/// `cognicode_meta.category`. Derived from [`build_all_tools`] on first
/// access and reused for the lifetime of the process. Tools without a
/// cognicode meta block or without a category string are absent from
/// the map (the lookup falls back to "unknown", which gets the default
/// 30s timeout and the non-strict key prefix).
fn tool_category_map() -> &'static HashMap<String, String> {
    use std::sync::OnceLock;
    static MAP: OnceLock<HashMap<String, String>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m = HashMap::new();
        for tool in build_all_tools() {
            let name = tool.name.to_string();
            if let Some(meta) = tool.meta.as_ref() {
                if let Some(cognicode) = meta.get("cognicode") {
                    if let Some(cat) = cognicode.get("category").and_then(|v| v.as_str()) {
                        m.insert(name, cat.to_string());
                    }
                }
            }
        }
        m
    })
}

/// M3.2 / M3.3: Resolve the category for a tool name. Falls back to
/// "unknown" when the tool has no cognicode meta or the meta is
/// missing the `category` field.
fn lookup_category(tool_name: &str) -> String {
    tool_category_map()
        .get(tool_name)
        .cloned()
        .unwrap_or_else(|| "unknown".to_string())
}

/// M3.4: Lazily-built map from tool name to its graph-dependency string
/// (extracted from the tool description via regex). The regex matches
/// "Requires build_graph first." and any similar dependency markers.
/// Returns `None` for tools that have no graph dependency requirement.
pub(crate) fn tool_graph_deps_map() -> &'static HashMap<String, String> {
    use std::sync::OnceLock;
    static MAP: OnceLock<HashMap<String, String>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m = HashMap::new();
        // Regex to capture "Requires X first." patterns from tool descriptions
        let re = Regex::new(r"(?i)(?:requires?\s+(\w+(?:\s+\w+)*)\s+first\.)")
            .expect("regex compilation error");
        for tool in build_all_tools() {
            if let Some(desc) = tool.description.as_deref() {
                if let Some(caps) = re.captures(desc) {
                    if let Some(dep) = caps.get(1) {
                        m.insert(tool.name.to_string(), dep.as_str().to_string());
                    }
                }
            }
        }
        m
    })
}

/// Returns the graph dependency requirement for a tool, if any.
/// For example, "iac_query" returns `Some("build_graph")` because its
/// description contains "Requires build_graph first."
pub fn lookup_tool_deps(tool_name: &str) -> Option<String> {
    tool_graph_deps_map().get(tool_name).cloned()
}

/// Returns the complete list of public MCP tool definitions.
/// This is the single source of truth for `tools/list` and the parity-test surface.
pub(crate) fn build_all_tools() -> Vec<Tool> {
    vec![
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
                    )
                    .with_meta(cognicode_meta("stable", "graph", false, false, 3000)),
                    Tool::new(
                        "get_file_symbols",
                        "Extract symbols (functions, classes, variables) from a source file. Set compressed=true for natural language summary. Set hierarchical=true for tree output.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string", "description": "Path to the source file" },
                                "compressed": { "type": "boolean", "description": "Return compressed natural language summary instead of JSON (default: false)" },
                                "hierarchical": { "type": "boolean", "description": "Group symbols by nesting (parent→children tree) (default: false)" }
                            },
                            "required": ["file_path"]
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "file", false, false, 150)),
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
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 250)),
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
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 250)),
                    Tool::new(
                        "find_usages",
                        "Find all usages of a symbol across the project.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "symbol_name": { "type": "string", "description": "Symbol to search" },
                                "include_declaration": { "type": "boolean", "description": "Include definition (default: true)" },
                                "context_lines": { "type": "integer", "description": "Number of surrounding source lines to include per usage (default: none)" }
                            },
                            "required": ["symbol_name"]
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "search", false, false, 400)),
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
                    )
                    .with_meta(cognicode_meta("stable", "quality", false, false, 200)),
                    Tool::new(
                        "get_entry_points",
                        "Find symbols with no incoming edges (entry points in the call graph). Requires build_graph first.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "compressed": { "type": "boolean", "description": "Return compressed natural language summary instead of JSON (default: false)" }
                            }
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 150)),
                    Tool::new(
                        "get_leaf_functions",
                        "Find symbols with no outgoing edges (leaf functions in the call graph). Requires build_graph first.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "compressed": { "type": "boolean", "description": "Return compressed natural language summary instead of JSON (default: false)" }
                            }
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 150)),
                    Tool::new(
                        "trace_path",
                        "Find execution path between two symbols using BFS.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "source": { "type": "string", "description": "Source symbol name (function or method)" },
                                "target": { "type": "string", "description": "Target symbol name (function or method)" },
                                "max_depth": { "type": "integer", "description": "Maximum depth for path search (default: 10)" },
                                "all": { "type": "boolean", "description": "If true, return ALL paths (BFS exhaustive). If false, return only the shortest path (default: false)" }
                            },
                            "required": ["source", "target"]
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 300)),
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
                    )
                    .with_meta(cognicode_meta("stable", "composite", true, false, 400)),
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
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 250)),
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
                    )
                    .with_meta(cognicode_meta("stable", "search", false, false, 300)),
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
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 500)),
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
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 200)),
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
                    )
                    .with_meta(cognicode_meta("stable", "file", false, false, 100)),
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
                    )
                    .with_meta(cognicode_meta("stable", "navigation", false, false, 150)),
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
                    )
                    .with_meta(cognicode_meta("stable", "navigation", false, false, 150)),
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
                    )
                    .with_meta(cognicode_meta("stable", "navigation", false, false, 300)),
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
                    )
                    .with_meta(cognicode_meta("stable", "file", false, false, 100)),
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
                    )
                    .with_meta(cognicode_meta("stable", "search", false, false, 400)),
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
                    )
                    .with_meta(cognicode_meta("stable", "file", false, false, 200)),
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
                    )
                    .with_meta(cognicode_meta("stable", "search", false, false, 800)),
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
                    )
                    .with_meta(cognicode_meta("stable", "file", false, false, 100)),
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
                                            "oldString": { "type": "string", "description": "The exact text to replace (required)" },
                                            "newString": { "type": "string", "description": "The replacement text (required)" }
                                        },
                                        "required": ["oldString", "newString"]
                                    }
                                }
                            },
                            "required": ["path", "edits"]
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "file", false, false, 200)),
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
                    )
                    .with_meta(cognicode_meta("stable", "refactor", false, false, 1000)),
                    // AIX-1: Smart Overview & Ranked Symbols
                    // AIX-2: Onboarding Plan & Auto Diagnose & Refactor Plan
                    // AIX-3: NL to Symbol & Ask About Code & Find Pattern
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
                    )
                    .with_meta(cognicode_meta("experimental", "aix", true, false, 5000)),
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
                    )
                    .with_meta(cognicode_meta("experimental", "aix", true, false, 5000)),
                    Tool::new(
                        "nl_to_symbol",
                        "Convert natural language descriptions to symbol matches using keyword extraction and semantic search.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "query": { "type": "string", "description": "Natural language description of symbol to find" },
                                "limit": { "type": "integer", "description": "Maximum number of results (default: 20)" }
                            },
                            "required": ["query"]
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "search", true, false, 500)),
                    // AIX-4: Compare Call Graphs & Detect API Breaks
                    // AIX-5: System Prompt Context & God Functions & Long Params
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
                    )
                    .with_meta(cognicode_meta("stable", "quality", true, false, 1000)),
                    Tool::new(
                        "detect_long_parameter_lists",
                        "Find functions with too many parameters that should be consolidated into structs.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "max_params": { "type": "integer", "description": "Maximum number of parameters allowed (default: 5)" }
                            }
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "quality", true, false, 600)),
                    // PL3: Symbol Hotness Tracking
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
                    )
                    .with_meta(cognicode_meta("gated", "composite", false, true, 500)),
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
                    )
                    .with_meta(cognicode_meta("stable", "composite", false, false, 300)),
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
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 2500)),
                    Tool::new(
                        "graph_all_paths",
                        "Find all simple paths between two symbols in the call graph. Requires build_graph first.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "from_symbol": { "type": "string", "description": "Source symbol name (substring match, case-insensitive)" },
                                "to_symbol": { "type": "string", "description": "Target symbol name (substring match, case-insensitive)" },
                                "max_hops": { "type": "integer", "description": "Maximum number of intermediate nodes (default: 5)" }
                            },
                            "required": ["from_symbol", "to_symbol"]
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 1500)),
                    Tool::new(
                        "graph_condensed",
                        "Compute the SCC condensation of the call graph: every strongly connected component is collapsed into a single node, producing an acyclic condensation DAG. Use to spot circular dependency clusters. Requires build_graph first.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {}
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 500)),
                    Tool::new(
                        "graph_god_nodes",
                        "Find god nodes — symbols with unusually high PageRank (above the supplied percentile). These are symbols that too many things depend on and are prime refactoring candidates. Requires build_graph first.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "percentile": { "type": "number", "description": "Percentile threshold in [0.0, 1.0] (default: 0.95). Symbols at or above this PageRank percentile are returned." }
                            }
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 1000)),
                    Tool::new(
                        "graph_reduced",
                        "Compute the transitive reduction of the call graph — the minimal set of dependency edges that preserves reachability. Redundant edges (implied by longer paths) are dropped. Requires build_graph first.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {}
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 500)),
                    Tool::new(
                        "graph_feedback_arcs",
                        "Find a feedback arc set — edges whose removal would make the call graph acyclic. The greedy heuristic is not optimal but fast; use the result as a starting point when breaking circular dependencies. Requires build_graph first.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {}
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 1000)),
                    // Phase 5: Community Detection (Label Propagation).
                    //
                    // `graph_communities` runs Label Propagation over the
                    // in-memory call graph and returns deterministic
                    // community labels. `graph_community_detail` drills
                    // into a single community, and `graph_surprising_
                    // connections` highlights edges that cross community
                    // boundaries (often a sign of unwanted coupling).
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
                        "graph_communities",
                        "Detect code communities using Label Propagation. Groups symbols that are tightly coupled into clusters. Returns communities with cohesion scores. Requires build_graph first.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "max_iterations": { "type": "integer", "description": "Max label propagation iterations (default: 100)" }
                            }
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 2500)),
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
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 500)),
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
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 1200)),
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
                    )
                    .with_meta(cognicode_meta("stable", "search", true, false, 400)),
                    Tool::new(
                        "graph_insights",
                        "Get a complete architecture health report: god nodes, circular dependencies, community overview, surprising cross-module connections, and a health score (0-100). Requires build_graph first.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {}
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "composite", true, false, 2000)),
                    Tool::new(
                        "graph_suggest_questions",
                        "Generate intelligent questions about the codebase architecture based on graph analysis. Helps identify areas that need attention. Requires build_graph first.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {}
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("experimental", "composite", true, false, 5000)),
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
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 800)),
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
                    )
                    .with_meta(cognicode_meta("stable", "composite", true, false, 1000)),
                    // Phase 3A: Proactive Tools
                    #[cfg(feature = "persistence")]
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
                    )
                    .with_meta(cognicode_meta("experimental", "quality", false, true, 1500)),
                    Tool::new(
                        "get_type_references",
                        "List type annotation references for a symbol (param types, return types, field types). Uses References edges from type-ref extraction. Requires build_graph first.",
                        Arc::new(serde_json::json!({"type":"object","properties":{"symbol_name":{"type":"string","description":"Symbol name"}},"required":["symbol_name"]}).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 250)),
                    Tool::new(
                        "get_imports",
                        "List all imports for a file. Uses Imports edges from the ingest pipeline. Requires build_graph first.",
                        Arc::new(serde_json::json!({"type":"object","properties":{"file_path":{"type":"string","description":"File path"}},"required":["file_path"]}).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 200)),
                    Tool::new(
                        "get_implementors",
                        "Find all types that implement a given trait/interface. Uses Implements edges. Requires build_graph first.",
                        Arc::new(serde_json::json!({"type":"object","properties":{"trait_name":{"type":"string","description":"Trait or interface name"}},"required":["trait_name"]}).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 400)),
                    Tool::new(
                        "get_members",
                        "List methods and fields of a class/struct. Uses Contains edges. Requires build_graph first.",
                        Arc::new(serde_json::json!({"type":"object","properties":{"class_name":{"type":"string","description":"Class or struct name"}},"required":["class_name"]}).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 250)),
                    Tool::new(
                        "graph_query_filtered",
                        "Graph query with provenance, node kind, and community filters. Requires build_graph first.",
                        Arc::new(serde_json::json!({"type":"object","properties":{"question":{"type":"string"},"limit":{"type":"integer"},"filters":{"type":"object","properties":{"provenance":{"type":"array","items":{"type":"string"}},"node_kinds":{"type":"array","items":{"type":"string"}},"community_id":{"type":"integer"},"exclude_kinds":{"type":"array","items":{"type":"string"}}}}},"required":["question"]}).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 800)),
                    Tool::new(
                        "export_callflow",
                        "Export a community-level Mermaid architecture call-flow diagram. Shows module-level relationships.",
                        Arc::new(serde_json::json!({"type":"object","properties":{"max_sections":{"type":"integer","description":"Max architecture sections (default: 8)"},"format":{"type":"string","enum":["code"]}}}).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "composite", true, false, 600)),
                    // SOLID Audit tool — heuristic-based SOLID principle analysis
                    Tool::new(
                        "solid_audit",
                        "Analyze code for SOLID principle violations (SRP, OCP, LSP, ISP, DIP). Returns violations with severity, location, and suggestions. Requires build_graph first.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {}
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "quality", true, false, 2000)),

    // Batch D: Agent Task Tools (bidirectional interaction)
                    // Sprint 5.3: graph_diff and graph_timeline tools
                    Tool::new(
                        "graph_diff",
                        "Compare two graph reports by date to show changes in symbol count, edge count, and health score. Requires PostgresRepository.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "baseline_date": {
                                    "type": "string",
                                    "description": "Baseline date to compare against (YYYY-MM-DD format)"
                                },
                                "current": {
                                    "type": "boolean",
                                    "description": "If true, compare against the latest report (default: false)"
                                }
                            },
                            "required": ["baseline_date"]
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("gated", "graph", true, true, 2000)),
                    Tool::new(
                        "graph_timeline",
                        "Show trend data over N days for symbol count, edge count, and health score. Requires PostgresRepository.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "days": {
                                    "type": "integer",
                                    "description": "Number of days to look back (default: 30)"
                                }
                            }
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("gated", "graph", true, true, 2000)),
                    Tool::new(
                        "graph_analyze",
                        "Run advanced graph algorithms: scc, reduced, or feedback_arcs.",
                        Arc::new(serde_json::json!({"type":"object","properties":{"mode":{"type":"string","enum":["scc","reduced","feedback_arcs"]}}}).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 1500)),
                    Tool::new(
                        "project_overview",
                        "Get a comprehensive project overview at quick, medium, or detailed levels.",
                        Arc::new(serde_json::json!({"type":"object","properties":{"detail":{"type":"string","enum":["quick","medium","detailed"]}}}).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("experimental", "composite", true, false, 3000)),
                    Tool::new(
                        "codebase_map",
                        "Generate a compact, LLM-optimized codebase map. Format: compact (~400 tokens) or detailed (~2000).",
                        Arc::new(serde_json::json!({"type":"object","properties":{"format":{"type":"string","enum":["compact","detailed"]}}}).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "composite", true, false, 1000)),
                    Tool::new(
                        "project_insights",
                        "Dashboard in a single call: symbols, edges, entry points, dead code, health score, hot paths.",
                        Arc::new(serde_json::json!({"type":"object","properties":{}}).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("experimental", "composite", true, false, 3000)),
                    Tool::new(
                        "smart_search",
                        "Run semantic_search + ranked_symbols + graph_search_idf in parallel with deduplication. Returns merged results ranked by combined score.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "query": { "type": "string", "description": "Search query string" },
                                "limit": { "type": "integer", "description": "Maximum number of results (default: 20)" }
                            },
                            "required": ["query"]
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "composite", true, false, 2000)),
                    Tool::new(
                        "compare_graph",
                        "Compare the current call graph snapshot vs the latest PostgreSQL graph_report. Shows added/removed symbols and metric deltas. Requires PostgreSQL persistence.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "baseline": { "type": "string", "description": "Baseline reference: 'latest' or a date string YYYY-MM-DD (default: 'latest')" }
                            }
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("gated", "composite", true, true, 2000)),
                    Tool::new(
                        "check_architecture",
                        "Detect cycles and architecture violations using Tarjan SCC algorithm. Requires build_graph first.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "scope": { "type": "string", "description": "Optional scope to filter analysis (e.g., module name)" }
                            }
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 200)),
                    Tool::new(
                        "graph_checkpoint",
                        "Manage graph checkpoints: create (build+checkpoint), current (get latest), restore (get by id), list (list all). Requires build_graph first.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "operation": { "type": "string", "description": "Operation: create, current, restore, list (default: create)" },
                                "checkpoint_id": { "type": "integer", "description": "Checkpoint ID for restore operation" }
                            }
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 500)),
                    Tool::new(
                        "merge_graphs",
                        "Merge per-file call graphs into a consolidated project graph.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "file_paths": { "type": "array", "items": {"type": "string"}, "description": "List of source file paths to merge" }
                            },
                            "required": ["file_paths"]
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "graph", false, false, 500)),
                    Tool::new(
                        "build_lightweight_index",
                        "Build a lightweight symbol index for fast lookups. Supports strategies: lightweight, on_demand, per_file, full.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "directory": { "type": "string", "description": "Directory to index (default: cwd)" },
                                "strategy": { "type": "string", "enum": ["lightweight", "on_demand", "per_file", "full"], "description": "Indexing strategy (default: lightweight)" }
                            }
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "graph", true, false, 300)),
                    Tool::new(
                        "reparse_on_edit",
                        "Incrementally reindex changed files without rebuilding the full graph. Much faster than full rebuild for small edits. Requires persistence feature.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "file_paths": { "type": "array", "items": {"type": "string"}, "description": "List of changed file paths to reindex" }
                            },
                            "required": ["file_paths"]
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("experimental", "graph", false, true, 200)),
                    Tool::new(
                        "iac_query",
                        "Query infrastructure-as-code resources (Terraform, Ansible) and their dependencies from the graph. Requires build_graph first. Accepts bare resource names (aws_instance.web) or canonical IDs (tf:main.tf:aws_instance.web). Returns resource type, dependencies, and dependents. Uses PostgreSQL when available for persistent storage; falls back to in-memory graph.",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "resource_id": { "type": "string", "description": "Resource ID: bare name (aws_instance.web) or canonical (tf:main.tf:aws_instance.web)" },
                                "depth": { "type": "integer", "description": "Max traversal depth (default: 2)", "default": 2 }
                            },
                            "required": ["resource_id"]
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("experimental", "infra", true, true, 500)),
                    Tool::new(
                        "ingest",
                        "Run the full ingest pipeline (scan + extract + pg_upsert) on a workspace directory. Populates PostgreSQL with graph nodes and edges for IaC resources (Terraform, Ansible) and code symbols. Must be called before iac_query when using Mode B (--postgres). Requires PostgreSQL connection (build_graph is not a dependency).",
                        Arc::new(serde_json::json!({
                            "type": "object",
                            "properties": {
                                "directory": { "type": "string", "description": "Directory to ingest (defaults to working directory)" }
                            }
                        }).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("experimental", "infra", true, true, 30000)),
                    Tool::new(
                        "review_pr",
                        "Analyze PR impact: provide changed files, get risk level, impacted files, and breaking changes.",
                        Arc::new(serde_json::json!({"type":"object","properties":{"files":{"type":"array","items":{"type":"string"},"description":"Changed file paths"}},"required":["files"]}).as_object().cloned().unwrap()),
                    )
                    .with_meta(cognicode_meta("stable", "composite", false, false, 2000)),

    ]
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
            let all_tools = build_all_tools();

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
///
/// M1.1: All tool calls flow through this single instrumentation boundary.
/// Timing, call count, and error classification are recorded here — no
/// per-handler timing or record_tool_usage calls needed.
async fn call_tool_handler(
    ctx: &HandlerContext,
    request: CallToolRequestParams,
) -> InterfaceResult<String> {
    let tool_name = request.name.as_ref();
    let arguments = request.arguments.unwrap_or_default();

    // M1.1: Centralized instrumentation boundary
    let start = Instant::now();
    let metrics = get_global_metrics();

    // M3.2 / M3.3: Resolve the tool's category once. Used both for
    // timeout selection (M3.2) and rate-limit key prefixing (M3.3).
    let category = lookup_category(tool_name);

    // M3.3: Pre-match rate-limit check. The key is per-tool so each
    // tool has its own 100/60s budget. Expensive categories (graph,
    // navigation, aix) use a stricter key prefix to keep their
    // budgets separate from the rest of the dispatcher.
    let rate_key = if is_strict_rate_limit_category(&category) {
        format!("strict:{}", tool_name)
    } else {
        format!("tool:{}", tool_name)
    };
    if !ctx.rate_limiter().check_with_key(&rate_key) {
        let err: InterfaceResult<String> =
            Err(InterfaceError::Internal("rate_limit_exceeded".to_string()));
        let status = crate::interface::mcp::status::classify_status(tool_name, &err);
        let duration_ms = start.elapsed().as_millis() as f64;
        // M3.4: Emit the structured per-call log line for parity
        // with the success path so dashboards see the rate-limit hit.
        tracing::info!(
            tool = %tool_name,
            duration_ms = %duration_ms as u64,
            status = %status,
            "tool_call"
        );
        if let Some(m) = &metrics {
            m.calls.add(
                1,
                &[
                    KeyValue::new("tool", tool_name.to_string()),
                    KeyValue::new("status", status),
                ],
            );
            m.duration.record(
                duration_ms,
                &[
                    KeyValue::new("tool", tool_name.to_string()),
                    KeyValue::new("status", status),
                ],
            );
            // Record the error with error_type="rate_limit_exceeded"
            // so it shows up in the M1.6 error metric.
            m.errors.add(
                1,
                &[
                    KeyValue::new("tool", tool_name.to_string()),
                    KeyValue::new("error_type", "rate_limit_exceeded"),
                ],
            );
        }
        return err;
    }

    // M3.2: Wrap the per-tool match dispatch with a category-based
    // timeout. The match block itself is unchanged; only the wrapper
    // differs. The timeout is selected by `timeout_for_category` from
    // the cognicode_meta.category of the tool. On timeout, the
    // dispatch future is dropped and we return Internal("timeout").
    let timeout = timeout_for_category(&category);
    let dispatch = async {
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

            // M2.1: Record graph statistics after successful build
            if output.success {
                let graph = ctx.analysis_service.get_project_graph();
                let symbols = graph.symbol_count() as u64;
                let edges = graph.edge_count() as u64;
                let health_score =
                    crate::application::services::graph_insights::GraphInsightsService::analyze(&graph)
                        .health_score;
                if let Some(m) = &metrics {
                    m.record_graph_stats(symbols, edges, health_score);
                }
                // M3.1: Flip the readiness flag so /ready returns 200
                ctx.mark_graph_loaded();
            }

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

        // AIX-2: Onboarding Plan & Auto Diagnose & Refactor Plan

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

        // AIX-5: System Prompt Context & God Functions & Long Params
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

        // AVC: Agent-Verifiable Context tools
        "generate_contract" => {
            let input: crate::interface::mcp::schemas::GenerateContractInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_generate_contract(ctx, input)
                    .await?;
            Ok(serde_json::to_string(&output)?)
        }
        "validate_contract" => {
            let input: crate::interface::mcp::schemas::ValidateContractInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_validate_contract(ctx, input)
                    .await?;
            Ok(serde_json::to_string(&output)?)
        }
        // Phase 3A: Proactive Tools
        #[cfg(feature = "persistence")]
        // Detect Drift tool (S7000-S7003)
        "detect_drift" => {
            let input: crate::interface::mcp::schemas::DetectDriftInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::aix_handlers::handle_detect_drift(ctx, input)
                    .await?;
            Ok(serde_json::to_string(&output)?)
        }
        // Batch D: Agent Task Tools (bidirectional interaction)

        // Phase 4b: Graph analytics tools (extracted to graph_handlers.rs)
        "graph_pagerank" => {
            let input: crate::interface::mcp::schemas::GraphPageRankInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::graph_handlers::handle_graph_pagerank(ctx, input)
                    .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_all_paths" => {
            let input: crate::interface::mcp::schemas::GraphAllPathsInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::graph_handlers::handle_graph_all_paths(ctx, input)
                    .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_condensed" => {
            let input: crate::interface::mcp::schemas::GraphCondensedInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::graph_handlers::handle_graph_condensed(ctx, input)
                    .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_god_nodes" => {
            let input: crate::interface::mcp::schemas::GraphGodNodesInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::graph_handlers::handle_graph_god_nodes(ctx, input)
                    .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_reduced" => {
            let input: crate::interface::mcp::schemas::GraphReducedInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::graph_handlers::handle_graph_reduced(ctx, input)
                    .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_feedback_arcs" => {
            let input: crate::interface::mcp::schemas::GraphFeedbackArcsInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::graph_handlers::handle_graph_feedback_arcs(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }

        // Phase 5: Community Detection handlers (extracted to graph_handlers.rs)
        "graph_communities" => {
            let input: crate::interface::mcp::schemas::GraphCommunitiesInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_handlers::handle_graph_communities(
                ctx, input,
            )
            .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_community_detail" => {
            let input: crate::interface::mcp::schemas::GraphCommunityDetailInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::graph_handlers::handle_graph_community_detail(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_surprising_connections" => {
            let input: crate::interface::mcp::schemas::GraphSurprisingConnectionsInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::graph_handlers::handle_graph_surprising_connections(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }

        // Phase 6: IDF-weighted Search & Unified Insights (extracted to graph_handlers.rs)
        "graph_search_idf" => {
            let input: crate::interface::mcp::schemas::GraphSearchIdfInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_handlers::handle_graph_search_idf(
                ctx, input,
            )
            .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_insights" => {
            let input: crate::interface::mcp::schemas::GraphInsightsInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::graph_handlers::handle_graph_insights(ctx, input)
                    .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_suggest_questions" => {
            let input: crate::interface::mcp::schemas::GraphSuggestQuestionsInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::graph_handlers::handle_graph_suggest_questions(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        // Sprint 2: Graphify-style tools (ADR-026)
        "graph_query" => {
            let input: crate::interface::mcp::handlers::graph_query_handlers::GraphQueryInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_query_handlers::handle_graph_query(
                ctx, input,
            )
            .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_explain" => {
            let input: crate::interface::mcp::handlers::graph_query_handlers::GraphExplainInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::graph_query_handlers::handle_graph_explain(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        // Edge-type query tools (ADR-026)
        "get_type_references" => {
            let input: crate::interface::mcp::handlers::graph_query_handlers::GetTypeRefsInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::graph_query_handlers::handle_get_type_references(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "get_imports" => {
            let input: crate::interface::mcp::handlers::graph_query_handlers::GetImportsInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_query_handlers::handle_get_imports(
                ctx, input,
            )
            .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "get_implementors" => {
            let input: crate::interface::mcp::handlers::graph_query_handlers::GetImplementorsInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::graph_query_handlers::handle_get_implementors(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "get_members" => {
            let input: crate::interface::mcp::handlers::graph_query_handlers::GetMembersInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::graph_query_handlers::handle_get_members(
                ctx, input,
            )
            .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_query_filtered" => {
            let input: crate::interface::mcp::handlers::graph_query_handlers::GraphQueryFilteredInput = serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::graph_query_handlers::handle_graph_query_filtered(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "export_callflow" => {
            let input: crate::interface::mcp::handlers::graph_query_handlers::ExportCallflowInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::graph_query_handlers::handle_export_callflow(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        // Sprint 5: Consolidated + High-value tools (ADR-027 + ADR-028)
        "smart_search" => {
            let input: crate::interface::mcp::schemas::SmartSearchInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::consolidated_handlers::handle_smart_search(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_analyze" => {
            let input: crate::interface::mcp::handlers::consolidated_handlers::GraphAnalyzeInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::consolidated_handlers::handle_graph_analyze(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "project_overview" => {
            let input: crate::interface::mcp::handlers::consolidated_handlers::ProjectOverviewInput = serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::consolidated_handlers::handle_project_overview(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "compare_graph" => {
            let input: crate::interface::mcp::schemas::CompareGraphInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::consolidated_handlers::handle_compare_graph(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "codebase_map" => {
            let input: crate::interface::mcp::handlers::consolidated_handlers::CodebaseMapInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::consolidated_handlers::handle_codebase_map(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "project_insights" => {
            let input: crate::interface::mcp::handlers::consolidated_handlers::ProjectInsightsInput = serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::consolidated_handlers::handle_project_insights(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "review_pr" => {
            let input: crate::interface::mcp::handlers::consolidated_handlers::ReviewPrInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::consolidated_handlers::handle_review_pr(
                ctx, input,
            )
            .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "iac_query" => {
            let input: crate::interface::mcp::handlers::consolidated_handlers::IacQueryInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::consolidated_handlers::handle_iac_query(
                ctx, input,
            )
            .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "ingest" => {
            let input: crate::interface::mcp::handlers::consolidated_handlers::IngestInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::consolidated_handlers::handle_ingest(
                ctx, input,
            )
            .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        // SOLID Audit tool
        "solid_audit" => {
            let output = crate::interface::mcp::handlers::handle_solid_audit(ctx).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        // Architecture check
        "check_architecture" => {
            let input: crate::interface::mcp::schemas::CheckArchitectureInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_check_architecture(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        // Graph checkpoint management
        "graph_checkpoint" => {
            let input: crate::interface::mcp::handlers::consolidated_handlers::GraphCheckpointInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::consolidated_handlers::handle_graph_checkpoint(
                ctx, input,
            )
            .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        // Merge per-file graphs into consolidated project graph
        "merge_graphs" => {
            let input: crate::interface::mcp::schemas::MergeGraphsInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_merge_graphs(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        // Build lightweight symbol index
        "build_lightweight_index" => {
            let input: crate::interface::mcp::schemas::BuildIndexInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::handle_build_lightweight_index(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        // Incremental reindex after edits (requires persistence feature)
        "reparse_on_edit" => {
            let input: crate::interface::mcp::schemas::ReparseOnEditInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::aix_handlers::handle_reparse_on_edit(ctx, input).await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        // Sprint 5.3: graph_diff and graph_timeline tools
        "graph_diff" => {
            let input: crate::interface::mcp::handlers::consolidated_handlers::GraphDiffInput =
                serde_json::from_value(arguments.into())?;
            let output = crate::interface::mcp::handlers::consolidated_handlers::handle_graph_diff(
                ctx, input,
            )
            .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }
        "graph_timeline" => {
            let input: crate::interface::mcp::handlers::consolidated_handlers::GraphTimelineInput =
                serde_json::from_value(arguments.into())?;
            let output =
                crate::interface::mcp::handlers::consolidated_handlers::handle_graph_timeline(
                    ctx, input,
                )
                .await?;
            Ok(serde_json::to_string_pretty(&output)?)
        }

        _ => return Err(InterfaceError::ToolNotFound(tool_name.to_string())),
    }
    };
    let result = match tokio::time::timeout(timeout, dispatch).await {
        Ok(inner) => inner,
        Err(_elapsed) => Err(InterfaceError::Internal("timeout".to_string())),
    };

    // M1.2: Classify status for metrics
    let status = crate::interface::mcp::status::classify_status(tool_name, &result);

    // M1.1: Record duration + classify status for error recording
    let duration_ms = start.elapsed().as_millis() as f64;

    // M3.4: Structured per-call log line — one entry per tool call
    // (success, error, gated, missing, skipped). Emitted at the
    // universal instrumentation boundary so every tool flow is captured
    // uniformly. Field names match M3-Sprint-spec.md §M3.4.
    tracing::info!(
        tool = %tool_name,
        duration_ms = %duration_ms as u64,
        status = %status,
        "tool_call"
    );
    if let Some(m) = &metrics {
        // M1.6: Record calls with tool + status labels
        m.calls.add(
            1,
            &[
                KeyValue::new("tool", tool_name.to_string()),
                KeyValue::new("status", status),
            ],
        );
        // M1.6: Record duration with tool + status labels
        m.duration.record(
            duration_ms,
            &[
                KeyValue::new("tool", tool_name.to_string()),
                KeyValue::new("status", status),
            ],
        );
    }

    // M1.1: Record error metrics (error_type is separate from status)
    if let Err(e) = &result {
        if let Some(m) = &metrics {
            // M3.2 / M3.3: Distinguish timeout and rate_limit_exceeded
            // from generic errors. Both are conveyed as
            // `InterfaceError::Internal(<sentinel>)` so we match on
            // the inner string to keep the error_type taxonomy
            // machine-readable.
            let error_type = match e {
                InterfaceError::ToolNotFound(_) => "missing",
                InterfaceError::Internal(msg) if msg == "timeout" => "timeout",
                InterfaceError::Internal(msg) if msg == "rate_limit_exceeded" => {
                    "rate_limit_exceeded"
                }
                _ => "error",
            };
            m.errors.add(
                1,
                &[
                    KeyValue::new("tool", tool_name.to_string()),
                    KeyValue::new("error_type", error_type),
                ],
            );
        }
    }

    result
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

    // ============================================================================
    // M3.2 — Per-tool timeout
    // ============================================================================

    #[test]
    fn test_timeout_for_category_known_categories() {
        // graph analytics is allowed the longest window (60s)
        assert_eq!(timeout_for_category("graph"), Duration::from_secs(60));
        // LSP navigation is allowed 45s
        assert_eq!(
            timeout_for_category("navigation"),
            Duration::from_secs(45)
        );
        // in-memory search is the tightest bound (500ms)
        assert_eq!(
            timeout_for_category("search"),
            Duration::from_millis(500)
        );
        // file operations default to 30s
        assert_eq!(timeout_for_category("file"), Duration::from_secs(30));
        // composite/quality/refactor/aix are also 30s
        assert_eq!(
            timeout_for_category("composite"),
            Duration::from_secs(30)
        );
        assert_eq!(
            timeout_for_category("quality"),
            Duration::from_secs(30)
        );
        assert_eq!(
            timeout_for_category("refactor"),
            Duration::from_secs(30)
        );
        assert_eq!(timeout_for_category("aix"), Duration::from_secs(30));
    }

    #[test]
    fn test_timeout_for_category_unknown_defaults_to_30s() {
        // Unknown categories and the empty string fall through to the
        // 30s default so we never accidentally disable the timeout.
        assert_eq!(
            timeout_for_category("unknown_category"),
            Duration::from_secs(30)
        );
        assert_eq!(timeout_for_category(""), Duration::from_secs(30));
    }

    #[tokio::test]
    async fn test_timeout_fires_when_handler_takes_longer() {
        // M3.2: A handler that exceeds the category timeout must be
        // cancelled and replaced with a timeout error. We exercise
        // the `tokio::time::timeout` wrapper directly here without
        // going through the full `call_tool_handler` (which needs a
        // real `HandlerContext` and would be slower to set up).
        let timeout = timeout_for_category("search"); // 500ms — fast
        let slow_handler = async {
            // Sleep longer than the search timeout (2s > 500ms)
            tokio::time::sleep(Duration::from_secs(2)).await;
            Ok::<_, InterfaceError>("should not reach here".to_string())
        };
        let result = tokio::time::timeout(timeout, slow_handler).await;
        assert!(
            result.is_err(),
            "expected tokio::time::timeout to fire on slow handler"
        );
        // Map the Elapsed to the same error we use in the boundary.
        let err: InterfaceResult<String> =
            Err(InterfaceError::Internal("timeout".to_string()));
        assert!(matches!(err, Err(InterfaceError::Internal(ref m)) if m == "timeout"));
    }

    #[tokio::test]
    async fn test_timeout_does_not_fire_when_handler_returns_quickly() {
        // M3.2: Fast handlers must NOT be cancelled.
        let timeout = timeout_for_category("search"); // 500ms
        let fast_handler = async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok::<_, InterfaceError>("done".to_string())
        };
        let result = tokio::time::timeout(timeout, fast_handler).await;
        assert!(result.is_ok(), "fast handler must not be timed out");
        assert_eq!(result.unwrap().unwrap(), "done");
    }

    // ============================================================================
    // M3.3 — Per-tool rate limit + strict category namespacing
    // ============================================================================

    #[test]
    fn test_is_strict_rate_limit_category() {
        // Categories that get their own rate-limit namespace.
        assert!(is_strict_rate_limit_category("graph"));
        assert!(is_strict_rate_limit_category("navigation"));
        assert!(is_strict_rate_limit_category("aix"));
        // Cheap categories share the regular namespace.
        assert!(!is_strict_rate_limit_category("file"));
        assert!(!is_strict_rate_limit_category("search"));
        assert!(!is_strict_rate_limit_category("quality"));
        assert!(!is_strict_rate_limit_category("refactor"));
        assert!(!is_strict_rate_limit_category("composite"));
        // Unknown / empty
        assert!(!is_strict_rate_limit_category(""));
        assert!(!is_strict_rate_limit_category("unknown"));
    }

    #[test]
    fn test_tool_category_map_contains_known_tools() {
        // M3.2 / M3.3: The lazy-init map must be populated from
        // `build_all_tools()` and include a representative sample of
        // tool names mapped to their declared category. A failure
        // here means the timeout / rate-limit routing will silently
        // fall back to the 30s default and the non-strict key prefix.
        let map = tool_category_map();
        assert_eq!(map.get("build_graph").map(|s| s.as_str()), Some("graph"));
        assert_eq!(
            map.get("go_to_definition").map(|s| s.as_str()),
            Some("navigation")
        );
        assert_eq!(
            map.get("search_content").map(|s| s.as_str()),
            Some("search")
        );
        assert_eq!(
            map.get("ask_about_code").map(|s| s.as_str()),
            Some("aix")
        );
        assert_eq!(
            map.get("read_file").map(|s| s.as_str()),
            Some("file")
        );
        // Every tool declared in `build_all_tools()` must have a
        // category — otherwise the lookup is incomplete and
        // roundtrip parity tests will diverge.
        let total = build_all_tools().len();
        assert_eq!(map.len(), total, "every tool must have a category");
    }

    #[test]
    fn test_lookup_category_falls_back_to_unknown() {
        // Unknown tool names must NOT panic and must return a
        // "unknown" placeholder that maps to the default 30s timeout
        // and the non-strict rate-limit key prefix.
        assert_eq!(lookup_category("definitely_not_a_real_tool"), "unknown");
        assert_eq!(lookup_category(""), "unknown");
    }

    #[test]
    fn test_rate_limiter_dispatch_key_separation() {
        // M3.3: Different rate-limit key namespaces (tool: vs
        // strict:) must track tokens independently so an expensive
        // tool cannot exhaust the regular dispatcher budget.
        let limiter = crate::interface::mcp::security::RateLimiter::new(2, 60);
        // Strict graph keys: 2 hits each, then exhausted.
        assert!(limiter.check_with_key("strict:graph_pagerank"));
        assert!(limiter.check_with_key("strict:graph_pagerank"));
        assert!(!limiter.check_with_key("strict:graph_pagerank"));
        // Regular file keys: untouched by the graph bucket.
        assert!(limiter.check_with_key("tool:read_file"));
        assert!(limiter.check_with_key("tool:read_file"));
        assert!(!limiter.check_with_key("tool:read_file"));
    }

    // ============================================================================
    // M3.4 — Tool graph-dependency extraction
    // ============================================================================

    #[test]
    fn test_tool_graph_deps_map_extracts_requires_build_graph() {
        // The regex should extract "build_graph" from tool descriptions
        // that contain "Requires build_graph first."
        let map = tool_graph_deps_map();
        // iac_query now has "Requires build_graph first." in its description
        assert_eq!(
            map.get("iac_query").map(|s| s.as_str()),
            Some("build_graph"),
            "iac_query should have build_graph as a dependency"
        );
        // get_call_hierarchy has "Requires build_graph first." in its description
        assert_eq!(
            map.get("get_call_hierarchy").map(|s| s.as_str()),
            Some("build_graph"),
            "get_call_hierarchy should have build_graph as a dependency"
        );
    }

    #[test]
    fn test_lookup_tool_deps_returns_optional_deps() {
        // lookup_tool_deps should return Some("build_graph") for tools
        // that require it, and None for tools that don't.
        assert_eq!(
            lookup_tool_deps("iac_query"),
            Some("build_graph".to_string())
        );
        assert_eq!(
            lookup_tool_deps("get_call_hierarchy"),
            Some("build_graph".to_string())
        );
        assert_eq!(
            lookup_tool_deps("read_file"),
            None,
            "read_file should not have graph dependencies"
        );
        assert_eq!(
            lookup_tool_deps("nonexistent_tool"),
            None,
            "nonexistent tools should return None"
        );
    }

    #[test]
    fn test_tool_graph_deps_map_consistency() {
        // Every tool in build_all_tools() that has "Requires build_graph first."
        // in its description must be present in the deps map.
        let deps_map = tool_graph_deps_map();
        let re = Regex::new(r"(?i)(?:requires?\s+(\w+(?:\s+\w+)*)\s+first\.)").unwrap();
        for tool in build_all_tools() {
            if let Some(desc) = tool.description.as_deref() {
                if re.captures(desc).is_some() {
                    // Verify the tool is in the deps map by name matching
                    let tool_name = tool.name.to_string();
                    assert!(
                        deps_map.contains_key(&tool_name),
                        "Tool '{}' has a requires clause but is not in tool_graph_deps_map",
                        tool_name
                    );
                }
            }
        }
    }
}
