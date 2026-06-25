//! MCP server entry point for the Explorer.
//!
//! This module provides the top-level [`ExplorerMcpHandler`] which bridges
//! the ISP-segregated [`ToolHandlerRegistry`](super::handler::ToolHandlerRegistry)
//! to the MCP JSON-RPC protocol.
//!
//! ## Tool constants
//!
//! All tool names are declared here as `pub const TOOL_*`. The same names are
//! re-exported through [`super`](crate::mcp) so handler submodules can import
//! them without a cyclical path.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ErrorData, Implementation, ListToolsResult,
    PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use cognicode_core::domain::traits::GraphQueryPort;

pub use super::context::McpContext;
pub use super::error::ToolError;
pub use super::handler::{ToolHandler, ToolHandlerRegistry};
use crate::facades::LensExecutor;
use crate::facades::graph::GraphServiceImpl;
use crate::facades::moldql::MoldQLServiceImpl;
use crate::facades::persistence::PersistenceServiceImpl;
use crate::facades::search::SearchServiceImpl;
use crate::facades::view::ViewServiceImpl;
use crate::facades::workspace::WorkspaceServiceImpl;
use crate::facades::{
    GraphService, MoldQLService, PersistenceService, SearchService, ViewService, WorkspaceService,
};
use crate::ports::source_reader::SourceReader;
use crate::ports::symbol_repository::SymbolRepository;
use crate::session::SessionRegistry;

/// Sentinel value for `max_depth` when none is supplied.
pub const DEFAULT_IMPACT_RADIUS_DEPTH: usize = 5;

/// Sentinel value for `max_depth` on `graph_subgraph` when none is supplied.
pub const DEFAULT_SUBGRAPH_DEPTH: usize = 3;

/// Default page size for `graph_search` (multimodal).
pub const DEFAULT_GRAPH_SEARCH_LIMIT: i64 = 50;

/// Maximum page size for `graph_search` (multimodal).
pub const MAX_GRAPH_SEARCH_LIMIT: i64 = 200;

// ============================================================================
// Tool name constants — sorted alphabetically within group
// ============================================================================

/// `explorer_open_workspace` — open (or re-open) a workspace by path.
pub const TOOL_OPEN_WORKSPACE: &str = "explorer_open_workspace";

/// `explorer_spotter_search` — search symbols by name with optional kind filter.
pub const TOOL_SPOTTER_SEARCH: &str = "explorer_spotter_search";

/// `explorer_inspect_object` — inspect an object by its MVP id.
pub const TOOL_INSPECT_OBJECT: &str = "explorer_inspect_object";

/// `explorer_get_views` — list available views for an object.
pub const TOOL_GET_VIEWS: &str = "explorer_get_views";

/// `explorer_get_view` — build a specific contextual view.
pub const TOOL_GET_VIEW: &str = "explorer_get_view";

/// `explorer_get_lenses` — list available lenses for an object.
pub const TOOL_GET_LENSES: &str = "explorer_get_lenses";

/// `explorer_apply_lens` — apply a lens to an object.
pub const TOOL_APPLY_LENS: &str = "explorer_apply_lens";

/// `explorer_query_moldql` — execute a MoldQL query.
pub const TOOL_QUERY_MOLDQL: &str = "explorer_query_moldql";

/// `impact_radius` — predecessor (reverse) BFS from a root symbol.
pub const TOOL_IMPACT_RADIUS: &str = "impact_radius";

/// `impact_forward_radius` — successor (forward) BFS from a root symbol.
pub const TOOL_IMPACT_FORWARD_RADIUS: &str = "impact_forward_radius";

/// `impact_has_path` — check if a directed path exists between two symbols.
pub const TOOL_IMPACT_HAS_PATH: &str = "impact_has_path";

/// `impact_shortest_path` — compute lowest-cost path between two symbols.
pub const TOOL_IMPACT_SHORTEST_PATH: &str = "impact_shortest_path";

/// `impact_detect_cycles` — find all non-trivial strongly connected components.
pub const TOOL_IMPACT_DETECT_CYCLES: &str = "impact_detect_cycles";

/// `impact_component` — return the undirected connected component containing a symbol.
pub const TOOL_IMPACT_COMPONENT: &str = "impact_component";

/// `graph_subgraph` — extract a bounded neighborhood subgraph.
pub const TOOL_GRAPH_SUBGRAPH: &str = "graph_subgraph";

/// `graph_cluster` — cluster the graph by SCC or connected components.
pub const TOOL_GRAPH_CLUSTER: &str = "graph_cluster";

/// `graph_explain` — explain the lowest-cost path between two symbols.
pub const TOOL_GRAPH_EXPLAIN: &str = "graph_explain";

/// `graph_pagerank` — PageRank scores for a subgraph.
pub const TOOL_GRAPH_PAGERANK: &str = "graph_pagerank";

/// `graph_god_nodes` — find god nodes in a subgraph.
pub const TOOL_GRAPH_GOD_NODES: &str = "graph_god_nodes";

/// `graph_communities` — detect communities via Label Propagation.
pub const TOOL_GRAPH_COMMUNITIES: &str = "graph_communities";

/// `graph_community_god_nodes` — find god nodes within each community.
pub const TOOL_GRAPH_COMMUNITY_GOD_NODES: &str = "graph_community_god_nodes";

/// `graph_surprising_connections` — find cross-community edges.
pub const TOOL_GRAPH_SURPRISING_CONNECTIONS: &str = "graph_surprising_connections";

/// `graph_transitive_reduction` — minimal edge set preserving reachability.
pub const TOOL_GRAPH_TRANSITIVE_REDUCTION: &str = "graph_transitive_reduction";

/// `graph_feedback_arc_set` — edges to break all cycles.
pub const TOOL_GRAPH_FEEDBACK_ARC_SET: &str = "graph_feedback_arc_set";

/// `graph_all_simple_paths` — enumerate simple paths from→to.
pub const TOOL_GRAPH_ALL_SIMPLE_PATHS: &str = "graph_all_simple_paths";

/// `detect_architecture_drift` — compare inferred C4 architecture against expected.
pub const TOOL_DETECT_ARCHITECTURE_DRIFT: &str = "detect_architecture_drift";

/// `cognicode_ask` — natural-language front-end that classifies a question.
pub const TOOL_ASK: &str = "cognicode_ask";

/// `brain_open` — open a new brain session.
pub const TOOL_BRAIN_OPEN: &str = "brain_open";

/// `brain_attach` — rejoin an existing brain session.
pub const TOOL_BRAIN_ATTACH: &str = "brain_attach";

/// `brain_ask` — ask a question within a brain session (focus-aware).
pub const TOOL_BRAIN_ASK: &str = "brain_ask";

/// `brain_focus` — set the session's focus node.
pub const TOOL_BRAIN_FOCUS: &str = "brain_focus";

/// `brain_status` — get session status and metadata.
pub const TOOL_BRAIN_STATUS: &str = "brain_status";

/// `brain_close` — close (invalidate) a session.
pub const TOOL_BRAIN_CLOSE: &str = "brain_close";

/// `view_save` — persist a named view projection.
pub const TOOL_VIEW_SAVE: &str = "view_save";

/// `view_load` — load and re-invoke a saved named view.
pub const TOOL_VIEW_LOAD: &str = "view_load";

/// `view_list` — list all named views for a scope.
pub const TOOL_VIEW_LIST: &str = "view_list";

/// `view_delete` — delete a named view by id.
pub const TOOL_VIEW_DELETE: &str = "view_delete";

/// `brain_add_space` — register a space in a session (multimodal).
pub const TOOL_BRAIN_ADD_SPACE: &str = "brain_add_space";

/// `brain_remove_space` — unregister a space from a session (multimodal).
pub const TOOL_BRAIN_REMOVE_SPACE: &str = "brain_remove_space";

/// `brain_spaces` — list registered spaces in a session (multimodal).
pub const TOOL_BRAIN_SPACES: &str = "brain_spaces";

/// `docs_ingest` — ingest Markdown / ADR files (multimodal).
pub const TOOL_DOCS_INGEST: &str = "docs_ingest";

/// `graph_search` — FTS5-backed search across the graph_nodes table (multimodal).
pub const TOOL_GRAPH_SEARCH: &str = "graph_search";

/// `issues_ingest` — ingest GitHub issues from a repository (multimodal).
pub const TOOL_ISSUES_INGEST: &str = "issues_ingest";

/// `lens_find_dead_code` — symbols not reachable from any entry point.
pub const TOOL_FIND_DEAD_CODE: &str = "lens_find_dead_code";

/// `lens_find_intersection` — findings shared across multiple lenses
/// applied to the same object.
pub const TOOL_FIND_INTERSECTION: &str = "lens_find_intersection";

/// `lens_hotspots` — top-N symbols by PageRank across the full graph,
/// relative to an anchoring object.
pub const TOOL_HOTSPOTS: &str = "lens_hotspots";

/// `find_dead_code_v2` — workspace-wide dead-code analysis with confidence filter
/// (wraps internal MCP `find_dead_code` logic via CallGraph).
pub const TOOL_FIND_DEAD_CODE_V2: &str = "find_dead_code_v2";

/// `find_cycles` — detect all strongly-connected components (cycles) in the call graph
/// (wraps `CycleDetector` from cognicode-graph-algos).
pub const TOOL_FIND_CYCLES: &str = "find_cycles";

/// `health_dashboard` — single-call workspace health summary with findings
/// (derives health score from graph metrics).
pub const TOOL_HEALTH_DASHBOARD: &str = "health_dashboard";

/// `find_quality_issues` — workspace-wide quality findings with filters
/// (severity, category, file prefix, status). Wraps the
/// `QualityRepository` port.
pub const TOOL_FIND_QUALITY_ISSUES: &str = "find_quality_issues";

/// `quality_gate` — single-shot snapshot of the workspace quality gate
/// (rating, total issues, blockers, criticals, debt_minutes, last_run).
/// Wraps `QualityRepository::quality_gate()`.
pub const TOOL_QUALITY_GATE: &str = "quality_gate";

/// `build_context` — consolidates object inspection + lens findings +
/// quality issues + graph neighbors into a context blob for LLM
/// agent consumption. Returns both Markdown and JSON representations.
pub const TOOL_BUILD_CONTEXT: &str = "build_context";

// ============================================================================
// Result envelope types
// ============================================================================

/// Metadata about the source of a result (e.g. `"ask-router"`, `"brain-session"`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceMetadata {
    /// Classification / confidence score. `None` when unavailable.
    pub confidence: Option<f64>,
    /// Human-readable name of the subsystem that produced the result.
    pub source: Option<String>,
}

impl ProvenanceMetadata {
    /// Construct a new provenance entry.
    pub fn new(confidence: f64, source: Option<String>) -> Option<Self> {
        Some(Self {
            confidence: Some(confidence),
            source,
        })
    }
}

impl Default for ProvenanceMetadata {
    fn default() -> Self {
        Self {
            confidence: None,
            source: None,
        }
    }
}

/// A suggested follow-up question or action surfaced by a tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowUp {
    /// Tool name to call for this follow-up.
    pub tool: String,
    /// Human-readable reason why this follow-up is suggested.
    pub reason: String,
    /// Optional kind label (e.g. `"related_inverse"`, `"hint"`, `"inspect"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// Structured envelope returned by every tool in the MCP surface.
///
/// All 34 tools (base + multimodal) return the same JSON shape so clients
/// can parse one schema across the entire surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResultEnvelope<T = Value> {
    /// Canonical tool name that produced this result.
    pub tool_name: String,
    /// Version of the package at envelope construction time.
    pub version: String,
    /// RFC 3339 timestamp at construction time.
    pub timestamp: String,
    /// Provenance metadata (e.g. `"ask-router"`, `"brain-session"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<ProvenanceMetadata>,
    /// Tool-specific result payload.
    pub payload: T,
    /// Suggested follow-up questions / actions.
    #[serde(default)]
    pub suggested_follow_ups: Vec<FollowUp>,
}

/// Error reported inside an envelope payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvelopeError {
    pub error_code: String,
    pub error: String,
}

// ============================================================================
// ExplorerMcpHandler
// ============================================================================

/// Top-level MCP handler for the Explorer.
///
/// Owns a [`ToolHandlerRegistry`] and an [`McpContext`]. Dispatches
/// `tools/list` and `tools/call` requests by delegating to the registry.
#[derive(Clone)]
pub struct ExplorerMcpHandler {
    registry: Arc<ToolHandlerRegistry>,
    ctx: Arc<McpContext>,
}

impl ExplorerMcpHandler {
    /// Construct a new handler from a pre-built registry and context.
    pub fn new(registry: ToolHandlerRegistry, ctx: McpContext) -> Self {
        Self {
            registry: Arc::new(registry),
            ctx: Arc::new(ctx),
        }
    }

    /// Build an `ExplorerMcpHandler` from raw infrastructure components.
    ///
    /// This constructor wires all 6 ISP-segregated facades into the
    /// [`McpContext`] and populates the [`ToolHandlerRegistry`] with
    /// all registered tool families. Used by the MCP binary bootstrap.
    ///
    /// # Parameters
    ///
    /// * `symbol_repo` — Symbol resolution port.
    /// * `source_reader` — Source file read port.
    /// * `quality_repo` — Optional `QualityRepository` port backing the
///   quality-MCP tools (`find_quality_issues`, `quality_gate`) and the
///   internal lenses that surface quality findings. Wired from
///   `cognicode-runtime` via `PostgresQualityRepository` (PG-canonical).
#[allow(clippy::too_many_arguments)]
    pub fn with_graph(
        symbol_repo: Arc<dyn SymbolRepository>,
        source_reader: Arc<dyn SourceReader>,
        view_registry: Arc<crate::registry::ViewRegistry>,
        lens_registry: crate::domain::lens::LensRegistry,
        cwd: PathBuf,
        graph: Option<Arc<cognicode_core::domain::aggregates::CallGraph>>,
        quality_repo: Option<Arc<dyn crate::ports::QualityRepository>>,
    ) -> Self {
        // GraphQueryPort may be None when no call graph is loaded.
        let graph_query: Option<Arc<dyn GraphQueryPort>> = graph.as_ref().map(|g| {
            Arc::new(crate::adapters::CallGraphRepository::new(g.clone()))
                as Arc<dyn GraphQueryPort>
        });

        // Workspace facade.
        let workspace: Arc<dyn WorkspaceService> =
            Arc::new(WorkspaceServiceImpl::new(symbol_repo.clone(), cwd));

        // Search facade.
        let search: Arc<dyn SearchService> = Arc::new(SearchServiceImpl::new(
            symbol_repo.clone(),
            None, // search_repo
            view_registry.clone(),
            None, // view_spec_store
            quality_repo.clone(),
        ));

        // View facade (also provides LensExecutor for MoldQL).
        let view_impl: Arc<ViewServiceImpl> = Arc::new(ViewServiceImpl::new(
            symbol_repo.clone(),
            source_reader.clone(),
            quality_repo.clone(),
            lens_registry,
            graph_query.clone(),
            view_registry.clone(),
        ));
        let view: Arc<dyn ViewService> = view_impl.clone();
        let lens_executor: Arc<dyn LensExecutor> = view_impl;

        // Persistence facade.
        #[cfg(feature = "postgres")]
        let persistence: Arc<dyn PersistenceService> = Arc::new(PersistenceServiceImpl::new(
            None, // view_spec_store
            None, // postgres_repo
        ));
        #[cfg(not(feature = "postgres"))]
        let persistence: Arc<dyn PersistenceService> = Arc::new(PersistenceServiceImpl::new(
            None, // view_spec_store
        ));

        // MoldQL facade.
        let moldql: Arc<dyn MoldQLService> = Arc::new(MoldQLServiceImpl::new(
            symbol_repo.clone(),
            quality_repo.clone(),
            source_reader,
            lens_executor,
            #[cfg(feature = "multimodal")]
            None, // graph_repo
        ));

        // Graph facade.
        let graph_facade: Arc<dyn GraphService> =
            Arc::new(GraphServiceImpl::new(symbol_repo.clone(), graph_query));

        // Build McpContext with all facades wired.
        let mut ctx_builder = McpContext::builder()
            .with_graph(graph)
            .with_session_registry(SessionRegistry::new())
            .with_workspace(workspace.clone())
            .with_search(search.clone())
            .with_view(view.clone())
            .with_moldql(moldql.clone())
            .with_persistence(persistence)
            .with_graph_service(graph_facade.clone());
        if let Some(q) = quality_repo {
            ctx_builder = ctx_builder.with_quality(q);
        }
        let ctx = ctx_builder.build();

        // Build registry and register all handlers.
        let mut registry = ToolHandlerRegistry::new();
        crate::mcp::handler::register_ask_handlers(&mut registry);
        crate::mcp::handler::register_context_builder_handlers(&mut registry);
        crate::mcp::handler::register_drift_handlers(&mut registry);
        crate::mcp::handler::register_graph_handlers(&mut registry);
        crate::mcp::handler::register_graph_analyze_handlers(&mut registry);
        crate::mcp::handler::register_impact_handlers(&mut registry);
        crate::mcp::handler::register_ingest_handlers(&mut registry);
        crate::mcp::handler::register_internal_mcp_handlers(&mut registry);
        crate::mcp::handler::register_lens_mcp_handlers(&mut registry);
        crate::mcp::handler::register_named_views_handlers(&mut registry);
        crate::mcp::handler::register_quality_mcp_handlers(&mut registry);
        crate::mcp::handler::register_search_handlers(&mut registry);
        crate::mcp::handler::register_session_handlers(&mut registry);
        crate::mcp::handler::register_view_handlers(&mut registry);
        crate::mcp::handler::register_workspace_handlers(&mut registry);

        Self::new(registry, ctx)
    }

    /// Build the complete tool list for `tools/list`.
    ///
    /// Returns all registered tools as RMCP [`Tool`](rmcp::model::Tool) objects.
    pub fn tools_list(&self) -> Vec<Tool> {
        use std::borrow::Cow;
        self.registry
            .list()
            .into_iter()
            .map(|h| {
                let schema: serde_json::Value = h.arg_schema();
                let obj = schema.as_object().cloned().unwrap_or_default();
                Tool::new(
                    Cow::Borrowed(h.name()),
                    Cow::Owned(format!("Registered tool: {}", h.name())),
                    Arc::new(obj),
                )
            })
            .collect()
    }

    /// Dispatch a `tools/call` request.
    ///
    /// Looks up the handler by name and forwards the call. Returns a
    /// structured error envelope for unknown tools.
    pub async fn tools_call(&self, name: &str, params: Value) -> CallToolResult {
        self.registry.dispatch(name, &self.ctx, params).await
    }
}

impl ServerHandler for ExplorerMcpHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "cognicode-explorer",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_protocol_version(ProtocolVersion::V_2025_03_26)
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, ErrorData>> + Send + '_ {
        async move {
            Ok(ListToolsResult {
                meta: None,
                tools: self.tools_list(),
                next_cursor: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, ErrorData>> + Send + '_ {
        let name = request.name.clone();
        let arguments = match request.arguments {
            Some(map) => serde_json::Value::Object(map),
            None => serde_json::Value::Object(Default::default()),
        };
        async move { Ok(self.tools_call(&name, arguments).await) }
    }
}

/// Return the sorted list of all tool names known to the base build
/// (excludes multimodal-only tools).
pub fn tool_names() -> Vec<&'static str> {
    vec![
        TOOL_OPEN_WORKSPACE,
        TOOL_SPOTTER_SEARCH,
        TOOL_INSPECT_OBJECT,
        TOOL_GET_VIEWS,
        TOOL_GET_VIEW,
        TOOL_GET_LENSES,
        TOOL_APPLY_LENS,
        TOOL_QUERY_MOLDQL,
        TOOL_IMPACT_RADIUS,
        TOOL_IMPACT_FORWARD_RADIUS,
        TOOL_IMPACT_HAS_PATH,
        TOOL_IMPACT_SHORTEST_PATH,
        TOOL_IMPACT_DETECT_CYCLES,
        TOOL_IMPACT_COMPONENT,
        TOOL_GRAPH_SUBGRAPH,
        TOOL_GRAPH_CLUSTER,
        TOOL_GRAPH_EXPLAIN,
        TOOL_DETECT_ARCHITECTURE_DRIFT,
        TOOL_ASK,
        TOOL_BRAIN_OPEN,
        TOOL_BRAIN_ATTACH,
        TOOL_BRAIN_ASK,
        TOOL_BRAIN_FOCUS,
        TOOL_BRAIN_STATUS,
        TOOL_BRAIN_CLOSE,
        TOOL_VIEW_SAVE,
        TOOL_VIEW_LOAD,
        TOOL_VIEW_LIST,
        TOOL_VIEW_DELETE,
    ]
}

/// Sorted list of all tool names for the base build (excludes multimodal-only tools).
/// Mirrors [`tool_names()`](tool_names) as a static slice for cases that need a const.
pub const TOOL_NAMES: &[&str] = &[
    TOOL_OPEN_WORKSPACE,
    TOOL_SPOTTER_SEARCH,
    TOOL_INSPECT_OBJECT,
    TOOL_GET_VIEWS,
    TOOL_GET_VIEW,
    TOOL_GET_LENSES,
    TOOL_APPLY_LENS,
    TOOL_QUERY_MOLDQL,
    TOOL_IMPACT_RADIUS,
    TOOL_IMPACT_FORWARD_RADIUS,
    TOOL_IMPACT_HAS_PATH,
    TOOL_IMPACT_SHORTEST_PATH,
    TOOL_IMPACT_DETECT_CYCLES,
    TOOL_IMPACT_COMPONENT,
    TOOL_GRAPH_SUBGRAPH,
    TOOL_GRAPH_CLUSTER,
    TOOL_GRAPH_EXPLAIN,
    TOOL_GRAPH_PAGERANK,
    TOOL_GRAPH_GOD_NODES,
    TOOL_GRAPH_COMMUNITIES,
    TOOL_GRAPH_COMMUNITY_GOD_NODES,
    TOOL_GRAPH_SURPRISING_CONNECTIONS,
    TOOL_GRAPH_TRANSITIVE_REDUCTION,
    TOOL_GRAPH_FEEDBACK_ARC_SET,
    TOOL_GRAPH_ALL_SIMPLE_PATHS,
    TOOL_HEALTH_DASHBOARD,
    TOOL_DETECT_ARCHITECTURE_DRIFT,
    TOOL_ASK,
    TOOL_BRAIN_OPEN,
    TOOL_BRAIN_ATTACH,
    TOOL_BRAIN_ASK,
    TOOL_BRAIN_FOCUS,
    TOOL_BRAIN_STATUS,
    TOOL_BRAIN_CLOSE,
    TOOL_VIEW_SAVE,
    TOOL_VIEW_LOAD,
    TOOL_VIEW_LIST,
    TOOL_VIEW_DELETE,
    TOOL_FIND_CYCLES,
    TOOL_FIND_DEAD_CODE,
    TOOL_FIND_DEAD_CODE_V2,
    TOOL_FIND_INTERSECTION,
    TOOL_HOTSPOTS,
    TOOL_FIND_QUALITY_ISSUES,
    TOOL_QUALITY_GATE,
    TOOL_BUILD_CONTEXT,
];

/// Names of tools that are only available with the `multimodal` feature.
#[cfg(feature = "multimodal")]
pub const TOOL_NAMES_MULTIMODAL: &[&str] = &[
    TOOL_BRAIN_ADD_SPACE,
    TOOL_BRAIN_REMOVE_SPACE,
    TOOL_BRAIN_SPACES,
    TOOL_DOCS_INGEST,
    TOOL_GRAPH_SEARCH,
    TOOL_ISSUES_INGEST,
];

/// All tool names for the current build variant.
#[cfg(feature = "multimodal")]
pub fn all_tool_names() -> Vec<&'static str> {
    tool_names()
        .into_iter()
        .chain(TOOL_NAMES_MULTIMODAL.iter().copied())
        .collect()
}
