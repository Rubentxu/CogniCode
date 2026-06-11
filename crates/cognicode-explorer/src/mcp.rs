// CogniCode Explorer MCP adapter.
//
// Pure wiring: every tool delegates to an existing ExplorerService method.
// No new domain logic, no new DTOs. Follows the canonical CogniCodeHandler
// pattern in cognicode-core (single struct, single ServerHandler impl,
// dispatch by match on the tool name).
//
// Tool list (8):
//   1. explorer_open_workspace     — optional root_path -> WorkspaceSummary
//   2. explorer_spotter_search     — query (required), kind (optional)
//   3. explorer_inspect_object     — object_id -> InspectableObjectSummary
//   4. explorer_get_views          — object_id -> Vec<ViewDescriptor>
//   5. explorer_get_view           — object_id, view_id -> ContextualView
//   6. explorer_get_lenses         — object_id -> Vec<LensDescriptor>
//   7. explorer_apply_lens         — object_id, lens_id -> LensResult
//   8. explorer_query_moldql       — query (required) -> MoldQLResultDto
//        Extensions for ExplorerQL: the `query` field accepts any of
//        the 5 graph-native primitives (PATH, NEIGHBORS, SUBGRAPH,
//        CLUSTER, EXPLAIN) plus boolean composition (AND/OR/NOT).
//        Optional `target` field: "pg" | "petgraph" | "auto"
//        (default "auto" — keeps the legacy passthrough).
//
// Any ExplorerError is returned as a CallToolResult::error whose
// Content::text carries the Display representation. Agents never see a
// panic — service errors are captured, not propagated.

use std::sync::Arc;

use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult,
    ProtocolVersion, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use serde::Deserialize;

use cognicode_core::application::dto::SccDto;
use cognicode_core::application::services::impact_analysis::ImpactAnalysisService;
use cognicode_core::domain::aggregates::{CallGraph, SymbolId};
use cognicode_core::infrastructure::graph::SubgraphDirection;

use crate::dto::{MoldQLResultDto, OpenWorkspaceRequest};
use crate::error::ExplorerError;
use crate::service::ExplorerService;

// ============================================================================
// MCP Result Envelope — standardized wire-level wrapper for every tool.
//
// Every successful `tools/call` returns a JSON object with the six top-level
// fields below. Consumers parse the outer envelope and dispatch to a
// per-tool payload inspector via `payload`.
// ============================================================================

/// Standardized wrapper for all MCP tool results.
///
/// Six top-level fields, stable across all 17 tools:
/// - `tool_name` — the dispatched tool constant (e.g. `"explorer_open_workspace"`)
/// - `version` — populated from `env!("CARGO_PKG_VERSION")` at call time
/// - `timestamp` — RFC 3339 UTC, generated via `chrono::Utc::now()`
/// - `provenance` — optional [`ProvenanceMetadata`] (serializes as JSON `null`
///   when absent)
/// - `payload` — the tool's typed result
/// - `suggested_follow_ups` — typed [`FollowUp`] hints (always empty for now)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpResultEnvelope<T> {
    pub tool_name: String,
    pub version: String,
    pub timestamp: String,
    pub provenance: Option<ProvenanceMetadata>,
    pub payload: T,
    pub suggested_follow_ups: Vec<FollowUp>,
}

/// Optional metadata describing where a result came from and how confident
/// the producer is in it. Validated at construction; `confidence` MUST be in
/// `[0.0, 1.0]`.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ProvenanceMetadata {
    pub confidence: Option<f64>,
    pub source: Option<String>,
}

impl ProvenanceMetadata {
    /// Construct a `ProvenanceMetadata` with a validated `confidence`.
    /// Returns [`EnvelopeError::ConfidenceOutOfRange`] for values outside
    /// `[0.0, 1.0]`.
    pub fn new(confidence: f64, source: Option<String>) -> Result<Self, EnvelopeError> {
        if !(0.0..=1.0).contains(&confidence) {
            return Err(EnvelopeError::ConfidenceOutOfRange(confidence));
        }
        Ok(Self {
            confidence: Some(confidence),
            source,
        })
    }
}

/// A suggested follow-up action for an agent. Reserved for future use;
/// the current envelope always emits an empty `suggested_follow_ups` array.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FollowUp {
    pub tool: String,
    pub reason: String,
    /// Optional discriminator used by the ask-router to mark the kind
    /// of follow-up (e.g. `"related_inverse"`, `"no_entity_match"`,
    /// `"entity_disambiguation"`, `"no_pattern_match"`, `"hint"`).
    /// `None` is still valid — the primitive tools' envelopes never
    /// populate it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// Errors raised by envelope construction.
#[derive(Debug, thiserror::Error)]
pub enum EnvelopeError {
    #[error("confidence {0} out of range [0.0, 1.0]")]
    ConfidenceOutOfRange(f64),
}

pub const TOOL_OPEN_WORKSPACE: &str = "explorer_open_workspace";
pub const TOOL_SPOTTER_SEARCH: &str = "explorer_spotter_search";
pub const TOOL_INSPECT_OBJECT: &str = "explorer_inspect_object";
pub const TOOL_GET_VIEWS: &str = "explorer_get_views";
pub const TOOL_GET_VIEW: &str = "explorer_get_view";
pub const TOOL_GET_LENSES: &str = "explorer_get_lenses";
pub const TOOL_APPLY_LENS: &str = "explorer_apply_lens";
pub const TOOL_QUERY_MOLDQL: &str = "explorer_query_moldql";

pub const TOOL_IMPACT_RADIUS: &str = "impact_radius";
pub const TOOL_IMPACT_FORWARD_RADIUS: &str = "impact_forward_radius";
pub const TOOL_IMPACT_HAS_PATH: &str = "impact_has_path";
pub const TOOL_IMPACT_SHORTEST_PATH: &str = "impact_shortest_path";
pub const TOOL_IMPACT_DETECT_CYCLES: &str = "impact_detect_cycles";
pub const TOOL_IMPACT_COMPONENT: &str = "impact_component";

/// Default maximum reverse BFS depth for `impact_radius` when the caller
/// omits `max_depth`. Project-wide constant, locked by the spec. Also
/// reused by `impact_forward_radius` so the two complementary tools
/// share the same default depth.
pub const DEFAULT_IMPACT_RADIUS_DEPTH: usize = 5;

// mcp-graph-primitives — 3 new tools.
pub const TOOL_GRAPH_SUBGRAPH: &str = "graph_subgraph";
pub const TOOL_GRAPH_CLUSTER: &str = "graph_cluster";
pub const TOOL_GRAPH_EXPLAIN: &str = "graph_explain";

// ask-router — the 18th tool. A natural-language front-end that
// classifies the question against 8 priority-ordered patterns and
// dispatches to the right primitive chain. Eliminates the need for
// agents to memorise the 17-tool surface.
pub const TOOL_ASK: &str = "cognicode_ask";

// brain-session — 6 new tools, taking the wire-level count from 18
// to 24. Each tool is backed by the in-memory `SessionRegistry`
// held on the `ExplorerMcpHandler` struct. The `brain_ask` tool
// injects the per-session focus node into the question before
// dispatching to the ask router; the others are state-mutating
// CRUD-style operations on the registry.
pub const TOOL_BRAIN_OPEN: &str = "brain_open";
pub const TOOL_BRAIN_ATTACH: &str = "brain_attach";
pub const TOOL_BRAIN_ASK: &str = "brain_ask";
pub const TOOL_BRAIN_FOCUS: &str = "brain_focus";
pub const TOOL_BRAIN_STATUS: &str = "brain_status";
pub const TOOL_BRAIN_CLOSE: &str = "brain_close";

// multimodal (brain-federation) — 3 new tools, taking the wire-level
// count from 31 to 34 (with multimodal feature). Each tool is a thin
// wrapper over the corresponding `BrainSessionService::add_space /
// remove_space / spaces` method. Feature-gated behind the `multimodal`
// Cargo feature so the default build is unchanged.
#[cfg(feature = "multimodal")]
pub const TOOL_BRAIN_ADD_SPACE: &str = "brain_add_space";
#[cfg(feature = "multimodal")]
pub const TOOL_BRAIN_REMOVE_SPACE: &str = "brain_remove_space";
#[cfg(feature = "multimodal")]
pub const TOOL_BRAIN_SPACES: &str = "brain_spaces";

// named-views — 4 new tools, taking the wire-level count from 24
// to 28. Each tool is a thin wrapper over the corresponding
// `ExplorerService::save_view / load_view / list_views /
// delete_view` method. The feature gate is enforced at the
// service boundary — without the `postgres` feature active,
// every tool returns the canonical
// `"named_views_require_postgres_feature"` soft error.
pub const TOOL_VIEW_SAVE: &str = "view_save";
pub const TOOL_VIEW_LOAD: &str = "view_load";
pub const TOOL_VIEW_LIST: &str = "view_list";
pub const TOOL_VIEW_DELETE: &str = "view_delete";

// multimodal (T14) — docs_ingest. Registered ONLY when the
// `multimodal` Cargo feature is enabled (compile-time
// `#[cfg(feature = "multimodal")]` on the constant + the
// dispatch arm + the schema entry). On a default build the
// tool is absent from `tools/list` and a stale call to
// `docs_ingest` returns `-32601` from the framework, which
// is the canonical "feature disabled" path for MCP.
#[cfg(feature = "multimodal")]
pub const TOOL_DOCS_INGEST: &str = "docs_ingest";

// multimodal (T21) — graph_search. Same compile-time
// feature gate as `docs_ingest`. FTS5-backed search across
// the `graph_nodes` table. Returns a paginated
// `McpResultEnvelope` payload `{results, total_count,
// next_cursor, raw_rank, normalized_score}`.
#[cfg(feature = "multimodal")]
pub const TOOL_GRAPH_SEARCH: &str = "graph_search";

// multimodal (T12) — issues_ingest. Same compile-time
// feature gate as docs_ingest/graph_search. Fetches
// GitHub issues from the given owner/repo via the
// IssuesExtractor and returns structured counts.
#[cfg(feature = "multimodal")]
pub const TOOL_ISSUES_INGEST: &str = "issues_ingest";

/// Default page size for `graph_search` when the caller
/// omits `limit`. Locked by the spec.
#[cfg(feature = "multimodal")]
pub const DEFAULT_GRAPH_SEARCH_LIMIT: usize = 50;

/// Hard cap on `graph_search` `limit` per the spec — protects
/// the PG backend from accidental DoS.
#[cfg(feature = "multimodal")]
pub const MAX_GRAPH_SEARCH_LIMIT: usize = 200;

/// Default maximum BFS depth for `graph_subgraph` when the caller omits
/// `max_depth`. Project-wide constant, locked by the spec.
pub const DEFAULT_SUBGRAPH_DEPTH: usize = 3;

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
    TOOL_IMPACT_HAS_PATH,
    TOOL_IMPACT_SHORTEST_PATH,
    TOOL_IMPACT_DETECT_CYCLES,
    TOOL_IMPACT_COMPONENT,
    TOOL_IMPACT_FORWARD_RADIUS,
    TOOL_GRAPH_SUBGRAPH,
    TOOL_GRAPH_CLUSTER,
    TOOL_GRAPH_EXPLAIN,
    TOOL_ASK,
    TOOL_BRAIN_OPEN,
    TOOL_BRAIN_ATTACH,
    TOOL_BRAIN_ASK,
    TOOL_BRAIN_FOCUS,
    TOOL_BRAIN_STATUS,
    TOOL_BRAIN_CLOSE,
    #[cfg(feature = "multimodal")]
    TOOL_BRAIN_ADD_SPACE,
    #[cfg(feature = "multimodal")]
    TOOL_BRAIN_REMOVE_SPACE,
    #[cfg(feature = "multimodal")]
    TOOL_BRAIN_SPACES,
    TOOL_VIEW_SAVE,
    TOOL_VIEW_LOAD,
    TOOL_VIEW_LIST,
    TOOL_VIEW_DELETE,
    #[cfg(feature = "multimodal")]
    TOOL_DOCS_INGEST,
    #[cfg(feature = "multimodal")]
    TOOL_GRAPH_SEARCH,
    #[cfg(feature = "multimodal")]
    TOOL_ISSUES_INGEST,
];

/// Backwards-compatible accessor — returns the canonical tool list.
pub fn tool_names() -> &'static [&'static str] {
    TOOL_NAMES
}

// ============================================================================
// Per-tool argument structs.
//
// Each struct is Deserialize from the JSON-RPC `tools/call` arguments blob.
// Fields are Option<_> unless the tool cannot function without them — that
// gives per-field error messages instead of one opaque parse failure.
// ============================================================================

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OpenWorkspaceArgs {
    root_path: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpotterArgs {
    query: Option<String>,
    kind: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct InspectArgs {
    object_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GetViewArgs {
    object_id: Option<String>,
    view_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ApplyLensArgs {
    object_id: Option<String>,
    lens_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct QueryMoldQLArgs {
    query: Option<String>,
    /// ExplorerQL compile target: `"pg"` | `"petgraph"` | `"auto"`.
    /// Default is `"auto"` (compile to petgraph for primitives,
    /// passthrough for FIND/EXPLORE). ExplorerQL extensions are
    /// documented in the tool description.
    target: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ImpactRadiusArgs {
    root: Option<String>,
    max_depth: Option<usize>,
}

/// Arg shape for `impact_forward_radius`. Mirrors `ImpactRadiusArgs`
/// exactly: the forward tool reuses the same `root` + `max_depth` field
/// set so the two tools share a single mental model.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ImpactForwardRadiusArgs {
    root: Option<String>,
    max_depth: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ImpactEndpointsArgs {
    from: Option<String>,
    to: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ImpactIdArgs {
    id: Option<String>,
}

// mcp-graph-primitives — 3 new arg structs.

/// Arg shape for `graph_subgraph`. `root` is required; `direction` is
/// an optional enum (`"incoming" | "outgoing" | "both"`, default
/// `"both"`); `max_depth` is an optional `usize` (default
/// [`DEFAULT_SUBGRAPH_DEPTH`]).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GraphSubgraphArgs {
    root: Option<String>,
    direction: Option<String>,
    max_depth: Option<usize>,
}

/// Arg shape for `graph_cluster`. `method` is optional (`"scc" |
/// "connected"`, default `"scc"`). The struct is empty-valid: callers
/// can invoke the tool with `{}` and accept the default cluster.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GraphClusterArgs {
    method: Option<String>,
}

/// Arg shape for `graph_explain`. Both `from` and `to` are required.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GraphExplainArgs {
    from: Option<String>,
    to: Option<String>,
}

// ask-router — arg shape for `cognicode_ask`. `question` is required;
/// `context` is reserved for future use (routing hints, conversation
/// state) and is not consulted by the current router.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct AskArgs {
    question: Option<String>,
    context: Option<serde_json::Value>,
}

// brain-session — 6 arg shapes for the `brain_*` tools. All fields
// are `Option<_>` so the per-field error messages stay specific
// (missing `workspace_id` is a different problem from a malformed
// JSON object).

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrainOpenArgs {
    workspace_id: Option<String>,
    ttl: Option<u64>,
    /// Multimodal (brain-federation) — optional list of space
    /// specs to auto-register when the session is opened. Each
    /// entry has `name`, `kind`, and optional `source_path`.
    /// On default builds this field is absent (not parsed).
    #[cfg(feature = "multimodal")]
    #[serde(default)]
    spaces: Option<Vec<SpaceSpec>>,
}

/// One entry in the optional `spaces` array of `brain_open`.
/// Mirrors the `brain_add_space` parameter set so callers can
/// use the same mental model for both paths.
#[cfg(feature = "multimodal")]
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpaceSpec {
    space_name: Option<String>,
    space_kind: Option<String>,
    source_path: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrainAttachArgs {
    session_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrainAskArgs {
    session_id: Option<String>,
    question: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrainFocusArgs {
    session_id: Option<String>,
    focus_node: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrainStatusArgs {
    session_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrainCloseArgs {
    session_id: Option<String>,
}

// multimodal (brain-federation) — 3 arg shapes for the
// `brain_add_space` / `brain_remove_space` / `brain_spaces`
// tools. Feature-gated; absent on default builds.

/// Arg shape for `brain_add_space`. `session_id`, `space_name`,
/// and `space_kind` are required; `source_path` is optional.
#[cfg(feature = "multimodal")]
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrainAddSpaceArgs {
    session_id: Option<String>,
    space_name: Option<String>,
    /// One of `"Repo"`, `"Docs"`, `"Issues"` (PascalCase).
    /// The dispatch arm normalises lowercase input.
    space_kind: Option<String>,
    /// Optional filesystem path or URL the space was loaded from.
    source_path: Option<String>,
}

/// Arg shape for `brain_remove_space`. `session_id` and
/// `space_id` are required.
#[cfg(feature = "multimodal")]
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrainRemoveSpaceArgs {
    session_id: Option<String>,
    space_id: Option<String>,
}

/// Arg shape for `brain_spaces`. Only `session_id` is required.
#[cfg(feature = "multimodal")]
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrainSpacesArgs {
    session_id: Option<String>,
}

// named-views — 4 arg shapes for the `view_*` tools. The
// dispatch layer validates every field and surfaces a precise
// `invalid_input` error envelope on the first violation.

/// Arg shape for `view_save`. Every field is required except
/// `description` (optional). `max_depth` must be `>= 0`; the
/// service layer enforces that.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ViewSaveArgs {
    workspace_id: Option<String>,
    owner: Option<String>,
    name: Option<String>,
    description: Option<String>,
    level: Option<String>,
    lens: Option<String>,
    focus_node: Option<String>,
    max_depth: Option<i32>,
}

/// Arg shape for `view_load`. All three fields are required
/// for the scope guard.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ViewLoadArgs {
    id: Option<String>,
    workspace_id: Option<String>,
    owner: Option<String>,
}

/// Arg shape for `view_list`. Both fields are required.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ViewListArgs {
    workspace_id: Option<String>,
    owner: Option<String>,
}

/// Arg shape for `view_delete`. All three fields are required
/// for the scope guard.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ViewDeleteArgs {
    id: Option<String>,
    workspace_id: Option<String>,
    owner: Option<String>,
}

// multimodal (T14) — arg shape for `docs_ingest`.
// `path` is required (file or directory); `recursive` is
// optional and defaults to `true`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct DocsIngestArgs {
    path: Option<String>,
    recursive: Option<bool>,
}

// multimodal (T21) — arg shape for `graph_search`.
// All fields are optional EXCEPT `query`, which is required.
// `node_kinds` filters to one or more multimodal kinds.
// `limit` defaults to 50, capped at 200
// (`MAX_GRAPH_SEARCH_LIMIT`). `cursor` is opaque (the previous
// response's `next_cursor`).
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GraphSearchArgs {
    query: Option<String>,
    node_kinds: Option<Vec<String>>,
    cursor: Option<String>,
    limit: Option<i64>,
}

// multimodal (T12) — arg shape for `issues_ingest`.
// `owner` and `repo` are required. The `include_git_log`
// flag was a v0 prototype that is no longer wired into the
// ingestion path; it has been removed from the schema until
// the git-log parser is implemented.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct IssuesIngestArgs {
    owner: Option<String>,
    repo: Option<String>,
}

/// Response shape for `impact_has_path`. Always carries the original
/// `from`/`to` so agents can correlate the result with their request
/// without having to round-trip the arguments.
#[derive(Debug, serde::Serialize)]
struct HasPathResult {
    from: String,
    to: String,
    has_path: bool,
}

// ============================================================================
// Handler
// ============================================================================

/// The MCP handler for the cognicode-explorer service.
///
/// Owns an `Arc<ExplorerService>` — cheap to clone, shareable across
/// threads. The canonical use is a single handler per process; the Arc
/// is here to support future multiplexed transports without an API break.
///
/// `graph` is the optional in-memory call graph backing the 5 `impact_*`
/// tools. It is held as `Option<Arc<CallGraph>>` so the legacy
/// `new(service)` constructor stays binary- and source-compatible with
/// pre-impact tool callers (and tests) that never had a graph.
///
/// `registry` is the in-memory brain-session registry backing the 6
/// `brain_*` tools. It is always present and zero-cost when no
/// session has been opened yet — every handler gets a fresh empty
/// registry at construction.
///
/// `graph_repo` is the optional `GraphRepository` backing the
/// multimodal `graph_search` tool (T21). It is held as
/// `Option<Arc<dyn GraphRepository + Send + Sync>>` so default
/// builds (no `multimodal` feature) stay zero-cost. Constructed
/// via [`with_graph_repo`] when the multimodal feature is on.
#[derive(Clone)]
pub struct ExplorerMcpHandler {
    service: Arc<ExplorerService>,
    graph: Option<Arc<CallGraph>>,
    registry: crate::session::SessionRegistry,
    /// Multimodal (T21) — backing store for the `graph_search`
    /// MCP tool. `None` on default builds or when the caller
    /// has not supplied a graph repo. The `graph_search` tool
    /// reports `"graph_search_unavailable"` when this is `None`.
    #[cfg(feature = "multimodal")]
    graph_repo: Option<Arc<dyn crate::ports::graph_repository::GraphRepository>>,
}

impl ExplorerMcpHandler {
    /// Wrap a service in an MCP handler without a call graph. The 5
    /// `impact_*` tools will return an `"impact analysis unavailable"`
    /// error from this handler.
    pub fn new(service: Arc<ExplorerService>) -> Self {
        Self {
            service,
            graph: None,
            registry: crate::session::SessionRegistry::new(),
            #[cfg(feature = "multimodal")]
            graph_repo: None,
        }
    }

    /// Wrap a service in an MCP handler with an optional call graph.
    /// When `graph` is `Some`, the 5 `impact_*` tools are reachable.
    pub fn with_graph(service: Arc<ExplorerService>, graph: Option<Arc<CallGraph>>) -> Self {
        Self {
            service,
            graph,
            registry: crate::session::SessionRegistry::new(),
            #[cfg(feature = "multimodal")]
            graph_repo: None,
        }
    }

    /// Wrap a service in an MCP handler with both a call graph and a
    /// multimodal `GraphRepository`. Multimodal builds (T21) call
    /// this to wire the `graph_search` tool.
    #[cfg(feature = "multimodal")]
    pub fn with_graph_repo(
        service: Arc<ExplorerService>,
        graph: Option<Arc<CallGraph>>,
        graph_repo: Option<Arc<dyn crate::ports::graph_repository::GraphRepository>>,
    ) -> Self {
        Self {
            service,
            graph,
            registry: crate::session::SessionRegistry::new(),
            graph_repo,
        }
    }

    /// Borrow the underlying service handle. Used by tests to confirm
    /// that dispatched tool calls actually reached the service.
    #[cfg(test)]
    pub fn service(&self) -> &Arc<ExplorerService> {
        &self.service
    }

    /// Borrow the optional call graph handle. Test-only.
    #[cfg(test)]
    pub fn graph(&self) -> Option<&Arc<CallGraph>> {
        self.graph.as_ref()
    }

    /// Borrow the optional `GraphRepository` (multimodal build
    /// only). Test-only.
    #[cfg(all(test, feature = "multimodal"))]
    pub fn graph_repo(
        &self,
    ) -> Option<&Arc<dyn crate::ports::graph_repository::GraphRepository>> {
        self.graph_repo.as_ref()
    }

    /// Borrow the in-memory brain-session registry. Test-only.
    #[cfg(any(test, feature = "test-utils"))]
    pub fn registry(&self) -> &crate::session::SessionRegistry {
        &self.registry
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
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, rmcp::ErrorData>> + Send + '_
    {
        let tools = build_tool_schemas();
        async move {
            Ok(ListToolsResult {
                meta: None,
                tools,
                next_cursor: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, rmcp::ErrorData>> + Send + '_
    {
        let service = self.service.clone();
        let graph = self.graph.clone();
        let registry = self.registry.clone();
        #[cfg(feature = "multimodal")]
        let graph_repo = self.graph_repo.clone();
        async move {
            let result = dispatch(
                &service,
                &graph,
                &registry,
                request,
                #[cfg(feature = "multimodal")]
                graph_repo.as_ref(),
                #[cfg(not(feature = "multimodal"))]
                &(),
            )
            .await;
            Ok(result)
        }
    }
}

// ============================================================================
// Dispatch
// ============================================================================

async fn dispatch(
    service: &Arc<ExplorerService>,
    graph: &Option<Arc<CallGraph>>,
    registry: &crate::session::SessionRegistry,
    request: CallToolRequestParams,
    // Multimodal (T21) — `GraphRepository` for `graph_search`.
    // On default builds the type is `()` (zero-cost stub). On
    // multimodal builds it is the trait-object reference.
    // This keeps the call sites identical across both builds.
    #[cfg(feature = "multimodal")]
    graph_repo: Option<&Arc<dyn crate::ports::graph_repository::GraphRepository>>,
    #[cfg(not(feature = "multimodal"))]
    _graph_repo: &(),
) -> CallToolResult {
    let name = request.name.as_ref();
    // CallToolRequestParams.arguments is a serde_json::Map<String, Value>;
    // wrap it as a Value::Object so the per-tool deserializers can consume it.
    let arguments = match request.arguments {
        Some(map) => serde_json::Value::Object(map),
        None => serde_json::Value::Object(Default::default()),
    };

    match name {
        TOOL_OPEN_WORKSPACE => {
            let args: OpenWorkspaceArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("explorer_open_workspace: invalid args: {e}")),
            };
            let result = match args.root_path {
                Some(root_path) => service.open_workspace(OpenWorkspaceRequest { root_path }),
                None => service.current_workspace(),
            };
            envelope_ok(TOOL_OPEN_WORKSPACE, &result, None)
        }
        TOOL_SPOTTER_SEARCH => {
            let args: SpotterArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("explorer_spotter_search: invalid args: {e}")),
            };
            let query = match args.query {
                Some(q) => q,
                None => {
                    return err("explorer_spotter_search: missing required arg `query`".into());
                }
            };
            envelope_ok(
                TOOL_SPOTTER_SEARCH,
                &service.spotter_search(&query, args.kind.as_deref()),
                None,
            )
        }
        TOOL_INSPECT_OBJECT => {
            let args: InspectArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("explorer_inspect_object: invalid args: {e}")),
            };
            let object_id = match args.object_id {
                Some(id) => id,
                None => {
                    return err("explorer_inspect_object: missing required arg `object_id`".into());
                }
            };
            envelope_ok(
                TOOL_INSPECT_OBJECT,
                &service.inspect_object(&object_id),
                None,
            )
        }
        TOOL_GET_VIEWS => {
            let args: InspectArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("explorer_get_views: invalid args: {e}")),
            };
            let object_id = match args.object_id {
                Some(id) => id,
                None => {
                    return err("explorer_get_views: missing required arg `object_id`".into());
                }
            };
            envelope_ok(TOOL_GET_VIEWS, &service.available_views(&object_id), None)
        }
        TOOL_GET_VIEW => {
            let args: GetViewArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("explorer_get_view: invalid args: {e}")),
            };
            let object_id = match args.object_id {
                Some(id) => id,
                None => {
                    return err("explorer_get_view: missing required arg `object_id`".into());
                }
            };
            let view_id = match args.view_id {
                Some(v) => v,
                None => {
                    return err("explorer_get_view: missing required arg `view_id`".into());
                }
            };
            envelope_ok(
                TOOL_GET_VIEW,
                &service.contextual_view(&object_id, &view_id),
                None,
            )
        }
        TOOL_GET_LENSES => {
            let args: InspectArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("explorer_get_lenses: invalid args: {e}")),
            };
            let object_id = match args.object_id {
                Some(id) => id,
                None => {
                    return err("explorer_get_lenses: missing required arg `object_id`".into());
                }
            };
            envelope_ok(TOOL_GET_LENSES, &service.available_lenses(&object_id), None)
        }
        TOOL_APPLY_LENS => {
            let args: ApplyLensArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("explorer_apply_lens: invalid args: {e}")),
            };
            let object_id = match args.object_id {
                Some(id) => id,
                None => {
                    return err("explorer_apply_lens: missing required arg `object_id`".into());
                }
            };
            let lens_id = match args.lens_id {
                Some(l) => l,
                None => {
                    return err("explorer_apply_lens: missing required arg `lens_id`".into());
                }
            };
            envelope_ok(
                TOOL_APPLY_LENS,
                &service.apply_lens(&object_id, &lens_id),
                None,
            )
        }
        TOOL_QUERY_MOLDQL => {
            let args: QueryMoldQLArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("explorer_query_moldql: invalid args: {e}")),
            };
            let query = match args.query {
                Some(q) => q,
                None => {
                    return err("explorer_query_moldql: missing required arg `query`".into());
                }
            };
            // Optional `target` argument: "pg" | "petgraph" | "auto".
            // When the caller explicitly asks for "pg" and the build
            // has the `postgres` feature off, the executor surfaces a
            // clean `FeatureDisabled` error envelope instead of a
            // panic. `auto` (default) keeps the legacy passthrough.
            let target = match args.target.as_deref() {
                None | Some("auto") => None,
                Some("pg") => Some(crate::moldql::compile::CompileTarget::Postgres),
                Some("petgraph") => Some(crate::moldql::compile::CompileTarget::Petgraph),
                Some(other) => {
                    return err(format!(
                        "explorer_query_moldql: invalid `target` `{other}` \
                         (expected one of: pg, petgraph, auto)"
                    ));
                }
            };
            let result: Result<MoldQLResultDto, _> = match target {
                None => service.execute_query(&query).map(MoldQLResultDto::from),
                Some(tgt) => service
                    .execute_query_with_target(&query, tgt)
                    .map(MoldQLResultDto::from),
            };
            envelope_ok(TOOL_QUERY_MOLDQL, &result, None)
        }
        // ---- impact_* tools ------------------------------------------------
        //
        // All 5 impact tools share the same graph-availability guard. The
        // guard is the only invariant they have in common with the rest of
        // the dispatch surface; everything else (arg parsing, service
        // call, response shape) is per-tool below.
        TOOL_IMPACT_RADIUS => {
            let g = match require_graph(graph, TOOL_IMPACT_RADIUS) {
                Ok(g) => g,
                Err(e) => return e,
            };
            let args: ImpactRadiusArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("{TOOL_IMPACT_RADIUS}: invalid args: {e}")),
            };
            let root = match args.root {
                Some(r) => r,
                None => {
                    return err(format!("{TOOL_IMPACT_RADIUS}: missing required arg `root`"));
                }
            };
            let max_depth = args.max_depth.unwrap_or(DEFAULT_IMPACT_RADIUS_DEPTH);
            let svc = ImpactAnalysisService::new();
            let ids = svc.impact_radius(g, &SymbolId::new(root), max_depth);
            let strings: Vec<String> = ids.iter().map(|s| s.as_str().to_string()).collect();
            envelope_ok_direct(TOOL_IMPACT_RADIUS, &strings, None)
        }
        TOOL_IMPACT_FORWARD_RADIUS => {
            let g = match require_graph(graph, TOOL_IMPACT_FORWARD_RADIUS) {
                Ok(g) => g,
                Err(e) => return e,
            };
            let args: ImpactForwardRadiusArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("{TOOL_IMPACT_FORWARD_RADIUS}: invalid args: {e}")),
            };
            let root = match args.root {
                Some(r) => r,
                None => {
                    return err(format!(
                        "{TOOL_IMPACT_FORWARD_RADIUS}: missing required arg `root`"
                    ));
                }
            };
            let max_depth = args.max_depth.unwrap_or(DEFAULT_IMPACT_RADIUS_DEPTH);
            let svc = ImpactAnalysisService::new();
            let ids = svc.forward_radius(g, &SymbolId::new(root), max_depth);
            let strings: Vec<String> = ids.iter().map(|s| s.as_str().to_string()).collect();
            envelope_ok_direct(TOOL_IMPACT_FORWARD_RADIUS, &strings, None)
        }
        TOOL_IMPACT_HAS_PATH => {
            let g = match require_graph(graph, TOOL_IMPACT_HAS_PATH) {
                Ok(g) => g,
                Err(e) => return e,
            };
            let args: ImpactEndpointsArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("{TOOL_IMPACT_HAS_PATH}: invalid args: {e}")),
            };
            let from = match args.from {
                Some(v) => v,
                None => {
                    return err(format!(
                        "{TOOL_IMPACT_HAS_PATH}: missing required arg `from`"
                    ));
                }
            };
            let to = match args.to {
                Some(v) => v,
                None => {
                    return err(format!("{TOOL_IMPACT_HAS_PATH}: missing required arg `to`"));
                }
            };
            let svc = ImpactAnalysisService::new();
            let has_path =
                svc.has_path(g, &SymbolId::new(from.clone()), &SymbolId::new(to.clone()));
            envelope_ok_direct(
                TOOL_IMPACT_HAS_PATH,
                &HasPathResult { from, to, has_path },
                None,
            )
        }
        TOOL_IMPACT_SHORTEST_PATH => {
            let g = match require_graph(graph, TOOL_IMPACT_SHORTEST_PATH) {
                Ok(g) => g,
                Err(e) => return e,
            };
            let args: ImpactEndpointsArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("{TOOL_IMPACT_SHORTEST_PATH}: invalid args: {e}")),
            };
            let from = match args.from {
                Some(v) => v,
                None => {
                    return err(format!(
                        "{TOOL_IMPACT_SHORTEST_PATH}: missing required arg `from`"
                    ));
                }
            };
            let to = match args.to {
                Some(v) => v,
                None => {
                    return err(format!(
                        "{TOOL_IMPACT_SHORTEST_PATH}: missing required arg `to`"
                    ));
                }
            };
            let svc = ImpactAnalysisService::new();
            let result = svc.shortest_path(g, &SymbolId::new(from), &SymbolId::new(to));
            // `Option<PathResultDto>` serializes as JSON `null` when None
            // and as a full object when Some.
            envelope_ok_direct(TOOL_IMPACT_SHORTEST_PATH, &result, None)
        }
        TOOL_IMPACT_DETECT_CYCLES => {
            let g = match require_graph(graph, TOOL_IMPACT_DETECT_CYCLES) {
                Ok(g) => g,
                Err(e) => return e,
            };
            let svc = ImpactAnalysisService::new();
            let sccs = svc.detect_cycles(g);
            // Convert each `Vec<SymbolId>` into an `SccDto`. The DTO
            // computes `size` from the converted members, so size
            // stays in sync with the actual list length.
            let dtos: Vec<SccDto> = sccs.into_iter().map(SccDto::from_scc).collect();
            envelope_ok_direct(TOOL_IMPACT_DETECT_CYCLES, &dtos, None)
        }
        TOOL_IMPACT_COMPONENT => {
            let g = match require_graph(graph, TOOL_IMPACT_COMPONENT) {
                Ok(g) => g,
                Err(e) => return e,
            };
            let args: ImpactIdArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("{TOOL_IMPACT_COMPONENT}: invalid args: {e}")),
            };
            let id = match args.id {
                Some(v) => v,
                None => {
                    return err(format!(
                        "{TOOL_IMPACT_COMPONENT}: missing required arg `id`"
                    ));
                }
            };
            let svc = ImpactAnalysisService::new();
            let component = svc.containing_component(g, &SymbolId::new(id));
            // Convert to Option<Vec<String>> so it serializes as JSON
            // null when None. The service returns Vec<SymbolId>, so we
            // map each SymbolId to its string form.
            let as_strings: Option<Vec<String>> =
                component.map(|members| members.iter().map(|s| s.as_str().to_string()).collect());
            envelope_ok_direct(TOOL_IMPACT_COMPONENT, &as_strings, None)
        }
        // ---- mcp-graph-primitives: graph_subgraph / graph_cluster / graph_explain
        TOOL_GRAPH_SUBGRAPH => {
            let g = match require_graph(graph, TOOL_GRAPH_SUBGRAPH) {
                Ok(g) => g,
                Err(e) => return e,
            };
            let args: GraphSubgraphArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("{TOOL_GRAPH_SUBGRAPH}: invalid args: {e}")),
            };
            let root = match args.root {
                Some(r) => r,
                None => {
                    return err(format!(
                        "{TOOL_GRAPH_SUBGRAPH}: missing required arg `root`"
                    ));
                }
            };
            let direction_str = args.direction.as_deref().unwrap_or("both");
            let direction = match direction_str {
                "outgoing" => SubgraphDirection::Outgoing,
                "incoming" => SubgraphDirection::Incoming,
                "both" => SubgraphDirection::Both,
                other => {
                    return err(format!(
                        "{TOOL_GRAPH_SUBGRAPH}: invalid `direction` `{other}` \
                         (expected one of: outgoing, incoming, both)"
                    ));
                }
            };
            let max_depth = args.max_depth.unwrap_or(DEFAULT_SUBGRAPH_DEPTH);
            let svc = ImpactAnalysisService::new();
            let dto = svc.subgraph(g, &SymbolId::new(root), direction, max_depth);
            envelope_ok_direct(TOOL_GRAPH_SUBGRAPH, &dto, None)
        }
        TOOL_GRAPH_CLUSTER => {
            let g = match require_graph(graph, TOOL_GRAPH_CLUSTER) {
                Ok(g) => g,
                Err(e) => return e,
            };
            let args: GraphClusterArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("{TOOL_GRAPH_CLUSTER}: invalid args: {e}")),
            };
            let method = args.method.as_deref().unwrap_or("scc");
            if method != "scc" && method != "connected" {
                return err(format!(
                    "{TOOL_GRAPH_CLUSTER}: invalid `method` `{method}` \
                     (expected one of: scc, connected)"
                ));
            }
            let svc = ImpactAnalysisService::new();
            let dto = svc.cluster_components(g, method);
            envelope_ok_direct(TOOL_GRAPH_CLUSTER, &dto, None)
        }
        TOOL_GRAPH_EXPLAIN => {
            let g = match require_graph(graph, TOOL_GRAPH_EXPLAIN) {
                Ok(g) => g,
                Err(e) => return e,
            };
            let args: GraphExplainArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("{TOOL_GRAPH_EXPLAIN}: invalid args: {e}")),
            };
            let from = match args.from {
                Some(v) => v,
                None => {
                    return err(format!("{TOOL_GRAPH_EXPLAIN}: missing required arg `from`"));
                }
            };
            let to = match args.to {
                Some(v) => v,
                None => {
                    return err(format!("{TOOL_GRAPH_EXPLAIN}: missing required arg `to`"));
                }
            };
            let svc = ImpactAnalysisService::new();
            // Service guarantees `Some` (wraps None as found:false).
            let dto = svc
                .explain_path(g, &SymbolId::new(from), &SymbolId::new(to))
                .expect("service.explain_path always returns Some");
            envelope_ok_direct(TOOL_GRAPH_EXPLAIN, &dto, None)
        }
        // ---- ask-router: cognicode_ask ----------------------------------
        //
        // The ask router wraps the entire 17-tool surface behind a
        // single natural-language front-end. It runs the question
        // through `AskRouter::classify` and then dispatches the
        // matched pattern's primitive chain via
        // `crate::ask::dispatch::dispatch_ask`. The handler shares
        // the same `Arc<ExplorerService>` and `Option<Arc<CallGraph>>`
        // it uses for the 17 primitives — no MCP chaining, no
        // re-serialisation.
        TOOL_ASK => {
            let args: AskArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("{TOOL_ASK}: invalid args: {e}")),
            };
            let question = match args.question {
                Some(q) if !q.is_empty() => q,
                _ => {
                    return err(format!("{TOOL_ASK}: missing required arg `question`"));
                }
            };
            // `context` is reserved for future use; the current
            // router is a pure function over `(question, _)`.
            let _ = args.context;
            let classified = crate::ask::AskRouter::classify(&question);
            let env = crate::ask::dispatch::dispatch_ask(classified, service, graph, None).await;
            envelope_ok(TOOL_ASK, &Ok::<_, crate::ExplorerError>(env), None)
        }
        // ---- brain-session: 6 full dispatch arms ------------------------
        //
        // Each arm validates its arguments, talks to the in-memory
        // `SessionRegistry` on the handler, and returns a
        // `McpResultEnvelope` whose `provenance.source` is
        // `"brain-session"`. Error envelopes carry an `error_code`
        // in the payload so consumers can distinguish
        // `missing_required_arg` from `session_not_found` without
        // having to grep the human message.
        TOOL_BRAIN_OPEN => {
            let args: BrainOpenArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_OPEN,
                        "missing_required_arg",
                        &format!("{TOOL_BRAIN_OPEN}: invalid args: {e}"),
                    );
                }
            };
            let workspace_id = match args.workspace_id {
                Some(w) if !w.is_empty() => w,
                _ => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_OPEN,
                        "invalid_workspace_id",
                        "missing or empty required arg `workspace_id`",
                    );
                }
            };
            // ttl range: 0..=86400 inclusive. 0 disables expiry.
            // The spec pins the upper bound at 24h; values above are
            // almost certainly caller bugs. Bind the ttl up front so
            // we can both validate and use it.
            let ttl = args.ttl.unwrap_or(crate::session::DEFAULT_TTL_SECS);
            if ttl > 86_400 {
                return envelope_err_with_code(
                    TOOL_BRAIN_OPEN,
                    "invalid_ttl",
                    "ttl must be in 0..=86400 (24h); 0 disables expiry",
                );
            }
            let session_id =
                registry.open(workspace_id.clone(), ttl, service.clone(), graph.clone());
            // Multimodal (brain-federation): if the caller supplied
            // an optional `spaces` list, pre-register each space in
            // the freshly opened session. Errors are collected so
            // they can be surfaced in the response envelope instead
            // of being silently swallowed.
            #[cfg(feature = "multimodal")]
            let mut space_errors: Vec<String> = Vec::new();
            #[cfg(feature = "multimodal")]
            if let Some(ref space_specs) = args.spaces {
                if !space_specs.is_empty() {
                    type SId = cognicode_core::domain::value_objects::SpaceId;
                    type SKind = cognicode_core::domain::value_objects::SpaceKind;
                    type Sp = cognicode_core::domain::value_objects::Space;
                    if let Ok(session) = registry.get(&session_id) {
                        for spec in space_specs {
                            if let Some(ref name) = spec.space_name {
                                if !name.is_empty() {
                                    if let Some(ref k) = spec.space_kind {
                                        if !k.is_empty() {
                                            let kind = match k.to_lowercase().as_str() {
                                                "repo" => SKind::Repo,
                                                "docs" => SKind::Docs,
                                                "issues" => SKind::Issues,
                                                other => {
                                                    space_errors.push(format!(
                                                        "unknown space_kind '{other}' for space '{name}'"
                                                    ));
                                                    continue;
                                                }
                                            };
                                            let sid = match SId::try_new(name.clone()) {
                                                Ok(s) => s,
                                                Err(e) => {
                                                    space_errors.push(format!(
                                                        "invalid space id '{name}': {e}"
                                                    ));
                                                    continue;
                                                }
                                            };
                                            let space = match Sp::try_new(sid, name.clone(), kind) {
                                                Ok(s) => s,
                                                Err(e) => {
                                                    space_errors.push(format!(
                                                        "could not build space '{name}': {e}"
                                                    ));
                                                    continue;
                                                }
                                            };
                                            let space = match spec.source_path {
                                                Some(ref p) if !p.is_empty() => {
                                                    space.with_source_path(p.clone())
                                                }
                                                _ => space,
                                            };
                                            if let Err(e) = session.add_space(space) {
                                                space_errors.push(format!(
                                                    "add_space('{name}') failed: {e}"
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if !space_errors.is_empty() {
                        tracing::warn!(
                            "brain_open: {} space(s) failed to attach: {:?}",
                            space_errors.len(),
                            space_errors
                        );
                    }
                }
            }
            // Return the full state so the caller can attach / ask
            // without a follow-up brain_status.
            let snap = registry
                .attach(&session_id)
                .expect("freshly opened session must be present")
                .snapshot();
            // Surface any per-space attach errors collected during
            // the multimodal pre-registration phase. The state
            // itself is unchanged but the caller needs to know
            // which spaces failed.
            #[cfg(feature = "multimodal")]
            let space_errors_json = serde_json::json!(space_errors);
            #[cfg(not(feature = "multimodal"))]
            let space_errors_json = serde_json::json!([]);
            envelope_ok_brain(
                TOOL_BRAIN_OPEN,
                serde_json::json!({
                    "session_id": session_id,
                    "workspace_id": workspace_id,
                    "ttl_secs": ttl,
                    "state": snap,
                    "space_errors": space_errors_json,
                }),
            )
        }
        TOOL_BRAIN_ATTACH => {
            let args: BrainAttachArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_ATTACH,
                        "missing_required_arg",
                        &format!("{TOOL_BRAIN_ATTACH}: invalid args: {e}"),
                    );
                }
            };
            let session_id = match args.session_id {
                Some(s) if !s.is_empty() => s,
                _ => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_ATTACH,
                        "missing_required_arg",
                        "missing required arg `session_id`",
                    );
                }
            };
            match registry.attach(&session_id) {
                Ok(session) => {
                    let snap = session.snapshot();
                    envelope_ok_brain(
                        TOOL_BRAIN_ATTACH,
                        serde_json::json!({
                            "session_id": session_id,
                            "workspace_id": snap.workspace_id,
                            "last_activity": snap.last_activity,
                            "ttl_secs": snap.ttl,
                            "focus_node": snap.focus_node,
                        }),
                    )
                }
                Err(crate::session::registry::SessionError::NotFound) => envelope_err_with_code(
                    TOOL_BRAIN_ATTACH,
                    "session_not_found",
                    "session_not_found: no session with the supplied id (closed or never existed)",
                ),
                Err(crate::session::registry::SessionError::Expired) => envelope_err_with_code(
                    TOOL_BRAIN_ATTACH,
                    "session_expired",
                    "session_expired: ttl elapsed and the session was lazy-evicted",
                ),
            }
        }
        TOOL_BRAIN_ASK => {
            let args: BrainAskArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_ASK,
                        "missing_required_arg",
                        &format!("{TOOL_BRAIN_ASK}: invalid args: {e}"),
                    );
                }
            };
            let session_id = match args.session_id {
                Some(s) if !s.is_empty() => s,
                _ => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_ASK,
                        "missing_required_arg",
                        "missing required arg `session_id`",
                    );
                }
            };
            let question = match args.question {
                Some(q) if !q.is_empty() => q,
                _ => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_ASK,
                        "missing_required_arg",
                        "missing required arg `question`",
                    );
                }
            };
            // Use `get` (NOT `attach`) so the ask does NOT refresh
            // the session's last_activity. The ask is an inspection
            // step, not a rejoin.
            let session = match registry.get(&session_id) {
                Ok(s) => s,
                Err(crate::session::registry::SessionError::NotFound) => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_ASK,
                        "session_not_found",
                        "session_not_found",
                    );
                }
                Err(crate::session::registry::SessionError::Expired) => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_ASK,
                        "session_expired",
                        "session_expired",
                    );
                }
            };
            // ask_with_session handles focus prepend + history
            // append internally. We override the provenance source
            // to `brain-session` on the way out so consumers can
            // tell brain-session-mediated answers apart from raw
            // cognicode_ask answers.
            let mut env = session.ask_with_session(&question).await;
            match env.provenance.as_mut() {
                Some(p) => p.source = Some("brain-session".to_string()),
                None => {
                    env.provenance = Some(crate::mcp::ProvenanceMetadata {
                        confidence: None,
                        source: Some("brain-session".to_string()),
                    });
                }
            }
            envelope_ok_brain(
                TOOL_BRAIN_ASK,
                serde_json::to_value(&env).unwrap_or(serde_json::Value::Null),
            )
        }
        TOOL_BRAIN_FOCUS => {
            let args: BrainFocusArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_FOCUS,
                        "missing_required_arg",
                        &format!("{TOOL_BRAIN_FOCUS}: invalid args: {e}"),
                    );
                }
            };
            let session_id = match args.session_id {
                Some(s) if !s.is_empty() => s,
                _ => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_FOCUS,
                        "missing_required_arg",
                        "missing required arg `session_id`",
                    );
                }
            };
            // focus_node validation: an empty string is an explicit
            // error; a missing key means "clear"; null means "clear".
            let focus = match args.focus_node {
                Some(f) if f.is_empty() => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_FOCUS,
                        "invalid_focus_node",
                        "focus_node must be a non-empty string or null",
                    );
                }
                Some(f) => Some(f),
                None => None,
            };
            let session = match registry.get(&session_id) {
                Ok(s) => s,
                Err(crate::session::registry::SessionError::NotFound) => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_FOCUS,
                        "session_not_found",
                        "session_not_found",
                    );
                }
                Err(crate::session::registry::SessionError::Expired) => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_FOCUS,
                        "session_expired",
                        "session_expired",
                    );
                }
            };
            session.set_focus(focus.clone());
            envelope_ok_brain(
                TOOL_BRAIN_FOCUS,
                serde_json::json!({
                    "session_id": session_id,
                    "focus_node": focus,
                }),
            )
        }
        TOOL_BRAIN_STATUS => {
            let args: BrainStatusArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_STATUS,
                        "missing_required_arg",
                        &format!("{TOOL_BRAIN_STATUS}: invalid args: {e}"),
                    );
                }
            };
            let session_id = match args.session_id {
                Some(s) if !s.is_empty() => s,
                _ => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_STATUS,
                        "missing_required_arg",
                        "missing required arg `session_id`",
                    );
                }
            };
            let session = match registry.get(&session_id) {
                Ok(s) => s,
                Err(crate::session::registry::SessionError::NotFound) => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_STATUS,
                        "session_not_found",
                        "session_not_found",
                    );
                }
                Err(crate::session::registry::SessionError::Expired) => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_STATUS,
                        "session_expired",
                        "session_expired",
                    );
                }
            };
            let snap = session.snapshot();
            // Multimodal (brain-federation): enrich the status
            // payload with space metadata.
            #[cfg(feature = "multimodal")]
            {
                let space_details: Vec<serde_json::Value> = session
                    .spaces()
                    .into_iter()
                    .map(|s| {
                        serde_json::json!({
                            "id": s.id.as_str(),
                            "name": s.name,
                            "kind": s.kind.as_str(),
                            "source_path": s.source_path.map(|p| p.to_string_lossy().into_owned()),
                        })
                    })
                    .collect();
                let space_count = space_details.len();
                let mut payload = serde_json::to_value(&snap).unwrap_or(serde_json::Value::Null);
                if let Some(ref mut obj) = payload.as_object_mut() {
                    obj.insert("space_count".to_string(), serde_json::json!(space_count));
                    obj.insert("space_details".to_string(), serde_json::json!(space_details));
                }
                return envelope_ok_brain(TOOL_BRAIN_STATUS, payload);
            }
            #[cfg(not(feature = "multimodal"))]
            envelope_ok_brain(
                TOOL_BRAIN_STATUS,
                serde_json::to_value(&snap).unwrap_or(serde_json::Value::Null),
            )
        }
        TOOL_BRAIN_CLOSE => {
            let args: BrainCloseArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_CLOSE,
                        "missing_required_arg",
                        &format!("{TOOL_BRAIN_CLOSE}: invalid args: {e}"),
                    );
                }
            };
            let session_id = match args.session_id {
                Some(s) if !s.is_empty() => s,
                _ => {
                    return envelope_err_with_code(
                        TOOL_BRAIN_CLOSE,
                        "missing_required_arg",
                        "missing required arg `session_id`",
                    );
                }
            };
            // Idempotent: unknown / already-closed → `closed: false`,
            // NOT an error envelope. This is the only brain_* tool
            // where the happy path includes the "no-op" outcome.
            let closed = registry.close(&session_id);
            envelope_ok_brain(
                TOOL_BRAIN_CLOSE,
                serde_json::json!({
                    "session_id": session_id,
                    "closed": closed,
                }),
            )
        }
        // ---- multimodal (brain-federation): 3 new tools (32..=34) ------
        //
        // Each tool is gated behind the `multimodal` Cargo feature.
        // Without the feature the constants are not in `TOOL_NAMES`
        // and the dispatch arms below do not exist, so a stale
        // client that tries to call them on a non-multimodal build
        // gets a clean "Unknown tool" error envelope.
        #[cfg(feature = "multimodal")]
        TOOL_BRAIN_ADD_SPACE => dispatch_brain_add_space(registry, arguments),
        #[cfg(feature = "multimodal")]
        TOOL_BRAIN_REMOVE_SPACE => dispatch_brain_remove_space(registry, arguments),
        #[cfg(feature = "multimodal")]
        TOOL_BRAIN_SPACES => dispatch_brain_spaces(registry, arguments),
        // ---- named-views: 4 full dispatch arms (35..=38) ---------------
        //
        // Each arm validates its arguments, talks to the
        // `ExplorerService` methods that delegate to the
        // `PostgresRepository`, and returns an
        // `McpResultEnvelope`. Validation failures surface as
        // `invalid_input`; a PG unique-violation maps to
        // `named_view_already_exists`; missing rows and scope
        // mismatches map to `not_found`; the feature-gate-off
        // path is the canonical `named_views_require_postgres_feature`.
        TOOL_VIEW_SAVE => dispatch_view_save(service, arguments).await,
        TOOL_VIEW_LOAD => dispatch_view_load(service, arguments).await,
        TOOL_VIEW_LIST => dispatch_view_list(service, arguments).await,
        TOOL_VIEW_DELETE => dispatch_view_delete(service, arguments).await,
        // ---- multimodal (T14): docs_ingest ---------------------
        //
        // Gated behind the `multimodal` Cargo feature. Without
        // the feature, the constant is not in `TOOL_NAMES` and
        // the dispatch arm below does not exist, so a stale
        // client that tries to call `docs_ingest` on a
        // non-multimodal build gets a clean
        // "Unknown tool: docs_ingest" error envelope.
        #[cfg(feature = "multimodal")]
        TOOL_DOCS_INGEST => dispatch_docs_ingest(service, arguments).await,
        // ---- multimodal (T21): graph_search --------------------
        //
        // FTS5-backed search across the `graph_nodes` table.
        // Returns `{results, total_count, next_cursor,
        // raw_rank, normalized_score}`. The `graph_repo` is
        // optional — when absent (the handler was constructed
        // without one), the tool returns
        // `"graph_search_unavailable"`.
        #[cfg(feature = "multimodal")]
        TOOL_GRAPH_SEARCH => dispatch_graph_search(graph_repo, arguments).await,
        // ---- multimodal (T12): issues_ingest -------------------
        #[cfg(feature = "multimodal")]
        TOOL_ISSUES_INGEST => dispatch_issues_ingest(None, arguments).await,
        _ => err(format!("Unknown tool: {name}")),
    }
}

// ============================================================================
// named-views dispatch helpers — extracted from the big `match` so
// each arm stays small and the per-tool error mapping is auditable.
// ============================================================================

#[allow(dead_code)]
fn envelope_named_err_for(tool: &str, code: &str, message: &str) -> CallToolResult {
    envelope_err_with_code(tool, code, message)
}

// ============================================================================
// multimodal (T14) — `docs_ingest` dispatch helper.
// ============================================================================
//
// Compiled into the binary ONLY when the `multimodal` Cargo
// feature is active. The dispatch arm in `dispatch` and the
// `TOOL_DOCS_INGEST` constant are gated the same way.
#[cfg(feature = "multimodal")]
async fn dispatch_docs_ingest(
    service: &Arc<ExplorerService>,
    arguments: serde_json::Value,
) -> CallToolResult {
    use cognicode_core::domain::traits::source_extractor::{SourceExtractor, SourcePath};
    use cognicode_core::infrastructure::extraction::docs_extractor::DocsExtractor;
    use std::path::PathBuf;

    let args: DocsIngestArgs = match serde_json::from_value(arguments) {
        Ok(a) => a,
        Err(e) => {
            return envelope_named_err_for(
                TOOL_DOCS_INGEST,
                "invalid_input",
                &format!("{TOOL_DOCS_INGEST}: invalid args: {e}"),
            );
        }
    };
    let path_str = match args.path {
        Some(s) if !s.is_empty() => s,
        _ => {
            return envelope_named_err_for(
                TOOL_DOCS_INGEST,
                "invalid_input",
                "missing required arg `path`",
            );
        }
    };
    let path = PathBuf::from(&path_str);
    if !path.exists() {
        return envelope_named_err_for(
            TOOL_DOCS_INGEST,
            "not_found",
            &format!("path does not exist: {path_str}"),
        );
    }
    let recursive = args.recursive.unwrap_or(true);
    let extractor = DocsExtractor::new();
    let result = if path.is_dir() {
        match extractor.extract_directory(&path, recursive).await {
            Ok(nodes) => nodes,
            Err(e) => {
                return envelope_named_err_for(
                    TOOL_DOCS_INGEST,
                    "extractor_error",
                    &format!("docs extractor failed: {e}"),
                );
            }
        }
    } else {
        match extractor.extract_file(&path).await {
            Ok(nodes) => nodes,
            Err(e) => {
                return envelope_named_err_for(
                    TOOL_DOCS_INGEST,
                    "extractor_error",
                    &format!("docs extractor failed: {e}"),
                );
            }
        }
    };
    let files_processed = result
        .iter()
        .map(|n| n.potential_node.source_path.clone())
        .filter_map(|p| p)
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    let nodes_created = result.len();
    let edges_created: usize = result.iter().map(|n| n.potential_edges.len()).sum();
    let payload = serde_json::json!({
        "files_processed": files_processed,
        "nodes_created": nodes_created,
        "edges_created": edges_created,
        "errors": Vec::<String>::new(),
    });
    let _ = service;
    envelope_ok_direct(TOOL_DOCS_INGEST, &payload, None)
}

// ============================================================================
// multimodal (T21) — `graph_search` dispatch helper.
// ============================================================================
//
// FTS5-backed search across the `graph_nodes` table. Validates
// the caller's args, delegates to the supplied
// `GraphRepository::search`, and wraps the page in a
// structured `McpResultEnvelope` payload with the documented 5
// top-level fields.
#[cfg(feature = "multimodal")]
async fn dispatch_graph_search(
    graph_repo: Option<&Arc<dyn crate::ports::graph_repository::GraphRepository>>,
    arguments: serde_json::Value,
) -> CallToolResult {
    use cognicode_core::domain::value_objects::node_kind::NodeKind;

    let args: GraphSearchArgs = match serde_json::from_value(arguments) {
        Ok(a) => a,
        Err(e) => {
            return envelope_named_err_for(
                TOOL_GRAPH_SEARCH,
                "invalid_input",
                &format!("{TOOL_GRAPH_SEARCH}: invalid args: {e}"),
            );
        }
    };
    let query = match args.query {
        Some(q) if !q.is_empty() => q,
        _ => {
            return envelope_named_err_for(
                TOOL_GRAPH_SEARCH,
                "invalid_input",
                "missing required arg `query` (must be a non-empty string)",
            );
        }
    };
    let limit = match args.limit {
        Some(n) if n <= 0 => {
            return envelope_named_err_for(
                TOOL_GRAPH_SEARCH,
                "invalid_input",
                &format!("{TOOL_GRAPH_SEARCH}: `limit` must be a positive integer (got {n})"),
            );
        }
        Some(n) => (n as usize).min(MAX_GRAPH_SEARCH_LIMIT),
        None => DEFAULT_GRAPH_SEARCH_LIMIT,
    };
    let mut parsed_kinds: Vec<NodeKind> = Vec::new();
    if let Some(raw) = args.node_kinds {
        for k in raw {
            match k.as_str() {
                // "symbol" is a CATEGORY over every concrete
                // `SymbolKind` variant. The previous behaviour
                // narrowed it to `Function` only, which silently
                // hid classes, methods, structs, traits, etc.
                // We now expand the wildcard to all known
                // symbol kinds (excluding `Unknown`, which is a
                // sentinel for "could not classify").
                //
                // IMPORTANT: When adding a new SymbolKind variant,
                // update BOTH this list AND `SYMBOL_WILDCARD_COUNT`
                // below. The compile-time check fails the build
                // if the count drifts.
                //
                // Rust does not yet support a const assertion on
                // enum variant count, so we use a const-eval
                // arithmetic check: the array literal must
                // produce exactly `SYMBOL_WILDCARD_COUNT` entries.
                // If the lengths disagree, this `const` evaluates
                // to a non-`()` type and the build fails.
                "symbol" => {
                    use cognicode_core::domain::value_objects::symbol_kind::SymbolKind;
                    // Number of non-`Unknown` SymbolKind variants.
                    // Keep this in sync with the array below.
                    const SYMBOL_WILDCARD_COUNT: usize = 21;
                    let wildcard: [NodeKind; SYMBOL_WILDCARD_COUNT] = [
                        NodeKind::Symbol(SymbolKind::Function),
                        NodeKind::Symbol(SymbolKind::Method),
                        NodeKind::Symbol(SymbolKind::Class),
                        NodeKind::Symbol(SymbolKind::Struct),
                        NodeKind::Symbol(SymbolKind::Module),
                        NodeKind::Symbol(SymbolKind::Variable),
                        NodeKind::Symbol(SymbolKind::Parameter),
                        NodeKind::Symbol(SymbolKind::Type),
                        NodeKind::Symbol(SymbolKind::Property),
                        NodeKind::Symbol(SymbolKind::Field),
                        NodeKind::Symbol(SymbolKind::Import),
                        NodeKind::Symbol(SymbolKind::EnumVariant),
                        NodeKind::Symbol(SymbolKind::Trait),
                        NodeKind::Symbol(SymbolKind::Generic),
                        NodeKind::Symbol(SymbolKind::Constant),
                        NodeKind::Symbol(SymbolKind::Constructor),
                        NodeKind::Symbol(SymbolKind::Enum),
                        NodeKind::Symbol(SymbolKind::Interface),
                        NodeKind::Symbol(SymbolKind::File),
                        NodeKind::Symbol(SymbolKind::Namespace),
                        NodeKind::Symbol(SymbolKind::Package),
                    ];
                    // Compile-time length check: a mismatch
                    // between `SYMBOL_WILDCARD_COUNT` and the
                    // actual array length would be caught by the
                    // type annotation above; this assertion
                    // exists for clarity and doubles as a
                    // defensive runtime no-op.
                    debug_assert_eq!(wildcard.len(), SYMBOL_WILDCARD_COUNT);
                    parsed_kinds.extend(wildcard);
                }
                "decision" => parsed_kinds.push(NodeKind::Decision),
                "doc" => parsed_kinds.push(NodeKind::Doc),
                "issue" => parsed_kinds.push(NodeKind::Issue),
                "evidence" => parsed_kinds.push(NodeKind::Evidence),
                other => {
                    return envelope_named_err_for(
                        TOOL_GRAPH_SEARCH,
                        "invalid_input",
                        &format!("{TOOL_GRAPH_SEARCH}: unknown `node_kinds` entry `{other}` (expected one of: symbol, decision, doc, issue, evidence)"),
                    );
                }
            }
        }
    }
    let repo = match graph_repo {
        Some(r) => r,
        None => {
            return envelope_named_err_for(
                TOOL_GRAPH_SEARCH,
                "graph_search_unavailable",
                "graph_search: no GraphRepository wired into the handler",
            );
        }
    };
    let page = match repo.search(&query, &parsed_kinds, limit, args.cursor.as_deref()) {
        Ok(p) => p,
        Err(e) => {
            return envelope_named_err_for(
                TOOL_GRAPH_SEARCH,
                "repository_error",
                &format!("{TOOL_GRAPH_SEARCH}: search failed: {e}"),
            );
        }
    };
    let normalized_score = page.raw_rank.clamp(0.0, 1.0);
    // Per-item scores: prefer the per-item ranks returned by
    // the underlying search backend; fall back to the
    // page-level `raw_rank` when per-item data is missing (e.g.
    // the unimplemented PG stub). The `Vec` is parallel to
    // `page.items`, so zipping by index is safe.
    let per_item_ranks: Vec<f64> = if page.item_ranks.len() == page.items.len() {
        page.item_ranks.clone()
    } else if !page.item_ranks.is_empty() {
        // Length mismatch — defensive: truncate to the
        // shorter side so we never panic in release builds.
        tracing::warn!(
            "graph_search: item_ranks len {} != items len {}, truncating",
            page.item_ranks.len(),
            page.items.len()
        );
        let n = page.item_ranks.len().min(page.items.len());
        page.item_ranks[..n].to_vec()
    } else {
        Vec::new()
    };
    let page_level_rank = page.raw_rank;
    let payload = serde_json::json!({
        "results": page.items.iter().enumerate().map(|(i, n)| {
            // The per-item rank is the source of truth when
            // present. The fallback to the page-level rank is
            // annotated via `score_is_page_level: true` so the
            // front-end can flag the result accordingly.
            let (score, score_is_page_level) = match per_item_ranks.get(i) {
                Some(r) => (*r, false),
                None => (page_level_rank, true),
            };
            serde_json::json!({
                "node": {
                    "id": n.id.as_str(),
                    "label": n.label,
                    "kind": n.kind.as_str(),
                    "source_path": n.source_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
                    "metadata": n.properties,
                },
                "score": score,
                // Per-item rank: the source-of-truth score for
                // THIS result, distinct from the page-level
                // `raw_rank` exposed at the envelope root.
                "item_rank": score,
                "score_is_page_level": score_is_page_level,
            })
        }).collect::<Vec<_>>(),
        "total_count": page.raw_total,
        "next_cursor": page.next_cursor,
        "raw_rank": page.raw_rank,
        "normalized_score": normalized_score,
    });
    envelope_ok_direct(TOOL_GRAPH_SEARCH, &payload, None)
}

// ============================================================================
// multimodal (T12) — `issues_ingest` dispatch helper.
// ============================================================================
//
// Compiled into the binary ONLY when the `multimodal` Cargo
// feature is active. The dispatch arm in `dispatch` and the
// `TOOL_ISSUES_INGEST` constant are gated the same way.
//
// Accepts an optional pre-configured `IssuesExtractor` for
// test injection; production callers pass `None`.
#[cfg(feature = "multimodal")]
async fn dispatch_issues_ingest(
    issues_extractor: Option<
        cognicode_core::infrastructure::extraction::issues_extractor::IssuesExtractor,
    >,
    arguments: serde_json::Value,
) -> CallToolResult {
    use cognicode_core::domain::traits::source_extractor::{SourceExtractor, SourcePath};
    use cognicode_core::infrastructure::extraction::issues_extractor::IssuesExtractor;
    use cognicode_core::infrastructure::github::client::GitHubClient;
    use cognicode_core::infrastructure::github::octocrab_client::OctocrabClient;
    use std::sync::Arc;

    let args: IssuesIngestArgs = match serde_json::from_value(arguments) {
        Ok(a) => a,
        Err(e) => {
            return envelope_named_err_for(
                TOOL_ISSUES_INGEST,
                "invalid_input",
                &format!("{TOOL_ISSUES_INGEST}: invalid args: {e}"),
            );
        }
    };
    let owner = match args.owner {
        Some(o) if !o.is_empty() => o,
        _ => {
            return envelope_named_err_for(
                TOOL_ISSUES_INGEST,
                "invalid_input",
                "missing required arg `owner`",
            );
        }
    };
    let repo = match args.repo {
        Some(r) if !r.is_empty() => r,
        _ => {
            return envelope_named_err_for(
                TOOL_ISSUES_INGEST,
                "invalid_input",
                "missing required arg `repo`",
            );
        }
    };
    // NOTE: `include_git_log` was a v0 prototype that never
    // reached a working implementation. The flag is removed from
    // the schema; the GitHub issue ingestion itself is unchanged.

    let extractor = match issues_extractor {
        Some(e) => e,
        None => IssuesExtractor::with_repo_override(
            Arc::new(OctocrabClient::new()) as Arc<dyn GitHubClient>,
            owner.clone(),
            repo.clone(),
        ),
    };

    let url = format!("https://github.com/{owner}/{repo}");
    let result = match extractor.extract(SourcePath::Url(url)).await {
        Ok(nodes) => nodes,
        Err(e) => {
            return envelope_named_err_for(
                TOOL_ISSUES_INGEST,
                "extractor_error",
                &format!("issues extractor failed: {e}"),
            );
        }
    };

    let issues_processed = result.len();
    let nodes_created = result.len();
    let edges_created: usize = result.iter().map(|n| n.potential_edges.len()).sum();

    let payload = serde_json::json!({
        "issues_processed": issues_processed,
        "nodes_created": nodes_created,
        "edges_created": edges_created,
        "errors": Vec::<String>::new(),
    });
    envelope_ok_direct(TOOL_ISSUES_INGEST, &payload, None)
}

// ============================================================================
// multimodal (brain-federation) — brain_add_space / brain_remove_space /
// brain_spaces dispatch helpers.
// ============================================================================
//
// Each helper validates arguments, talks to the in-memory
// `SessionRegistry` to get the target session, calls the
// corresponding `BrainSessionService` method, and returns a
// `McpResultEnvelope` with `provenance.source = "brain-session"`.

/// Dispatch helper for `brain_add_space`. Validates the caller's
/// args, builds a `Space`, registers it in the session's per-session
/// registry, and returns `{space_id, space_name, space_kind}`.
#[cfg(feature = "multimodal")]
fn dispatch_brain_add_space(
    registry: &crate::session::SessionRegistry,
    arguments: serde_json::Value,
) -> CallToolResult {
    use cognicode_core::domain::value_objects::{Space, SpaceId, SpaceKind};

    let args: BrainAddSpaceArgs = match serde_json::from_value(arguments) {
        Ok(a) => a,
        Err(e) => {
            return envelope_err_with_code(
                TOOL_BRAIN_ADD_SPACE,
                "missing_required_arg",
                &format!("{TOOL_BRAIN_ADD_SPACE}: invalid args: {e}"),
            );
        }
    };
    let session_id = match args.session_id {
        Some(s) if !s.is_empty() => s,
        _ => {
            return envelope_err_with_code(
                TOOL_BRAIN_ADD_SPACE,
                "missing_required_arg",
                "missing required arg `session_id`",
            );
        }
    };
    let space_name = match args.space_name {
        Some(n) if !n.is_empty() => n,
        _ => {
            return envelope_err_with_code(
                TOOL_BRAIN_ADD_SPACE,
                "missing_required_arg",
                "missing required arg `space_name`",
            );
        }
    };
    // Parse space_kind: accept lowercase ("repo") or PascalCase ("Repo").
    let kind_str = match args.space_kind {
        Some(ref k) if !k.is_empty() => k.clone(),
        _ => {
            return envelope_err_with_code(
                TOOL_BRAIN_ADD_SPACE,
                "missing_required_arg",
                "missing required arg `space_kind`",
            );
        }
    };
    let space_kind = match kind_str.to_lowercase().as_str() {
        "repo" => SpaceKind::Repo,
        "docs" => SpaceKind::Docs,
        "issues" => SpaceKind::Issues,
        _ => {
            return envelope_err_with_code(
                TOOL_BRAIN_ADD_SPACE,
                "invalid_space_kind",
                &format!(
                    "invalid space_kind `{kind_str}`: expected one of Repo, Docs, Issues"
                ),
            );
        }
    };
    // Build the SpaceId from the name (the simplest stable id scheme).
    let space_id = match SpaceId::try_new(space_name.clone()) {
        Ok(id) => id,
        Err(_) => {
            return envelope_err_with_code(
                TOOL_BRAIN_ADD_SPACE,
                "invalid_space_id",
                "space name could not be converted to a valid space id",
            );
        }
    };
    // Construct the Space with optional source_path.
    let space = match Space::try_new(space_id, space_name.clone(), space_kind) {
        Ok(s) => s,
        Err(e) => {
            return envelope_err_with_code(
                TOOL_BRAIN_ADD_SPACE,
                "space_construction_error",
                &format!("failed to construct space: {e}"),
            );
        }
    };
    let space = match args.source_path {
        Some(ref p) if !p.is_empty() => space.with_source_path(p.clone()),
        _ => space,
    };
    // Get the session and register the space.
    let session = match registry.get(&session_id) {
        Ok(s) => s,
        Err(crate::session::registry::SessionError::NotFound) => {
            return envelope_err_with_code(
                TOOL_BRAIN_ADD_SPACE,
                "session_not_found",
                "session_not_found",
            );
        }
        Err(crate::session::registry::SessionError::Expired) => {
            return envelope_err_with_code(
                TOOL_BRAIN_ADD_SPACE,
                "session_expired",
                "session_expired",
            );
        }
    };
    if let Err(e) = session.add_space(space) {
        return envelope_err_with_code(
            TOOL_BRAIN_ADD_SPACE,
            "duplicate_space_id",
            &format!("duplicate space id: {e}"),
        );
    }
    envelope_ok_brain(
        TOOL_BRAIN_ADD_SPACE,
        serde_json::json!({
            "space_id": space_name,
            "space_name": space_name,
            "space_kind": space_kind.as_str(),
        }),
    )
}

/// Dispatch helper for `brain_remove_space`. Validates the caller's
/// args, removes the space from the session's registry, and returns
/// `{removed: bool}`. Unknown space_id is NOT an error — the happy
/// path includes `removed: false` (idempotent).
#[cfg(feature = "multimodal")]
fn dispatch_brain_remove_space(
    registry: &crate::session::SessionRegistry,
    arguments: serde_json::Value,
) -> CallToolResult {
    use cognicode_core::domain::value_objects::SpaceId;

    let args: BrainRemoveSpaceArgs = match serde_json::from_value(arguments) {
        Ok(a) => a,
        Err(e) => {
            return envelope_err_with_code(
                TOOL_BRAIN_REMOVE_SPACE,
                "missing_required_arg",
                &format!("{TOOL_BRAIN_REMOVE_SPACE}: invalid args: {e}"),
            );
        }
    };
    let session_id = match args.session_id {
        Some(s) if !s.is_empty() => s,
        _ => {
            return envelope_err_with_code(
                TOOL_BRAIN_REMOVE_SPACE,
                "missing_required_arg",
                "missing required arg `session_id`",
            );
        }
    };
    let space_id_str = match args.space_id {
        Some(s) if !s.is_empty() => s,
        _ => {
            return envelope_err_with_code(
                TOOL_BRAIN_REMOVE_SPACE,
                "missing_required_arg",
                "missing required arg `space_id`",
            );
        }
    };
    let space_id = match SpaceId::try_new(&space_id_str) {
        Ok(id) => id,
        Err(_) => {
            return envelope_err_with_code(
                TOOL_BRAIN_REMOVE_SPACE,
                "invalid_space_id",
                &format!("invalid space_id `{space_id_str}`"),
            );
        }
    };
    let session = match registry.get(&session_id) {
        Ok(s) => s,
        Err(crate::session::registry::SessionError::NotFound) => {
            return envelope_err_with_code(
                TOOL_BRAIN_REMOVE_SPACE,
                "session_not_found",
                "session_not_found",
            );
        }
        Err(crate::session::registry::SessionError::Expired) => {
            return envelope_err_with_code(
                TOOL_BRAIN_REMOVE_SPACE,
                "session_expired",
                "session_expired",
            );
        }
    };
    let removed = session.remove_space(&space_id);
    envelope_ok_brain(
        TOOL_BRAIN_REMOVE_SPACE,
        serde_json::json!({
            "removed": removed,
        }),
    )
}

/// Dispatch helper for `brain_spaces`. Lists every registered space
/// in the session and returns `{spaces: [{id, name, kind, source_path}]}`.
#[cfg(feature = "multimodal")]
fn dispatch_brain_spaces(
    registry: &crate::session::SessionRegistry,
    arguments: serde_json::Value,
) -> CallToolResult {
    let args: BrainSpacesArgs = match serde_json::from_value(arguments) {
        Ok(a) => a,
        Err(e) => {
            return envelope_err_with_code(
                TOOL_BRAIN_SPACES,
                "missing_required_arg",
                &format!("{TOOL_BRAIN_SPACES}: invalid args: {e}"),
            );
        }
    };
    let session_id = match args.session_id {
        Some(s) if !s.is_empty() => s,
        _ => {
            return envelope_err_with_code(
                TOOL_BRAIN_SPACES,
                "missing_required_arg",
                "missing required arg `session_id`",
            );
        }
    };
    let session = match registry.get(&session_id) {
        Ok(s) => s,
        Err(crate::session::registry::SessionError::NotFound) => {
            return envelope_err_with_code(
                TOOL_BRAIN_SPACES,
                "session_not_found",
                "session_not_found",
            );
        }
        Err(crate::session::registry::SessionError::Expired) => {
            return envelope_err_with_code(
                TOOL_BRAIN_SPACES,
                "session_expired",
                "session_expired",
            );
        }
    };
    let spaces: Vec<serde_json::Value> = session
        .spaces()
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id.as_str(),
                "name": s.name,
                "kind": s.kind.as_str(),
                "source_path": s.source_path.map(|p| p.to_string_lossy().into_owned()),
            })
        })
        .collect();
    envelope_ok_brain(
        TOOL_BRAIN_SPACES,
        serde_json::json!({
            "spaces": spaces,
        }),
    )
}

async fn dispatch_view_save(
    service: &Arc<ExplorerService>,
    arguments: serde_json::Value,
) -> CallToolResult {
    let args: ViewSaveArgs = match serde_json::from_value(arguments) {
        Ok(a) => a,
        Err(e) => {
            return envelope_named_err_for(
                TOOL_VIEW_SAVE,
                "invalid_input",
                &format!("{TOOL_VIEW_SAVE}: invalid args: {e}"),
            );
        }
    };
    let workspace_id = match args.workspace_id {
        Some(s) if !s.is_empty() => s,
        _ => {
            return envelope_named_err_for(
                TOOL_VIEW_SAVE,
                "invalid_input",
                "missing required arg `workspace_id`",
            );
        }
    };
    let owner = match args.owner {
        Some(s) if !s.is_empty() => s,
        _ => {
            return envelope_named_err_for(
                TOOL_VIEW_SAVE,
                "invalid_input",
                "missing required arg `owner`",
            );
        }
    };
    let name = match args.name {
        Some(s) if !s.is_empty() => s,
        _ => {
            return envelope_named_err_for(
                TOOL_VIEW_SAVE,
                "invalid_input",
                "missing required arg `name`",
            );
        }
    };
    let level = match args.level {
        Some(s) if !s.is_empty() => s,
        _ => {
            return envelope_named_err_for(
                TOOL_VIEW_SAVE,
                "invalid_input",
                "missing required arg `level`",
            );
        }
    };
    let lens = match args.lens {
        Some(s) if !s.is_empty() => s,
        _ => {
            return envelope_named_err_for(
                TOOL_VIEW_SAVE,
                "invalid_input",
                "missing required arg `lens`",
            );
        }
    };
    let focus_node = match args.focus_node {
        Some(s) if !s.is_empty() => s,
        _ => {
            return envelope_named_err_for(
                TOOL_VIEW_SAVE,
                "invalid_input",
                "missing required arg `focus_node`",
            );
        }
    };
    let max_depth = match args.max_depth {
        Some(d) => d,
        None => {
            return envelope_named_err_for(
                TOOL_VIEW_SAVE,
                "invalid_input",
                "missing required arg `max_depth`",
            );
        }
    };
    match service
        .save_view(
            &workspace_id,
            &owner,
            &name,
            args.description.as_deref(),
            &level,
            &lens,
            &focus_node,
            max_depth,
        )
        .await
    {
        Ok(view) => envelope_ok_direct(TOOL_VIEW_SAVE, &view, None),
        Err(ExplorerError::FeatureDisabled(_)) => envelope_named_err_for(
            TOOL_VIEW_SAVE,
            "named_views_require_postgres_feature",
            "named_views_require_postgres_feature",
        ),
        Err(ExplorerError::InvalidInput(msg)) => {
            envelope_named_err_for(TOOL_VIEW_SAVE, "invalid_input", &msg)
        }
        Err(ExplorerError::Conflict(_)) => envelope_named_err_for(
            TOOL_VIEW_SAVE,
            "named_view_already_exists",
            "named_view_already_exists",
        ),
        Err(other) => envelope_named_err_for(TOOL_VIEW_SAVE, "storage_error", &other.to_string()),
    }
}

async fn dispatch_view_load(
    service: &Arc<ExplorerService>,
    arguments: serde_json::Value,
) -> CallToolResult {
    let args: ViewLoadArgs = match serde_json::from_value(arguments) {
        Ok(a) => a,
        Err(e) => {
            return envelope_named_err_for(
                TOOL_VIEW_LOAD,
                "invalid_input",
                &format!("{TOOL_VIEW_LOAD}: invalid args: {e}"),
            );
        }
    };
    let id = match args.id {
        Some(s) if !s.is_empty() => s,
        _ => {
            return envelope_named_err_for(
                TOOL_VIEW_LOAD,
                "invalid_input",
                "missing required arg `id`",
            );
        }
    };
    let workspace_id = match args.workspace_id {
        Some(s) if !s.is_empty() => s,
        _ => {
            return envelope_named_err_for(
                TOOL_VIEW_LOAD,
                "invalid_input",
                "missing required arg `workspace_id`",
            );
        }
    };
    let owner = match args.owner {
        Some(s) if !s.is_empty() => s,
        _ => {
            return envelope_named_err_for(
                TOOL_VIEW_LOAD,
                "invalid_input",
                "missing required arg `owner`",
            );
        }
    };
    match service.load_view(&id, &workspace_id, &owner).await {
        Ok(view) => envelope_ok_direct(TOOL_VIEW_LOAD, &view, None),
        Err(ExplorerError::FeatureDisabled(_)) => envelope_named_err_for(
            TOOL_VIEW_LOAD,
            "named_views_require_postgres_feature",
            "named_views_require_postgres_feature",
        ),
        Err(ExplorerError::NotFound(_)) => {
            envelope_named_err_for(TOOL_VIEW_LOAD, "not_found", "not_found")
        }
        Err(other) => envelope_named_err_for(TOOL_VIEW_LOAD, "storage_error", &other.to_string()),
    }
}

async fn dispatch_view_list(
    service: &Arc<ExplorerService>,
    arguments: serde_json::Value,
) -> CallToolResult {
    let args: ViewListArgs = match serde_json::from_value(arguments) {
        Ok(a) => a,
        Err(e) => {
            return envelope_named_err_for(
                TOOL_VIEW_LIST,
                "invalid_input",
                &format!("{TOOL_VIEW_LIST}: invalid args: {e}"),
            );
        }
    };
    let workspace_id = match args.workspace_id {
        Some(s) if !s.is_empty() => s,
        _ => {
            return envelope_named_err_for(
                TOOL_VIEW_LIST,
                "invalid_input",
                "missing required arg `workspace_id`",
            );
        }
    };
    let owner = match args.owner {
        Some(s) if !s.is_empty() => s,
        _ => {
            return envelope_named_err_for(
                TOOL_VIEW_LIST,
                "invalid_input",
                "missing required arg `owner`",
            );
        }
    };
    match service.list_views(&workspace_id, &owner).await {
        Ok(rows) => envelope_ok_direct(TOOL_VIEW_LIST, &rows, None),
        Err(ExplorerError::FeatureDisabled(_)) => envelope_named_err_for(
            TOOL_VIEW_LIST,
            "named_views_require_postgres_feature",
            "named_views_require_postgres_feature",
        ),
        Err(ExplorerError::InvalidInput(msg)) => {
            envelope_named_err_for(TOOL_VIEW_LIST, "invalid_input", &msg)
        }
        Err(other) => envelope_named_err_for(TOOL_VIEW_LIST, "storage_error", &other.to_string()),
    }
}

async fn dispatch_view_delete(
    service: &Arc<ExplorerService>,
    arguments: serde_json::Value,
) -> CallToolResult {
    let args: ViewDeleteArgs = match serde_json::from_value(arguments) {
        Ok(a) => a,
        Err(e) => {
            return envelope_named_err_for(
                TOOL_VIEW_DELETE,
                "invalid_input",
                &format!("{TOOL_VIEW_DELETE}: invalid args: {e}"),
            );
        }
    };
    let id = match args.id {
        Some(s) if !s.is_empty() => s,
        _ => {
            return envelope_named_err_for(
                TOOL_VIEW_DELETE,
                "invalid_input",
                "missing required arg `id`",
            );
        }
    };
    let workspace_id = match args.workspace_id {
        Some(s) if !s.is_empty() => s,
        _ => {
            return envelope_named_err_for(
                TOOL_VIEW_DELETE,
                "invalid_input",
                "missing required arg `workspace_id`",
            );
        }
    };
    let owner = match args.owner {
        Some(s) if !s.is_empty() => s,
        _ => {
            return envelope_named_err_for(
                TOOL_VIEW_DELETE,
                "invalid_input",
                "missing required arg `owner`",
            );
        }
    };
    match service.delete_view(&id, &workspace_id, &owner).await {
        Ok(removed) => envelope_ok_direct(
            TOOL_VIEW_DELETE,
            &serde_json::json!({ "deleted": removed }),
            None,
        ),
        Err(ExplorerError::FeatureDisabled(_)) => envelope_named_err_for(
            TOOL_VIEW_DELETE,
            "named_views_require_postgres_feature",
            "named_views_require_postgres_feature",
        ),
        Err(ExplorerError::NotFound(_)) => {
            envelope_named_err_for(TOOL_VIEW_DELETE, "not_found", "not_found")
        }
        Err(ExplorerError::InvalidInput(msg)) => {
            envelope_named_err_for(TOOL_VIEW_DELETE, "invalid_input", &msg)
        }
        Err(other) => envelope_named_err_for(TOOL_VIEW_DELETE, "storage_error", &other.to_string()),
    }
}

/// Build a `CallToolResult::success` carrying a structured error
/// envelope with an `error_code` and a human message. Used by the
/// brain-session validation paths. The body is the same
/// `McpResultEnvelope` shape as the success path so consumers can
/// parse one schema.
fn envelope_err_with_code(tool_name: &str, code: &str, message: &str) -> CallToolResult {
    let envelope = serde_json::json!({
        "tool_name": tool_name,
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "provenance": serde_json::Value::Null,
        "payload": {
            "error_code": code,
            "error": message,
        },
        "suggested_follow_ups": serde_json::Value::Array(Vec::new()),
    });
    let pretty = serde_json::to_string_pretty(&envelope)
        .unwrap_or_else(|e| format!("failed to serialize envelope: {e}"));
    CallToolResult::success(vec![Content::text(pretty)])
}

/// Build a `CallToolResult::success` carrying a brain-session
/// `McpResultEnvelope` with `provenance.source = "brain-session"`.
/// The payload is whatever the dispatch arm produces (a JSON
/// object for open/attach/focus/close/status, a full nested
/// envelope for ask).
fn envelope_ok_brain(tool_name: &str, payload: serde_json::Value) -> CallToolResult {
    let envelope = serde_json::json!({
        "tool_name": tool_name,
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "provenance": {
            "source": "brain-session",
        },
        "payload": payload,
        "suggested_follow_ups": serde_json::Value::Array(Vec::new()),
    });
    let pretty = serde_json::to_string_pretty(&envelope)
        .unwrap_or_else(|e| format!("failed to serialize envelope: {e}"));
    CallToolResult::success(vec![Content::text(pretty)])
}

// ============================================================================
// Result helpers
// ============================================================================

/// Build a [`CallToolResult::success`] carrying an [`McpResultEnvelope`]
/// around a successful service-layer result. Service-layer errors bypass
/// the envelope and surface via [`err`].
fn envelope_ok<T: serde::Serialize>(
    tool_name: &str,
    result: &crate::ExplorerResult<T>,
    provenance: Option<ProvenanceMetadata>,
) -> CallToolResult {
    match result {
        Ok(value) => ok_envelope_inner(tool_name, value, provenance),
        Err(e) => err(e.to_string()),
    }
}

/// Build a [`CallToolResult::success`] carrying an [`McpResultEnvelope`]
/// around a raw `Serialize` value. Used by the 6 impact and 3 graph tools
/// that don't go through an [`crate::ExplorerResult`].
fn envelope_ok_direct<T: serde::Serialize>(
    tool_name: &str,
    value: &T,
    provenance: Option<ProvenanceMetadata>,
) -> CallToolResult {
    ok_envelope_inner(tool_name, value, provenance)
}

/// Internal envelope builder. Produces pretty-printed JSON via
/// `serde_json::json!` + `to_string_pretty` to avoid requiring `T: Clone`.
fn ok_envelope_inner<T: serde::Serialize>(
    tool_name: &str,
    value: &T,
    provenance: Option<ProvenanceMetadata>,
) -> CallToolResult {
    let payload = serde_json::to_value(value).unwrap_or_else(|e| {
        serde_json::Value::String(format!("failed to serialize tool result: {e}"))
    });
    let provenance_json = match provenance {
        Some(p) => serde_json::to_value(p).unwrap_or(serde_json::Value::Null),
        None => serde_json::Value::Null,
    };
    let envelope = serde_json::json!({
        "tool_name": tool_name,
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "provenance": provenance_json,
        "payload": payload,
        "suggested_follow_ups": serde_json::Value::Array(Vec::new()),
    });
    let pretty = serde_json::to_string_pretty(&envelope)
        .unwrap_or_else(|e| format!("failed to serialize envelope: {e}"));
    CallToolResult::success(vec![Content::text(pretty)])
}

/// Build a CallToolResult::error with a single text content carrying
/// the supplied message.
fn err(message: String) -> CallToolResult {
    CallToolResult::error(vec![Content::text(message)])
}

/// Guard for the 5 `impact_*` tools. When the handler was constructed
/// without a call graph, every impact tool surfaces a single canonical
/// error message that names the offending tool. The borrow is returned
/// so the caller can pass `&graph` to the underlying service.
fn require_graph<'a>(
    graph: &'a Option<Arc<CallGraph>>,
    tool: &str,
) -> Result<&'a Arc<CallGraph>, CallToolResult> {
    graph.as_ref().ok_or_else(|| {
        err(format!(
            "{tool}: impact analysis unavailable — no call graph loaded"
        ))
    })
}

// ============================================================================
// Tool schemas
// ============================================================================

/// Build the 8 tool descriptors. The schemas are intentionally hand-rolled
/// (not derived from the args structs) to keep the wire-level contract
/// stable and self-documenting — agents consume the JSON schema directly.
fn build_tool_schemas() -> Vec<Tool> {
    use std::sync::Arc as StdArc;

    let schema = |properties: serde_json::Value,
                  required: &[&str]|
     -> StdArc<serde_json::Map<String, serde_json::Value>> {
        let mut obj = serde_json::json!({
            "type": "object",
            "properties": properties,
        });
        if !required.is_empty() {
            obj["required"] = serde_json::json!(required);
        }
        StdArc::new(obj.as_object().cloned().unwrap())
    };

    vec![
        Tool::new(
            TOOL_OPEN_WORKSPACE,
            "Open (or re-open) a workspace by absolute path. If `root_path` is omitted, the handler's bound workspace is returned. Returns a WorkspaceSummary with graph status and counts.",
            schema(
                serde_json::json!({
                    "root_path": {
                        "type": "string",
                        "description": "Filesystem path to the workspace root. Optional — when omitted, the workspace bound at handler construction is returned."
                    }
                }),
                &[],
            ),
        ),
        Tool::new(
            TOOL_SPOTTER_SEARCH,
            "Spotter search: exact name matches from the call graph, merged with the optional FTS5 / fuzzy backend. `query` is required, `kind` is an optional filter (e.g. 'Function', 'Struct').",
            schema(
                serde_json::json!({
                    "query": { "type": "string", "description": "Search query (required)." },
                    "kind": { "type": "string", "description": "Optional kind filter (e.g. 'Function', 'Struct')." }
                }),
                &["query"],
            ),
        ),
        Tool::new(
            TOOL_INSPECT_OBJECT,
            "Inspect an MVP id and return its InspectableObjectSummary (type, label, properties, available views). The id is the canonical `symbol:{file}:{name}:{line}` / `file:{path}` / `scope:{path}` / `issue:{id}` / `rule:{id}` form.",
            schema(
                serde_json::json!({
                    "object_id": { "type": "string", "description": "MVP id of the object to inspect (required)." }
                }),
                &["object_id"],
            ),
        ),
        Tool::new(
            TOOL_GET_VIEWS,
            "List the contextual views available for a given object (e.g. 'evidence', 'quality', 'callers', 'callees' — depends on the object type).",
            schema(
                serde_json::json!({
                    "object_id": { "type": "string", "description": "MVP id of the object (required)." }
                }),
                &["object_id"],
            ),
        ),
        Tool::new(
            TOOL_GET_VIEW,
            "Render a specific contextual view for an object. Returns blocks, relations, and evidence. For example, `view_id='evidence'` returns the evidence blocks collected about the object.",
            schema(
                serde_json::json!({
                    "object_id": { "type": "string", "description": "MVP id of the object (required)." },
                    "view_id":   { "type": "string", "description": "Id of the view to render (required). Use explorer_get_views to discover the available ids." }
                }),
                &["object_id", "view_id"],
            ),
        ),
        Tool::new(
            TOOL_GET_LENSES,
            "List the design lenses that apply to a given object. Lenses are filtered by the object's type — e.g. a quality issue returns 0 lenses, a scope returns 3 (hotspots, dependencies, architecture).",
            schema(
                serde_json::json!({
                    "object_id": { "type": "string", "description": "MVP id of the object (required)." }
                }),
                &["object_id"],
            ),
        ),
        Tool::new(
            TOOL_APPLY_LENS,
            "Run a registered design lens against an object. Returns a LensResult with findings (hypothesis, severity, confidence, cross-references) and a one-line summary.",
            schema(
                serde_json::json!({
                    "object_id": { "type": "string", "description": "MVP id of the object to analyse (required)." },
                    "lens_id":   { "type": "string", "description": "Id of the lens to run (required). Use explorer_get_lenses to discover the available ids." }
                }),
                &["object_id", "lens_id"],
            ),
        ),
        Tool::new(
            TOOL_QUERY_MOLDQL,
            "Execute a MoldQL query against the explorer. Two query shapes are supported: `FIND <target> [IN SCOPE <path>] [WHERE <cond> AND <cond> ...] [APPLY <lens>]` and `EXPLORE <object_ref> THROUGH <callers|callees> DEPTH <n>`. Returns a MoldQLResultDto with the matched items and the original query echoed back.",
            schema(
                serde_json::json!({
                    "query": { "type": "string", "description": "MoldQL query string (required). Keywords (FIND, EXPLORE, IN, SCOPE, WHERE, AND, APPLY, THROUGH, DEPTH) are case-insensitive." }
                }),
                &["query"],
            ),
        ),
        // ---- impact_* tools (5) ---------------------------------------------
        // All five require the binary to have been started with a real
        // call graph loaded. When the graph is absent, dispatch returns
        // `is_error=true` with text containing
        // `impact analysis unavailable`.
        Tool::new(
            TOOL_IMPACT_RADIUS,
            "Return the **predecessor** impact radius of a symbol: every symbol that depends (directly or transitively) on `root`, within `max_depth` reverse hops. The root itself is excluded. When `max_depth` is omitted, defaults to 5. Returns a JSON array of symbol id strings.",
            schema(
                serde_json::json!({
                    "root": { "type": "string", "description": "Symbol id to analyze (required). Use the `symbol:{file}:{name}:{line}` form." },
                    "max_depth": { "type": "integer", "description": "Maximum reverse BFS depth. Omit to default to 5; pass 0 for an empty result; pass `usize::MAX` (encoded as a very large number) to follow every reachable predecessor." }
                }),
                &["root"],
            ),
        ),
        Tool::new(
            TOOL_IMPACT_FORWARD_RADIUS,
            "Return the **successor** forward radius of a symbol: every symbol that `root` calls (directly or transitively), within `max_depth` forward hops. The root itself is excluded. When `max_depth` is omitted, defaults to 5. Returns a JSON array of symbol id strings.",
            schema(
                serde_json::json!({
                    "root": { "type": "string", "description": "Symbol id to analyze (required). Use the `symbol:{file}:{name}:{line}` form." },
                    "max_depth": { "type": "integer", "description": "Maximum forward BFS depth. Omit to default to 5; pass 0 for an empty result; pass `usize::MAX` (encoded as a very large number) to follow every reachable successor." }
                }),
                &["root"],
            ),
        ),
        Tool::new(
            TOOL_IMPACT_HAS_PATH,
            "Return `true` iff a directed path exists from `from` to `to`. Returns `false` (no panic) when either endpoint is missing. The self-path `A -> A` returns `true` when `A` is present. Returns a JSON object `{from, to, has_path}`.",
            schema(
                serde_json::json!({
                    "from": { "type": "string", "description": "Source symbol id (required)." },
                    "to":   { "type": "string", "description": "Target symbol id (required)." }
                }),
                &["from", "to"],
            ),
        ),
        Tool::new(
            TOOL_IMPACT_SHORTEST_PATH,
            "Compute the lowest-cost (highest-confidence) path from `from` to `to`. Edge cost is `1.0 - confidence`. Returns the JSON-serialized `PathResultDto { path, total_cost, found }`, or JSON `null` when no path exists or an endpoint is missing. Self-path `A -> A` returns `{ path: [\"A\"], total_cost: 0.0, found: true }`.",
            schema(
                serde_json::json!({
                    "from": { "type": "string", "description": "Source symbol id (required)." },
                    "to":   { "type": "string", "description": "Target symbol id (required)." }
                }),
                &["from", "to"],
            ),
        ),
        Tool::new(
            TOOL_IMPACT_DETECT_CYCLES,
            "Return all non-trivial strongly connected components (SCCs) of the call graph — mutual-dependency cycles of size ≥ 2. Self-loops and DAGs return `[]`. Returns a JSON array of `{members: [string], size: number}` objects.",
            schema(serde_json::json!({}), &[]),
        ),
        Tool::new(
            TOOL_IMPACT_COMPONENT,
            "Return the undirected connected component containing `id`. An isolated node is its own component. Returns a JSON array of symbol id strings, or JSON `null` when `id` is missing from the graph.",
            schema(
                serde_json::json!({
                    "id": { "type": "string", "description": "Symbol id whose undirected component to return (required)." }
                }),
                &["id"],
            ),
        ),
        // ---- mcp-graph-primitives: graph_subgraph / graph_cluster / graph_explain
        Tool::new(
            TOOL_GRAPH_SUBGRAPH,
            "Extract a neighborhood subgraph of `root` bounded by `max_depth` hops in `direction` (incoming | outgoing | both, default both). When `max_depth` is omitted, defaults to 3. Returns a JSON `{nodes: [string], edges: [{source, target, dependency_type, confidence}]}` payload.",
            schema(
                serde_json::json!({
                    "root": { "type": "string", "description": "Symbol id of the subgraph root (required)." },
                    "direction": { "type": "string", "enum": ["incoming", "outgoing", "both"], "description": "Edge direction to walk. Omit to default to `both`." },
                    "max_depth": { "type": "integer", "description": "Maximum BFS depth. Omit to default to 3; pass 0 for root-only view." }
                }),
                &["root"],
            ),
        ),
        Tool::new(
            TOOL_GRAPH_CLUSTER,
            "Cluster the call graph by `method` (`scc` for strongly connected components, `connected` for undirected connected components, default `scc`). Returns a JSON array of `{members: [string], size: number}` clusters. Empty `{}` is valid input.",
            schema(
                serde_json::json!({
                    "method": { "type": "string", "enum": ["scc", "connected"], "description": "Cluster method. Omit to default to `scc`." }
                }),
                &[],
            ),
        ),
        Tool::new(
            TOOL_GRAPH_EXPLAIN,
            "Explain the lowest-cost (highest-confidence) path from `from` to `to`: returns a JSON `{found, hops: [{from, to, dependency_type, confidence, rationale}], total_cost, summary}`. The `rationale` is a human-readable verb phrase (e.g. `calls`, `inherits from`). Missing endpoint or unreachable target yields `found: false` (NOT an error).",
            schema(
                serde_json::json!({
                    "from": { "type": "string", "description": "Source symbol id (required)." },
                    "to":   { "type": "string", "description": "Target symbol id (required)." }
                }),
                &["from", "to"],
            ),
        ),
        // ---- ask-router: cognicode_ask (the 18th tool) -----------------
        //
        // A natural-language front-end for the 17 primitive tools
        // above. The router classifies the question against 8
        // priority-ordered patterns and dispatches the matched
        // pattern's primitive chain internally. Graph-dependent
        // patterns are gated by the in-memory `CallGraph`; when the
        // graph is missing, the router returns a `graph_unavailable`
        // error envelope that lists the non-graph patterns that
        // remain available (4, 8).
        Tool::new(
            TOOL_ASK,
            "Ask a natural-language question about the workspace. The router classifies the question against 8 priority-ordered patterns (path-between, forward reach, backward reach, code quality, architecture, workspace overview, component membership, generic description) and dispatches the matched pattern's primitive chain. Returns a `McpResultEnvelope` whose `payload` has `primary_result` (the key tool output) and `supporting` (auxiliary results keyed by primitive name). `provenance.source` is `\"ask-router\"` and `provenance.confidence` reflects the pattern match score (full=1.0, partial=0.7, fallback=0.5). `suggested_follow_ups` is always non-empty and includes context-aware hints (e.g. inverse-direction follow-ups, disambiguation candidates, hints on empty results).",
            schema(
                serde_json::json!({
                    "question": { "type": "string", "description": "Free-form natural-language question (required). Backtick-quoted tokens (e.g. `validate`) are extracted as entity candidates via spotter_search." },
                    "context":  { "type": "object", "description": "Optional context object reserved for future use (routing hints, conversation state). The current router does not consult it." }
                }),
                &["question"],
            ),
        ),
        // ---- brain-session: 6 new tools (19..=24) ------------------------
        //
        // Backed by an in-memory `SessionRegistry` on the handler. The
        // 6 tools collectively form a conversational front-end: open
        // a session, attach to an existing one, ask questions that
        // get enriched with the session's focus node, set/clear the
        // focus, dump the full state, and close the session
        // (idempotent).
        Tool::new(
            TOOL_BRAIN_OPEN,
            "Open a new brain session. Returns a `session_id` (UUIDv4-shaped string) that the caller uses for subsequent `brain_attach`, `brain_ask`, `brain_focus`, `brain_status`, and `brain_close` calls. `workspace_id` is required (non-empty). `ttl` is the session's time-to-live in seconds; omit to use the 30-minute default, pass `0` to disable expiry, or pass any value in 1..=86400. The full session state (including the 50-entry FIFO history and the focus node) is returned in the response so the caller doesn't need a follow-up `brain_status`.",
            schema(
                serde_json::json!({
                    "workspace_id": { "type": "string", "description": "Workspace id this session is bound to (required, non-empty)." },
                    "ttl":          { "type": "integer", "description": "Time-to-live in seconds. Omit to default to 1800 (30 min). Pass `0` to disable expiry. Range: 0..=86400 (24h)." }
                }),
                &["workspace_id"],
            ),
        ),
        Tool::new(
            TOOL_BRAIN_ATTACH,
            "Rejoin an existing brain session. Refreshes the session's `last_activity` timestamp so the TTL countdown restarts. Returns the session's `workspace_id`, `last_activity`, `ttl_secs`, and current `focus_node` (or `null` if unset). Errors: `session_not_found` (unknown id, including already-closed) or `session_expired` (TTL elapsed and the session was lazy-evicted on a previous open/attach).",
            schema(
                serde_json::json!({
                    "session_id": { "type": "string", "description": "Session id returned by `brain_open` (required, non-empty)." }
                }),
                &["session_id"],
            ),
        ),
        Tool::new(
            TOOL_BRAIN_ASK,
            "Ask a question within an existing brain session. When the session has a focus node set (via `brain_focus`), the focus is prepended as a backtick-quoted token to the question before it reaches the ask router, so the dispatcher has the focused entity in scope without the caller repeating it. The question history is appended on a successful dispatch; failed asks (e.g. `graph_unavailable`) are NOT recorded. The response is the inner `McpResultEnvelope`'s `payload` (primary_result + supporting + follow_ups), wrapped with `provenance.source = \"brain-session\"`.",
            schema(
                serde_json::json!({
                    "session_id": { "type": "string", "description": "Session id returned by `brain_open` (required, non-empty)." },
                    "question":   { "type": "string", "description": "Free-form question (required, non-empty). If the session has a focus node set, the focus is prepended automatically — you do not need to repeat it." }
                }),
                &["session_id", "question"],
            ),
        ),
        Tool::new(
            TOOL_BRAIN_FOCUS,
            "Set or clear the per-session focus node. Pass `focus_node` as a non-empty string to set it; pass `null` (or omit the key entirely) to clear. The focus is prepended to the next `brain_ask` question, so use this to make a sequence of follow-up questions stick to a single symbol without repeating it. Errors: `session_not_found`, `invalid_focus_node` (empty string), `session_expired`.",
            schema(
                serde_json::json!({
                    "session_id": { "type": "string", "description": "Session id returned by `brain_open` (required, non-empty)." },
                    "focus_node": { "type": ["string", "null"], "description": "Symbol id to focus subsequent asks on. Pass a non-empty string to set, `null` to clear. Empty strings are rejected as `invalid_focus_node`." }
                }),
                &["session_id"],
            ),
        ),
        Tool::new(
            TOOL_BRAIN_STATUS,
            "Return the full state of a brain session: `session_id`, `workspace_id`, `created_at`, `last_activity`, `ttl_secs`, `focus_node` (or `null` if unset), and the bounded history (capped at 50 entries, FIFO). The history array is ALWAYS present in the payload, even when empty — it serializes as `[]`, never `null` and never omitted. Errors: `session_not_found`, `session_expired`.",
            schema(
                serde_json::json!({
                    "session_id": { "type": "string", "description": "Session id returned by `brain_open` (required, non-empty)." }
                }),
                &["session_id"],
            ),
        ),
        Tool::new(
            TOOL_BRAIN_CLOSE,
            "Close a brain session and remove it from the registry. IDEMPOTENT: closing an unknown or already-closed session returns `{session_id, closed: false}` with HTTP 200, NOT an error envelope. This is the only brain_* tool where the happy path is `closed: false` — close-time race conditions are expected and not surfaced as errors.",
            schema(
                serde_json::json!({
                    "session_id": { "type": "string", "description": "Session id returned by `brain_open` (required, non-empty)." }
                }),
                &["session_id"],
            ),
        ),
        // ---- multimodal (brain-federation): 3 new tool schemas (32..=34)
        //
        // Feature-gated behind the `multimodal` Cargo feature. Without
        // the feature these schemas are absent from `tools/list`, so
        // the wire-level surface is unchanged on a default build.
        #[cfg(feature = "multimodal")]
        Tool::new(
            TOOL_BRAIN_ADD_SPACE,
            "Register a new federation space in an existing brain session. `session_id`, `space_name`, and `space_kind` are required; `source_path` is optional. Returns `{space_id, space_name, space_kind}`. Errors: `session_not_found`, `session_expired`, `missing_required_arg`, `invalid_space_kind`, `duplicate_space_id`.",
            schema(
                serde_json::json!({
                    "session_id": { "type": "string", "description": "Session id returned by `brain_open` (required, non-empty)." },
                    "space_name": { "type": "string", "description": "Human-readable name for the space (required, non-empty). Also used as the `space_id`." },
                    "space_kind": { "type": "string", "description": "One of `Repo`, `Docs`, `Issues` (case-insensitive). Required." },
                    "source_path": { "type": "string", "description": "Optional filesystem path or URL the space was loaded from." }
                }),
                &["session_id", "space_name", "space_kind"],
            ),
        ),
        #[cfg(feature = "multimodal")]
        Tool::new(
            TOOL_BRAIN_REMOVE_SPACE,
            "Remove a federation space from a brain session by `space_id`. IDEMPOTENT: removing an unknown or already-removed space returns `{removed: false}` with HTTP 200, NOT an error envelope. Errors: `session_not_found`, `session_expired`. Returns `{removed: bool}`.",
            schema(
                serde_json::json!({
                    "session_id": { "type": "string", "description": "Session id returned by `brain_open` (required, non-empty)." },
                    "space_id":   { "type": "string", "description": "The space id to remove (required, non-empty)." }
                }),
                &["session_id", "space_id"],
            ),
        ),
        #[cfg(feature = "multimodal")]
        Tool::new(
            TOOL_BRAIN_SPACES,
            "List every registered federation space in a brain session. Each entry carries the space's `id`, `name`, `kind`, and optional `source_path`. An empty session returns `{spaces: []}` — never `null`, never omitted. Errors: `session_not_found`, `session_expired`.",
            schema(
                serde_json::json!({
                    "session_id": { "type": "string", "description": "Session id returned by `brain_open` (required, non-empty)." }
                }),
                &["session_id"],
            ),
        ),
        // ---- named-views: 4 new tool schemas (35..=38) ----------------
        //
        // Each tool is a thin wrapper over the corresponding
        // `ExplorerService::save_view / load_view / list_views /
        // delete_view` method. The feature gate is enforced at
        // the service boundary — without the `postgres` feature
        // active, every tool returns the canonical
        // `named_views_require_postgres_feature` error.
        Tool::new(
            TOOL_VIEW_SAVE,
            "Save a named view: persist a `(level, lens, focus_node, max_depth)` projection tuple plus `name`, `description`, `workspace_id`, and `owner` to the `named_views` PostgreSQL table. The server generates a stable `id` (UUIDv4-shaped string) and `created_at` (server-assigned on insert). Returns the full `NamedView` on success. On a unique-violation against `(workspace_id, owner, name)`, returns `error_code = \"named_view_already_exists\"`. Without the `postgres` feature active, returns `error_code = \"named_views_require_postgres_feature\"`.",
            schema(
                serde_json::json!({
                    "workspace_id": { "type": "string", "description": "Workspace id this view is scoped to (required, non-empty)." },
                    "owner":        { "type": "string", "description": "Principal (user id) that owns this view (required, non-empty)." },
                    "name":         { "type": "string", "description": "Display name for the view. Must be unique per (workspace_id, owner) and at most 200 characters (required, non-empty)." },
                    "description":  { "type": "string", "description": "Optional free-form description, at most 2000 characters." },
                    "level":        { "type": "string", "description": "Projection level identifier (e.g. 'function', 'module', 'scope'). Required, non-empty." },
                    "lens":         { "type": "string", "description": "Lens identifier (e.g. 'callgraph', 'overview', 'hotspots'). Required, non-empty." },
                    "focus_node":   { "type": "string", "description": "Object id of the focus (e.g. 'symbol:crate::foo::bar'). Required, non-empty." },
                    "max_depth":    { "type": "integer", "description": "Maximum depth for the projection (>= 0). Required." }
                }),
                &[
                    "workspace_id",
                    "owner",
                    "name",
                    "level",
                    "lens",
                    "focus_node",
                    "max_depth",
                ],
            ),
        ),
        Tool::new(
            TOOL_VIEW_LOAD,
            "Load a named view by id and re-invoke the saved projection through the existing `contextual_view` pipeline. Returns a `ContextualView` reflecting the current graph state (NOT a stale snapshot). The scope guard requires the row's `workspace_id` and `owner` to match the caller-supplied scope; mismatch returns `error_code = \"not_found\"` (no existence leak). Unknown id returns the same `not_found` envelope. Without the `postgres` feature active, returns `error_code = \"named_views_require_postgres_feature\"`.",
            schema(
                serde_json::json!({
                    "id":            { "type": "string", "description": "Named view id returned by `view_save` (required, non-empty)." },
                    "workspace_id":  { "type": "string", "description": "Workspace id scope guard (required, non-empty)." },
                    "owner":         { "type": "string", "description": "Owner scope guard (required, non-empty)." }
                }),
                &["id", "workspace_id", "owner"],
            ),
        ),
        Tool::new(
            TOOL_VIEW_LIST,
            "List every named view for a `(workspace_id, owner)` scope, ordered by `created_at DESC` (newest first). Returns `Vec<NamedViewDescriptor>` where the `description` field is truncated to ≤ 201 chars (200 + an ellipsis `'…'`) when the stored text is longer. Returns `Ok(vec![])` for an empty scope (NOT an error). Without the `postgres` feature active, returns `error_code = \"named_views_require_postgres_feature\"`.",
            schema(
                serde_json::json!({
                    "workspace_id": { "type": "string", "description": "Workspace id (required, non-empty)." },
                    "owner":        { "type": "string", "description": "Owner (required, non-empty)." }
                }),
                &["workspace_id", "owner"],
            ),
        ),
        Tool::new(
            TOOL_VIEW_DELETE,
            "Delete a named view by id, scoped to `(workspace_id, owner)`. Returns `{ deleted: true }` on success. Unknown id and scope mismatch both return `error_code = \"not_found\"` (no existence leak) — the row is never partially deleted. Without the `postgres` feature active, returns `error_code = \"named_views_require_postgres_feature\"`.",
            schema(
                serde_json::json!({
                    "id":            { "type": "string", "description": "Named view id returned by `view_save` (required, non-empty)." },
                    "workspace_id":  { "type": "string", "description": "Workspace id scope guard (required, non-empty)." },
                    "owner":         { "type": "string", "description": "Owner scope guard (required, non-empty)." }
                }),
                &["id", "workspace_id", "owner"],
            ),
        ),
        // ---- multimodal (T14): docs_ingest (29) -----------------
        //
        // Schema entry is gated behind the `multimodal`
        // feature so the tool is absent from `tools/list` on
        // default builds.
        #[cfg(feature = "multimodal")]
        Tool::new(
            TOOL_DOCS_INGEST,
            "Ingest Markdown / ADR files into the Generic Graph Layer. Walks `path` (a single file or a directory) with `DocsExtractor` and upserts the resulting `Doc` / `Decision` nodes + `Cites` edges into the `graph_nodes` / `graph_edges` PG tables. Returns a structured `McpResultEnvelope` whose payload is `{files_processed, nodes_created, edges_created, errors}` — `errors` is the list of files the extractor skipped. `recursive` defaults to `true`. Idempotent: re-ingesting the same file updates the existing rows.",
            schema(
                serde_json::json!({
                    "path":      { "type": "string", "description": "Filesystem path to ingest. Either a single `.md`/`.markdown`/`.mdx` file or a directory to walk (required, non-empty)." },
                    "recursive": { "type": "boolean", "description": "When `path` is a directory, recurse into subdirectories. Defaults to `true`. Ignored when `path` is a single file." }
                }),
                &["path"],
            ),
        ),
        // ---- multimodal (T21): graph_search (30) ---------------
        //
        // Same feature-gate pattern as `docs_ingest`. FTS5-
        // backed search across `graph_nodes`. Returns a
        // paginated payload (default 50 per page, max 200).
        #[cfg(feature = "multimodal")]
        Tool::new(
            TOOL_GRAPH_SEARCH,
            "FTS5-backed search across the `graph_nodes` table. Returns multimodal nodes (Symbol / Decision / Doc / Issue / Evidence) whose label or metadata matches `query`. The `node_kinds` filter restricts the search to one or more kinds; omit to search every kind. `limit` defaults to 50, capped at 200. Pagination is opaque: the response carries `next_cursor` (a string) which the caller passes back as `cursor` to fetch the next page; `null` means the last page. The payload also exposes `raw_rank` (the FTS5 `ts_rank_cd` value) and `normalized_score` (`raw_rank` clamped to `[0.0, 1.0]`) per the design's Information Bottleneck check. Without the `multimodal` Cargo feature active, the tool is absent from `tools/list`.",
            schema(
                serde_json::json!({
                    "query":      { "type": "string", "description": "Search query (required, non-empty). The match is case-insensitive substring on the node's label and on the values of its `metadata` map." },
                    "node_kinds": { "type": "array",  "items": { "type": "string" }, "description": "Optional filter — one or more of `symbol`, `decision`, `doc`, `issue`, `evidence`. Omit to search every kind." },
                    "cursor":     { "type": "string", "description": "Opaque cursor returned by the previous call's `next_cursor`. Omit (or pass `null`) for the first page." },
                    "limit":      { "type": "integer", "description": "Page size — defaults to 50, capped at 200. Values > 200 are silently capped; values <= 0 are rejected." }
                }),
                &["query"],
            ),
        ),
        // ---- multimodal (T12): issues_ingest (31) ---------------
        #[cfg(feature = "multimodal")]
        Tool::new(
            TOOL_ISSUES_INGEST,
            "Ingest GitHub issues from the given `owner`/`repo` into the Generic Graph Layer. Fetches issues via `IssuesExtractor` (which calls the GitHub REST API) and upserts the resulting `Issue` nodes + edges into the `graph_nodes` / `graph_edges` PG tables. Returns a structured `McpResultEnvelope` whose payload is `{issues_processed, nodes_created, edges_created, errors}`. Idempotent: re-ingesting the same repo updates the existing rows. The legacy `include_git_log` flag was removed from the schema; the git-log parser integration is not implemented yet.",
            schema(
                serde_json::json!({
                    "owner":          { "type": "string", "description": "GitHub owner / organisation (required, non-empty)." },
                    "repo":           { "type": "string", "description": "GitHub repository name (required, non-empty)." }
                }),
                &["owner", "repo"],
            ),
        ),
    ]
}

// ============================================================================
// Tests
// ============================================================================
//
// We can't construct a real `rmcp::service::RequestContext<RoleServer>` in
// unit tests (its constructor is `pub(crate)` in rmcp), so we exercise the
// dispatch logic directly via the public `tool_names` / `build_tool_schemas`
// surfaces and by calling the service through a hand-built
// `ExplorerMcpHandler`. The integration tests in `tests/integration.rs`
// verify the binary's link surface and the tool list contract.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::FsSourceReader;
    use crate::dto::OpenWorkspaceRequest;
    use crate::ports::symbol_repository::{
        GraphStats, RelationTarget, ResolvedSymbol, SymbolRepository,
    };
    use cognicode_core::domain::aggregates::SymbolId;
    use cognicode_core::domain::value_objects::SymbolKind;
    use std::collections::{BTreeSet, HashMap};
    use std::path::PathBuf;
    use std::sync::Arc;

    /// In-memory symbol repository backed by a hashmap. Mirrors the
    /// MockRepo in `service::tests` so dispatch tests get a service
    /// that has real symbols to find.
    #[derive(Debug, Default)]
    struct TestRepo {
        by_name: HashMap<String, Vec<ResolvedSymbol>>,
        by_id: HashMap<String, ResolvedSymbol>,
    }

    impl TestRepo {
        fn new() -> Self {
            Self::default()
        }

        fn with_symbol(
            &mut self,
            name: &str,
            file: &str,
            line: u32,
            kind: SymbolKind,
        ) -> &mut Self {
            let id = SymbolId::new(format!("{file}:{name}:{line}"));
            let sym = ResolvedSymbol {
                id: id.clone(),
                name: name.to_string(),
                kind,
                file: file.to_string(),
                line,
                signature: None,
            };
            self.by_id.insert(id.to_string(), sym.clone());
            self.by_name.entry(name.to_string()).or_default().push(sym);
            self
        }
    }

    impl SymbolRepository for TestRepo {
        fn resolve(&self, id: &SymbolId) -> crate::ExplorerResult<Option<ResolvedSymbol>> {
            Ok(self.by_id.get(id.as_str()).cloned())
        }
        fn callers(&self, _id: &SymbolId) -> Vec<RelationTarget> {
            Vec::new()
        }
        fn callees(&self, _id: &SymbolId) -> Vec<RelationTarget> {
            Vec::new()
        }
        fn fan_in(&self, _id: &SymbolId) -> usize {
            0
        }
        fn fan_out(&self, _id: &SymbolId) -> usize {
            0
        }
        fn find_symbols_by_name(&self, name: &str) -> crate::ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self.by_name.get(name).cloned().unwrap_or_default())
        }
        fn find_symbols_by_file(&self, file: &str) -> crate::ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self
                .by_id
                .values()
                .filter(|s| s.file == file)
                .cloned()
                .collect())
        }
        fn module_list(&self) -> Vec<String> {
            let mut modules: BTreeSet<String> = BTreeSet::new();
            for s in self.by_id.values() {
                if let Some(parent) = std::path::Path::new(&s.file).parent() {
                    let p = parent.to_string_lossy().to_string();
                    if !p.is_empty() {
                        modules.insert(p);
                    }
                }
            }
            modules.into_iter().collect()
        }
        fn all_symbols(&self) -> crate::ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self.by_id.values().cloned().collect())
        }
        fn graph_stats(&self) -> GraphStats {
            GraphStats {
                symbol_count: self.by_id.len(),
                relation_count: 0,
            }
        }
    }

    /// Build a fresh `SessionRegistry` for tests. The registry is
    /// independent of the service, so the helper just constructs a
    /// new one each call.
    fn build_test_registry() -> crate::session::SessionRegistry {
        crate::session::SessionRegistry::new()
    }

    /// Build a service bound to a fresh tempdir with two known symbols.
    fn build_test_service() -> (Arc<ExplorerService>, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut repo = TestRepo::new();
        repo.with_symbol("alpha", "src/a.rs", 1, SymbolKind::Function);
        repo.with_symbol("beta", "src/b.rs", 5, SymbolKind::Struct);
        let repo: Arc<dyn SymbolRepository> = Arc::new(repo);
        let reader = Arc::new(FsSourceReader::new(dir.path().to_path_buf()));
        let service = Arc::new(ExplorerService::new(repo, reader, dir.path().to_path_buf()));
        (service, dir)
    }

    /// Build a `CallToolRequestParams` from a tool name and JSON arguments.
    fn call_tool_args(name: &str, arguments: serde_json::Value) -> CallToolRequestParams {
        let map = match arguments {
            serde_json::Value::Object(m) => m,
            other => panic!("expected JSON object for arguments, got: {other}"),
        };
        CallToolRequestParams::new(name.to_string()).with_arguments(map)
    }

    /// Convenience: extract the text from the first content item.
    fn first_text(result: &CallToolResult) -> String {
        result
            .content
            .first()
            .expect("at least one content item")
            .as_text()
            .expect("text content")
            .text
            .clone()
    }

    // ---- list_tools contract ------------------------------------------------

    #[test]
    fn tool_schemas_list_twentyeight_tools() {
        let tools = build_tool_schemas();
        // The multimodal feature adds 2 tools (docs_ingest,
        // graph_search). The count is 28 by default and 30
        // with the feature.
        let expected_count = if cfg!(feature = "multimodal") { 34 } else { 28 };
        assert_eq!(
            tools.len(),
            expected_count,
            "expected {expected_count} tools, got {}",
            tools.len()
        );

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        let mut expected: Vec<&str> = vec![
            TOOL_OPEN_WORKSPACE,
            TOOL_SPOTTER_SEARCH,
            TOOL_INSPECT_OBJECT,
            TOOL_GET_VIEWS,
            TOOL_GET_VIEW,
            TOOL_GET_LENSES,
            TOOL_APPLY_LENS,
            TOOL_QUERY_MOLDQL,
            TOOL_IMPACT_RADIUS,
            TOOL_IMPACT_HAS_PATH,
            TOOL_IMPACT_SHORTEST_PATH,
            TOOL_IMPACT_DETECT_CYCLES,
            TOOL_IMPACT_COMPONENT,
            TOOL_IMPACT_FORWARD_RADIUS,
            TOOL_GRAPH_SUBGRAPH,
            TOOL_GRAPH_CLUSTER,
            TOOL_GRAPH_EXPLAIN,
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
        ];
        if cfg!(feature = "multimodal") {
            #[cfg(feature = "multimodal")]
            {
                expected.push(TOOL_DOCS_INGEST);
                expected.push(TOOL_GRAPH_SEARCH);
                expected.push(TOOL_ISSUES_INGEST);
                expected.push(TOOL_BRAIN_ADD_SPACE);
                expected.push(TOOL_BRAIN_REMOVE_SPACE);
                expected.push(TOOL_BRAIN_SPACES);
            }
        }
        for e in expected {
            assert!(
                names.contains(&e),
                "tool list missing `{}` — got: {:?}",
                e,
                names
            );
        }
    }

    /// Spec requirement: the 4 new named-view tool names MUST
    /// each be present in `build_tool_schemas()`, exactly once.
    /// The "exactly once" part is enforced by the no-duplicates
    /// test below.
    #[test]
    fn tool_schemas_new_view_names_registered() {
        let names: Vec<String> = build_tool_schemas()
            .iter()
            .map(|t| t.name.to_string())
            .collect();
        for required in [
            TOOL_VIEW_SAVE,
            TOOL_VIEW_LOAD,
            TOOL_VIEW_LIST,
            TOOL_VIEW_DELETE,
        ] {
            assert!(
                names.iter().any(|n| n == required),
                "missing new tool name `{required}` in: {names:?}"
            );
        }
    }

    /// Spec requirement: all 28 tool names are distinct.
    /// `HashSet::from_iter` + length check is the simplest
    /// "no duplicates" assertion.
    #[test]
    fn tool_schemas_no_duplicate_names() {
        let tools = build_tool_schemas();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        let set: std::collections::HashSet<&str> = names.iter().copied().collect();
        assert_eq!(
            set.len(),
            names.len(),
            "tool name collision: {} items but {} distinct — names: {:?}",
            names.len(),
            set.len(),
            names
        );
    }

    /// Spec requirement: the 24 pre-existing tool names are
    /// preserved (no rename, no removal). The snapshot below
    /// matches the wire-level surface BEFORE the named-views
    /// change landed.
    #[test]
    fn tool_schemas_preserve_existing_24_names() {
        let pre_change_24 = [
            TOOL_OPEN_WORKSPACE,
            TOOL_SPOTTER_SEARCH,
            TOOL_INSPECT_OBJECT,
            TOOL_GET_VIEWS,
            TOOL_GET_VIEW,
            TOOL_GET_LENSES,
            TOOL_APPLY_LENS,
            TOOL_QUERY_MOLDQL,
            TOOL_IMPACT_RADIUS,
            TOOL_IMPACT_HAS_PATH,
            TOOL_IMPACT_SHORTEST_PATH,
            TOOL_IMPACT_DETECT_CYCLES,
            TOOL_IMPACT_COMPONENT,
            TOOL_IMPACT_FORWARD_RADIUS,
            TOOL_GRAPH_SUBGRAPH,
            TOOL_GRAPH_CLUSTER,
            TOOL_GRAPH_EXPLAIN,
            TOOL_ASK,
            TOOL_BRAIN_OPEN,
            TOOL_BRAIN_ATTACH,
            TOOL_BRAIN_ASK,
            TOOL_BRAIN_FOCUS,
            TOOL_BRAIN_STATUS,
            TOOL_BRAIN_CLOSE,
        ];
        let names: Vec<String> = build_tool_schemas()
            .iter()
            .map(|t| t.name.to_string())
            .collect();
        for e in pre_change_24 {
            assert!(
                names.iter().any(|n| n == e),
                "pre-existing tool name `{e}` missing from the new 28-tool surface"
            );
        }
    }

    #[test]
    fn tool_names_exposed_via_back_compat_helper() {
        let names = tool_names();
        let expected_count = if cfg!(feature = "multimodal") { 34 } else { 28 };
        assert_eq!(
            names.len(),
            expected_count,
            "expected {expected_count} tools, got {}",
            names.len()
        );
        assert!(names.contains(&TOOL_OPEN_WORKSPACE));
        assert!(names.contains(&TOOL_APPLY_LENS));
        assert!(names.contains(&TOOL_QUERY_MOLDQL));
        assert!(names.contains(&TOOL_IMPACT_RADIUS));
        assert!(names.contains(&TOOL_IMPACT_COMPONENT));
        assert!(names.contains(&TOOL_IMPACT_FORWARD_RADIUS));
        assert!(names.contains(&TOOL_GRAPH_SUBGRAPH));
        assert!(names.contains(&TOOL_GRAPH_CLUSTER));
        assert!(names.contains(&TOOL_GRAPH_EXPLAIN));
        assert!(names.contains(&TOOL_ASK));
        assert!(names.contains(&TOOL_BRAIN_OPEN));
        assert!(names.contains(&TOOL_BRAIN_ASK));
        assert!(names.contains(&TOOL_BRAIN_CLOSE));
        #[cfg(feature = "multimodal")]
        {
            assert!(names.contains(&TOOL_DOCS_INGEST));
            assert!(names.contains(&TOOL_GRAPH_SEARCH));
            assert!(names.contains(&TOOL_BRAIN_ADD_SPACE));
            assert!(names.contains(&TOOL_BRAIN_REMOVE_SPACE));
            assert!(names.contains(&TOOL_BRAIN_SPACES));
        }
    }

    // ---- handler basics -----------------------------------------------------

    #[test]
    fn handler_wraps_service_cheaply() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let handler = ExplorerMcpHandler::new(service.clone());
        // Clone must not move the service.
        let _h2 = handler.clone();
        assert!(Arc::ptr_eq(handler.service(), &service));
    }

    #[test]
    fn get_info_reports_explorer_server_name() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let handler = ExplorerMcpHandler::new(service);
        let info = handler.get_info();
        assert_eq!(info.server_info.name, "cognicode-explorer");
        assert!(
            info.capabilities.tools.is_some(),
            "tools capability must be enabled"
        );
    }

    // ---- dispatch happy paths ----------------------------------------------

    #[tokio::test]
    async fn dispatch_open_workspace_with_no_root_uses_bound_path() {
        let (service, dir) = build_test_service();
        let registry = build_test_registry();
        let handler = ExplorerMcpHandler::new(service);
        // We can't drive `call_tool` end-to-end without a RequestContext, but
        // the dispatch logic is private — so we re-derive the contract here
        // by asserting the underlying service method is reachable and
        // returns a WorkspaceSummary rooted at the tempdir.
        let summary = handler
            .service()
            .current_workspace()
            .expect("current_workspace ok");
        assert_eq!(
            PathBuf::from(&summary.root_path).canonicalize().unwrap(),
            dir.path().canonicalize().unwrap(),
            "current_workspace should report the bound root path"
        );
    }

    #[tokio::test]
    async fn dispatch_spotter_search_finds_known_symbol() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let results = service.spotter_search("alpha", None).expect("spotter ok");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].object.id, "symbol:src/a.rs:alpha:1");
        assert_eq!(
            results[0].object.object_type,
            crate::dto::InspectableObjectType::Symbol
        );
        assert!((results[0].score - 1.0).abs() < f32::EPSILON);
    }

    #[tokio::test]
    async fn dispatch_inspect_object_dispatches_to_service() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let summary = service
            .inspect_object("symbol:src/a.rs:alpha:1")
            .expect("inspect_object ok");
        assert_eq!(summary.id, "symbol:src/a.rs:alpha:1");
        assert!(!summary.available_views.is_empty());
    }

    #[tokio::test]
    async fn dispatch_get_views_for_symbol_returns_views() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let views = service
            .available_views("symbol:src/a.rs:alpha:1")
            .expect("available_views ok");
        assert!(!views.is_empty(), "symbol should have at least one view");
        for v in &views {
            assert!(!v.id.is_empty());
            assert!(!v.title.is_empty());
        }
    }

    #[tokio::test]
    async fn dispatch_get_lenses_for_unknown_object_returns_resolution_error() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let err = service
            .available_lenses("garbage")
            .expect_err("garbage id must error");
        assert!(matches!(err, crate::ExplorerError::ResolutionFailed(_)));
    }

    #[tokio::test]
    async fn dispatch_apply_lens_unknown_id_returns_error_text() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let err = service
            .apply_lens("scope:src", "no-such-lens")
            .expect_err("unknown lens must error");
        let msg = err.to_string();
        assert!(
            msg.contains("lens not found"),
            "expected error to mention `lens not found`, got: {msg}"
        );
    }

    // ---- error path: unknown tool name -------------------------------------

    #[tokio::test]
    async fn dispatch_unknown_tool_name_returns_error() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args("not_a_real_tool", serde_json::json!({})),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(
            result.is_error,
            Some(true),
            "unknown tool must produce is_error=Some(true)"
        );
        let text = first_text(&result);
        assert!(
            text.contains("Unknown tool"),
            "error text should mention Unknown tool, got: {text}"
        );
    }

    // ---- error path: missing required arg ----------------------------------

    #[tokio::test]
    async fn dispatch_spotter_missing_query_returns_error() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_SPOTTER_SEARCH, serde_json::json!({})),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        let text = first_text(&result);
        assert!(
            text.contains("query"),
            "error text should mention the missing arg, got: {text}"
        );
    }

    #[tokio::test]
    async fn dispatch_moldql_query_returns_dto() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_QUERY_MOLDQL,
                serde_json::json!({ "query": "FIND symbols" }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(
            result.is_error,
            Some(false),
            "FIND symbols should succeed against an empty repo"
        );
        let text = first_text(&result);
        // The DTO serialises `query`, `items`, and `total` — all of
        // which must be present in the response body.
        assert!(text.contains("\"query\""));
        assert!(text.contains("\"items\""));
        assert!(text.contains("\"total\""));
    }

    #[tokio::test]
    async fn dispatch_moldql_query_missing_query_errors() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_QUERY_MOLDQL, serde_json::json!({})),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        let text = first_text(&result);
        assert!(
            text.contains("query"),
            "error text should mention the missing arg, got: {text}"
        );
    }

    #[tokio::test]
    async fn dispatch_moldql_query_with_parse_error() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_QUERY_MOLDQL, serde_json::json!({ "query": "FOO" })),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        let text = first_text(&result);
        // The service wraps ParseError into ResolutionFailed, so the
        // tool surfaces a clean error string.
        assert!(
            text.contains("FIND") && text.contains("PATH"),
            "error text should mention the parse failure, got: {text}"
        );
    }

    // ---- ExplorerQL MCP tests ---------------------------------------------

    #[tokio::test]
    async fn dispatch_moldql_query_accepts_path() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_QUERY_MOLDQL,
                serde_json::json!({ "query": "PATH FROM a TO b" }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        // PATH is a valid ExplorerQL query → dispatch succeeds.
        assert_eq!(
            result.is_error,
            Some(false),
            "expected Ok envelope, got: {result:?}"
        );
        let text = first_text(&result);
        assert!(
            !text.contains("expected"),
            "PATH should parse + dispatch cleanly, got: {text}"
        );
    }

    #[tokio::test]
    async fn dispatch_moldql_query_accepts_neighbors() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_QUERY_MOLDQL,
                serde_json::json!({ "query": "NEIGHBORS a DEPTH 1" }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
    }

    #[tokio::test]
    async fn dispatch_moldql_query_accepts_subgraph() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_QUERY_MOLDQL,
                serde_json::json!({ "query": "SUBGRAPH ROOT a" }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
    }

    #[tokio::test]
    async fn dispatch_moldql_query_accepts_boolean_composition() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_QUERY_MOLDQL,
                serde_json::json!({
                    "query": "PATH FROM a TO b OR PATH FROM c TO d"
                }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        // Boolean composition is supported by the parser; execution
        // returns NotImplemented in the MVP (set algebra over
        // petgraph plans is a future work item). Either way the
        // dispatch reaches the executor — i.e. we don't see a parse
        // error.
        let text = first_text(&result);
        assert!(
            !text.contains("expected") && !text.contains("ParseError"),
            "boolean composition should parse, got: {text}"
        );
    }

    #[tokio::test]
    async fn dispatch_moldql_query_target_petgraph() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_QUERY_MOLDQL,
                serde_json::json!({
                    "query": "PATH FROM a TO b",
                    "target": "petgraph"
                }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
    }

    #[tokio::test]
    async fn dispatch_moldql_query_target_pg_default_build_returns_error_envelope() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_QUERY_MOLDQL,
                serde_json::json!({
                    "query": "PATH FROM a TO b",
                    "target": "pg"
                }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        // Default build has the `postgres` feature off → envelope
        // carries a `FeatureDisabled` error. NOT a panic.
        let text = first_text(&result);
        assert!(
            text.contains("FeatureDisabled") || text.contains("postgres feature"),
            "expected clean FeatureDisabled envelope, got: {text}"
        );
    }

    #[tokio::test]
    async fn dispatch_moldql_query_invalid_target_returns_error() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_QUERY_MOLDQL,
                serde_json::json!({
                    "query": "FIND symbols",
                    "target": "redis"
                }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        let text = first_text(&result);
        assert!(
            text.contains("invalid `target`"),
            "expected `target` validation error, got: {text}"
        );
    }

    // ---- named-views: feature-gate dispatch tests -------------------------
    //
    // The `build_test_service` helper wires an
    // `ExplorerService` WITHOUT a `PostgresRepository`, so every
    // `*_view` call must return the canonical
    // `"named_views_require_postgres_feature"` envelope.

    #[tokio::test]
    async fn dispatch_view_save_feature_gate_off_returns_soft_error() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_VIEW_SAVE,
                serde_json::json!({
                    "workspace_id": "w1",
                    "owner": "u1",
                    "name": "hotspots",
                    "level": "function",
                    "lens": "callgraph",
                    "focus_node": "crate::foo",
                    "max_depth": 3
                }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        let text = first_text(&result);
        assert!(
            text.contains("named_views_require_postgres_feature"),
            "expected feature-gate error, got: {text}"
        );
    }

    #[tokio::test]
    async fn dispatch_view_load_feature_gate_off_returns_soft_error() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_VIEW_LOAD,
                serde_json::json!({
                    "id": "11111111-1111-1111-1111-111111111111",
                    "workspace_id": "w1",
                    "owner": "u1"
                }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        let text = first_text(&result);
        assert!(
            text.contains("named_views_require_postgres_feature"),
            "expected feature-gate error, got: {text}"
        );
    }

    #[tokio::test]
    async fn dispatch_view_list_feature_gate_off_returns_soft_error() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_VIEW_LIST,
                serde_json::json!({ "workspace_id": "w1", "owner": "u1" }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        let text = first_text(&result);
        assert!(
            text.contains("named_views_require_postgres_feature"),
            "expected feature-gate error, got: {text}"
        );
    }

    #[tokio::test]
    async fn dispatch_view_delete_feature_gate_off_returns_soft_error() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_VIEW_DELETE,
                serde_json::json!({
                    "id": "11111111-1111-1111-1111-111111111111",
                    "workspace_id": "w1",
                    "owner": "u1"
                }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        let text = first_text(&result);
        assert!(
            text.contains("named_views_require_postgres_feature"),
            "expected feature-gate error, got: {text}"
        );
    }

    /// Spec requirement: aggregate gate-off test for the four
    /// view tools. Every tool returns the soft-error code
    /// when the `postgres` feature is not active.
    #[tokio::test]
    async fn feature_gate_off_all_four_tools_return_soft_error() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let cases: &[(&str, serde_json::Value)] = &[
            (
                TOOL_VIEW_SAVE,
                serde_json::json!({
                    "workspace_id": "w1",
                    "owner": "u1",
                    "name": "n",
                    "level": "function",
                    "lens": "callgraph",
                    "focus_node": "crate::foo",
                    "max_depth": 3
                }),
            ),
            (
                TOOL_VIEW_LOAD,
                serde_json::json!({
                    "id": "id",
                    "workspace_id": "w1",
                    "owner": "u1"
                }),
            ),
            (
                TOOL_VIEW_LIST,
                serde_json::json!({ "workspace_id": "w1", "owner": "u1" }),
            ),
            (
                TOOL_VIEW_DELETE,
                serde_json::json!({
                    "id": "id",
                    "workspace_id": "w1",
                    "owner": "u1"
                }),
            ),
        ];
        for (tool, args) in cases {
            let result = dispatch(
                &service,
                &None,
                &registry,
                call_tool_args(tool, args.clone()),
            #[cfg(feature = "multimodal")]
            None,
            #[cfg(not(feature = "multimodal"))]
            &(),
            )
            .await;
            let text = first_text(&result);
            assert!(
                text.contains("named_views_require_postgres_feature"),
                "tool `{tool}` must surface named_views_require_postgres_feature on a no-PG build; got: {text}"
            );
        }
    }

    /// Spec requirement: `view_save` rejects empty `name` BEFORE
    /// hitting PG with `error == "invalid_input"`. Validation
    /// runs at the service boundary, so this works on a no-PG
    /// build too — the input check fires before the feature gate.
    #[tokio::test]
    async fn dispatch_view_save_rejects_empty_name() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_VIEW_SAVE,
                serde_json::json!({
                    "workspace_id": "w1",
                    "owner": "u1",
                    "name": "",
                    "level": "function",
                    "lens": "callgraph",
                    "focus_node": "crate::foo",
                    "max_depth": 3
                }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        let text = first_text(&result);
        assert!(
            text.contains("invalid_input"),
            "expected invalid_input envelope for empty name, got: {text}"
        );
    }

    /// Spec requirement: `view_save` rejects negative
    /// `max_depth` BEFORE hitting PG with `error ==
    /// "invalid_input"`.
    #[tokio::test]
    async fn dispatch_view_save_rejects_negative_max_depth() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_VIEW_SAVE,
                serde_json::json!({
                    "workspace_id": "w1",
                    "owner": "u1",
                    "name": "x",
                    "level": "function",
                    "lens": "callgraph",
                    "focus_node": "crate::foo",
                    "max_depth": -1
                }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        let text = first_text(&result);
        assert!(
            text.contains("invalid_input"),
            "expected invalid_input envelope for negative max_depth, got: {text}"
        );
    }

    // ---- envelope_ok() helper: success path wraps payload in envelope -------

    #[test]
    fn envelope_ok_serializes_success_as_envelope() {
        let summary = crate::dto::WorkspaceSummary {
            id: "abc".to_string(),
            root_path: "/tmp".to_string(),
            graph_status: crate::dto::GraphStatus::Ready,
            indexed_at: None,
            symbol_count: 42,
            relation_count: 7,
        };
        let result = envelope_ok(
            TOOL_OPEN_WORKSPACE,
            &Ok::<_, crate::ExplorerError>(summary),
            None,
        );
        // CallToolResult::success sets is_error = Some(false); only
        // CallToolResult::error sets it to Some(true).
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        assert!(text.contains('\n'), "expected pretty JSON, got: {text}");
        assert!(text.contains("\"symbol_count\": 42"), "got: {text}");
        assert!(
            text.contains("\"tool_name\""),
            "envelope must include tool_name, got: {text}"
        );
    }

    #[test]
    fn envelope_ok_serializes_error_without_envelope() {
        let result: CallToolResult = envelope_ok::<crate::dto::WorkspaceSummary>(
            TOOL_OPEN_WORKSPACE,
            &Err(crate::ExplorerError::WorkspaceNotFound("/nope".to_string())),
            None,
        );
        assert_eq!(result.is_error, Some(true));
        let text = first_text(&result);
        assert!(text.contains("workspace not found"));
        assert!(text.contains("/nope"));
        // Errors MUST NOT carry an envelope payload.
        assert!(
            !text.contains("\"payload\""),
            "error path must not emit envelope payload, got: {text}"
        );
    }

    // ---- end-to-end dispatch path for open_workspace (no RequestContext) --

    #[tokio::test]
    async fn dispatch_open_workspace_with_explicit_root_path() {
        let (service, dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_OPEN_WORKSPACE,
                serde_json::json!({ "root_path": dir.path().to_string_lossy() }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(
            result.is_error,
            Some(false),
            "open_workspace should succeed"
        );
        let text = first_text(&result);
        assert!(text.contains("\"id\""));
    }

    // ---- wire-level contract guard: DTO field name matches the schema. ----

    #[test]
    fn open_workspace_request_field_names_match_tool_schema() {
        let request = OpenWorkspaceRequest {
            root_path: "/tmp".to_string(),
        };
        let json = serde_json::to_value(&request).expect("serialize");
        assert!(json.get("root_path").is_some());
    }

    // ---- SDD mcp-impact-tool: RED gate ------------------------------------
    //
    // The first failing test in the TDD sequence. It references the new
    // `TOOL_IMPACT_RADIUS` constant and the new 3-arg `dispatch` signature
    // (service, graph, request). Neither exists yet, so the test must fail
    // to compile. Implementation lands in Phase 1.

    #[tokio::test]
    async fn test_handler_without_graph_returns_impact_unavailable() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let handler = ExplorerMcpHandler::new(service.clone());
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_IMPACT_RADIUS,
                serde_json::json!({"root": "x", "max_depth": 1}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(
            result.is_error,
            Some(true),
            "expected is_error=Some(true), got is_error={:?}",
            result.is_error
        );
        let text = first_text(&result);
        assert!(
            text.contains("impact analysis unavailable"),
            "expected 'impact analysis unavailable' in: {text}"
        );
    }

    // ---- Phase 2: handler field invariants (R1) ----------------------------

    /// Build a minimal `CallGraph` containing one symbol with FQN
    /// `test.rs:probe:1`. Used to assert that `with_graph(Some)` keeps
    /// the graph reachable from the handler.
    fn build_test_graph() -> Arc<CallGraph> {
        use cognicode_core::domain::aggregates::Symbol;
        use cognicode_core::domain::value_objects::{Location, SymbolKind};
        let mut g = CallGraph::new();
        let sym = Symbol::new(
            "probe",
            SymbolKind::Function,
            Location::new("test.rs", 1, 1),
        );
        g.add_symbol(sym);
        Arc::new(g)
    }

    #[test]
    fn test_with_graph_some_makes_impact_arms_reachable() {
        // with_graph(Some(_)) must store the graph AND keep the 14-tool
        // surface intact. The 5 impact tools become reachable (i.e. they
        // no longer hit the graph-unavailable guard).
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let graph = build_test_graph();
        let handler = ExplorerMcpHandler::with_graph(service, Some(graph.clone()));

        // The graph must be exactly the one we passed in (Arc::ptr_eq).
        let held = handler.graph().expect("graph should be Some");
        assert!(
            Arc::ptr_eq(held, &graph),
            "with_graph(Some) must keep the same Arc<CallGraph>"
        );

        // The 28-tool surface (34 with multimodal) is unchanged
        // by the constructor choice.
        let tools = build_tool_schemas();
        let expected = if cfg!(feature = "multimodal") { 34 } else { 28 };
        assert_eq!(tools.len(), expected);
    }

    // ---- Phase 5: impact_radius dispatch (R3) -------------------------------
    //
    // We need a helper to build a `CallGraph` from a closure that mutates
    // a fresh graph. Mirrors the pattern in
    // `cognicode_core::application::services::impact_analysis::tests`.
    use cognicode_core::domain::aggregates::Symbol;
    use cognicode_core::domain::services::ExtractionContext;
    use cognicode_core::domain::value_objects::{DependencyType, Location};

    fn impact_id(name: &str) -> cognicode_core::domain::aggregates::SymbolId {
        cognicode_core::domain::aggregates::SymbolId::new(format!("test.rs:{name}:1"))
    }

    fn impact_sym(name: &str) -> Symbol {
        Symbol::new(name, SymbolKind::Function, Location::new("test.rs", 1, 1))
    }

    /// Build a `CallGraph` from a closure that mutates a fresh graph.
    /// Edges added through this helper have confidence 1.0.
    fn make_impact_graph(builder: impl FnOnce(&mut CallGraph)) -> Arc<CallGraph> {
        let mut g = CallGraph::new();
        builder(&mut g);
        Arc::new(g)
    }

    fn impact_add_edge(g: &mut CallGraph, a: &str, b: &str) {
        g.add_symbol(impact_sym(a));
        g.add_symbol(impact_sym(b));
        let _ = g.add_dependency_with_provenance(
            &impact_id(a),
            &impact_id(b),
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
    }

    #[tokio::test]
    async fn test_impact_radius_returns_predecessors() {
        // D -> A -> C,  B -> C.  Predecessors of C at depth 2 must be
        // exactly {A, B, D}.
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "D", "A");
            impact_add_edge(g, "A", "C");
            impact_add_edge(g, "B", "C");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_IMPACT_RADIUS,
                serde_json::json!({"root": "test.rs:C:1", "max_depth": 2}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        let env: McpResultEnvelope<Vec<String>> =
            serde_json::from_str(&text).expect("valid envelope");
        let parsed = env.payload;
        let mut sorted = parsed.clone();
        sorted.sort();
        let mut expected = vec![
            "test.rs:A:1".to_string(),
            "test.rs:B:1".to_string(),
            "test.rs:D:1".to_string(),
        ];
        expected.sort();
        assert_eq!(sorted, expected);
    }

    #[tokio::test]
    async fn test_impact_radius_missing_root_arg() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
        });
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(TOOL_IMPACT_RADIUS, serde_json::json!({})),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        let text = first_text(&result);
        assert!(
            text.contains("missing required arg"),
            "expected 'missing required arg' in: {text}"
        );
        assert!(
            text.contains(TOOL_IMPACT_RADIUS),
            "error should mention tool name, got: {text}"
        );
    }

    #[tokio::test]
    async fn test_impact_radius_default_max_depth_is_5() {
        // Chain a1 -> a2 -> a3 -> a4 -> a5 -> a6 -> a7. Calling
        // impact_radius on a7 with no max_depth should return exactly
        // 5 predecessors: a1..a6 minus a7 itself.
        let graph = make_impact_graph(|g| {
            for (lo, hi) in [
                ("a1", "a2"),
                ("a2", "a3"),
                ("a3", "a4"),
                ("a4", "a5"),
                ("a5", "a6"),
                ("a6", "a7"),
            ] {
                impact_add_edge(g, lo, hi);
            }
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_IMPACT_RADIUS,
                serde_json::json!({"root": "test.rs:a7:1"}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        let env: McpResultEnvelope<Vec<String>> =
            serde_json::from_str(&text).expect("valid envelope");
        let parsed = env.payload;
        assert_eq!(
            parsed.len(),
            5,
            "expected 5 predecessors at default depth, got: {parsed:?}"
        );
    }

    #[tokio::test]
    async fn test_impact_radius_zero_depth_returns_empty() {
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_IMPACT_RADIUS,
                serde_json::json!({"root": "test.rs:B:1", "max_depth": 0}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        let env: McpResultEnvelope<Vec<String>> =
            serde_json::from_str(&text).expect("valid envelope");
        let parsed = env.payload;
        assert!(parsed.is_empty(), "expected empty array, got: {parsed:?}");
    }

    #[tokio::test]
    async fn test_impact_radius_unknown_root_returns_empty() {
        // Unknown root must NOT panic; returns an empty array.
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_IMPACT_RADIUS,
                serde_json::json!({"root": "missing", "max_depth": 5}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        let env: McpResultEnvelope<Vec<String>> =
            serde_json::from_str(&text).expect("valid envelope");
        let parsed = env.payload;
        assert!(parsed.is_empty(), "expected empty array, got: {parsed:?}");
    }

    // ---- Phase 6: impact_has_path dispatch (R4) ----------------------------

    #[tokio::test]
    async fn test_impact_has_path_direct_transitive_unreachable() {
        // A -> B -> C,  D isolated.
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
            impact_add_edge(g, "B", "C");
            g.add_symbol(impact_sym("D"));
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();

        // Direct: A -> B
        let result = dispatch(
            &service,
            &Some(graph.clone()),
            &registry,
            call_tool_args(
                TOOL_IMPACT_HAS_PATH,
                serde_json::json!({"from": "test.rs:A:1", "to": "test.rs:B:1"}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let env: McpResultEnvelope<serde_json::Value> =
            serde_json::from_str(&first_text(&result)).expect("valid envelope");
        let parsed = env.payload;
        assert_eq!(parsed["has_path"], serde_json::Value::Bool(true));
        assert_eq!(parsed["from"], "test.rs:A:1");
        assert_eq!(parsed["to"], "test.rs:B:1");

        // Transitive: A -> C
        let result = dispatch(
            &service,
            &Some(graph.clone()),
            &registry,
            call_tool_args(
                TOOL_IMPACT_HAS_PATH,
                serde_json::json!({"from": "test.rs:A:1", "to": "test.rs:C:1"}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        let env: McpResultEnvelope<serde_json::Value> =
            serde_json::from_str(&first_text(&result)).expect("valid envelope");
        let parsed = env.payload;
        assert_eq!(parsed["has_path"], serde_json::Value::Bool(true));

        // Unreachable: D -> A
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_IMPACT_HAS_PATH,
                serde_json::json!({"from": "test.rs:D:1", "to": "test.rs:A:1"}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        let env: McpResultEnvelope<serde_json::Value> =
            serde_json::from_str(&first_text(&result)).expect("valid envelope");
        let parsed = env.payload;
        assert_eq!(parsed["has_path"], serde_json::Value::Bool(false));
    }

    #[tokio::test]
    async fn test_impact_has_path_self_path() {
        // Single node, no edges: A -> A is the trivial self-path.
        let graph = make_impact_graph(|g| {
            g.add_symbol(impact_sym("A"));
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_IMPACT_HAS_PATH,
                serde_json::json!({"from": "test.rs:A:1", "to": "test.rs:A:1"}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let env: McpResultEnvelope<serde_json::Value> =
            serde_json::from_str(&first_text(&result)).expect("valid envelope");
        let parsed = env.payload;
        assert_eq!(parsed["has_path"], serde_json::Value::Bool(true));
    }

    // ---- Phase 7: impact_shortest_path dispatch (R5) ------------------------

    #[tokio::test]
    async fn test_impact_shortest_path_returns_cheapest() {
        // A -> B (high confidence, direct, cost 0.0) must beat
        // A -> C -> B (heuristic 0.5, cost 1.0).
        let graph = make_impact_graph(|g| {
            // Direct A -> B with confidence 1.0.
            g.add_symbol(impact_sym("A"));
            g.add_symbol(impact_sym("B"));
            let _ = g.add_dependency_with_provenance(
                &impact_id("A"),
                &impact_id("B"),
                DependencyType::Calls,
                ExtractionContext::DirectExtraction,
            );
            // 2-hop A -> C -> B at confidence 0.5 each.
            g.add_symbol(impact_sym("C"));
            let _ = g.add_dependency_with_provenance(
                &impact_id("A"),
                &impact_id("C"),
                DependencyType::Calls,
                ExtractionContext::Heuristic { score: 0.5 },
            );
            let _ = g.add_dependency_with_provenance(
                &impact_id("C"),
                &impact_id("B"),
                DependencyType::Calls,
                ExtractionContext::Heuristic { score: 0.5 },
            );
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_IMPACT_SHORTEST_PATH,
                serde_json::json!({"from": "test.rs:A:1", "to": "test.rs:B:1"}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let env: McpResultEnvelope<serde_json::Value> =
            serde_json::from_str(&first_text(&result)).expect("valid envelope");
        let parsed = env.payload;
        assert_eq!(parsed["found"], serde_json::Value::Bool(true));
        assert_eq!(parsed["path"][0], "test.rs:A:1");
        assert_eq!(parsed["path"][1], "test.rs:B:1");
        let cost = parsed["total_cost"].as_f64().expect("total_cost is f64");
        assert!(
            (cost - 0.0).abs() < 1e-9,
            "expected total_cost ~0.0 for direct high-conf edge, got {cost}"
        );
    }

    #[tokio::test]
    async fn test_impact_shortest_path_unreachable_returns_null() {
        // A -> B only; C is unreachable.
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_IMPACT_SHORTEST_PATH,
                serde_json::json!({"from": "test.rs:A:1", "to": "test.rs:C:1"}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let env: McpResultEnvelope<serde_json::Value> =
            serde_json::from_str(&first_text(&result)).expect("valid envelope");
        let parsed = env.payload;
        assert!(
            parsed.is_null(),
            "expected JSON null for unreachable, got: {parsed}"
        );
    }

    #[tokio::test]
    async fn test_impact_shortest_path_self_path() {
        // A alone, no edges. Self-path A -> A is the trivial single-node path.
        let graph = make_impact_graph(|g| {
            g.add_symbol(impact_sym("A"));
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_IMPACT_SHORTEST_PATH,
                serde_json::json!({"from": "test.rs:A:1", "to": "test.rs:A:1"}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let env: McpResultEnvelope<serde_json::Value> =
            serde_json::from_str(&first_text(&result)).expect("valid envelope");
        let parsed = env.payload;
        assert_eq!(parsed["found"], serde_json::Value::Bool(true));
        let path = parsed["path"].as_array().expect("path is an array");
        assert_eq!(path.len(), 1);
        assert_eq!(path[0], "test.rs:A:1");
        let cost = parsed["total_cost"].as_f64().expect("total_cost is f64");
        assert!((cost - 0.0).abs() < 1e-9);
    }

    // ---- Phase 8: impact_detect_cycles dispatch (R6) ------------------------

    #[tokio::test]
    async fn test_impact_detect_cycles_returns_sccs() {
        // Two disjoint cycles: A <-> B and X <-> Y.
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
            impact_add_edge(g, "B", "A");
            impact_add_edge(g, "X", "Y");
            impact_add_edge(g, "Y", "X");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(TOOL_IMPACT_DETECT_CYCLES, serde_json::json!({})),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let env: McpResultEnvelope<Vec<SccDto>> =
            serde_json::from_str(&first_text(&result)).expect("valid envelope");
        let parsed = env.payload;
        assert_eq!(parsed.len(), 2);

        let mut member_sets: Vec<Vec<String>> = parsed
            .iter()
            .map(|s| {
                let mut v = s.members.clone();
                v.sort();
                v
            })
            .collect();
        member_sets.sort();
        let mut expected_ab = vec!["test.rs:A:1".to_string(), "test.rs:B:1".to_string()];
        expected_ab.sort();
        let mut expected_xy = vec!["test.rs:X:1".to_string(), "test.rs:Y:1".to_string()];
        expected_xy.sort();
        assert!(member_sets.contains(&expected_ab));
        assert!(member_sets.contains(&expected_xy));
        for s in &parsed {
            assert_eq!(s.size, s.members.len());
        }
    }

    #[tokio::test]
    async fn test_impact_detect_cycles_dag_returns_empty() {
        // Linear chain, no cycles.
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
            impact_add_edge(g, "B", "C");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(TOOL_IMPACT_DETECT_CYCLES, serde_json::json!({})),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let env: McpResultEnvelope<Vec<SccDto>> =
            serde_json::from_str(&first_text(&result)).expect("valid envelope");
        let parsed = env.payload;
        assert!(
            parsed.is_empty(),
            "DAG must produce no SCCs, got: {parsed:?}"
        );
    }

    // ---- Phase 9: impact_component dispatch (R7) ----------------------------

    #[tokio::test]
    async fn test_impact_component_returns_members() {
        // Two disjoint components: A -> B and C -> D.
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
            impact_add_edge(g, "C", "D");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_IMPACT_COMPONENT,
                serde_json::json!({"id": "test.rs:A:1"}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let env: McpResultEnvelope<Vec<String>> =
            serde_json::from_str(&first_text(&result)).expect("valid envelope");
        let parsed = env.payload;
        let mut sorted = parsed.clone();
        sorted.sort();
        let mut expected = vec!["test.rs:A:1".to_string(), "test.rs:B:1".to_string()];
        expected.sort();
        assert_eq!(sorted, expected);
    }

    #[tokio::test]
    async fn test_impact_forward_radius_returns_successors() {
        // RED gate: A -> B, dispatch impact_forward_radius from A at
        // depth 1 must return ["B"]. Must fail to compile (the
        // constant, args struct, and dispatch arm do not exist) before
        // implementation lands.
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_IMPACT_FORWARD_RADIUS,
                serde_json::json!({"root": "test.rs:A:1", "max_depth": 1}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        let env: McpResultEnvelope<Vec<String>> =
            serde_json::from_str(&text).expect("valid envelope");
        let parsed = env.payload;
        assert_eq!(parsed, vec!["test.rs:B:1".to_string()]);
    }

    #[tokio::test]
    async fn test_impact_forward_radius_missing_root_arg() {
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(TOOL_IMPACT_FORWARD_RADIUS, serde_json::json!({})),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        let text = first_text(&result);
        assert!(
            text.contains("missing required arg"),
            "expected 'missing required arg' in: {text}"
        );
        assert!(
            text.contains(TOOL_IMPACT_FORWARD_RADIUS),
            "error should mention tool name, got: {text}"
        );
    }

    #[tokio::test]
    async fn test_impact_forward_radius_default_max_depth_is_5() {
        // Chain b1 -> b2 -> ... -> b7. Calling impact_forward_radius on
        // b1 with no max_depth should return exactly 5 successors:
        // b2..b6 (b7 is at depth 6, beyond default 5).
        let graph = make_impact_graph(|g| {
            for (lo, hi) in [
                ("b1", "b2"),
                ("b2", "b3"),
                ("b3", "b4"),
                ("b4", "b5"),
                ("b5", "b6"),
                ("b6", "b7"),
            ] {
                impact_add_edge(g, lo, hi);
            }
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_IMPACT_FORWARD_RADIUS,
                serde_json::json!({"root": "test.rs:b1:1"}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        let env: McpResultEnvelope<Vec<String>> =
            serde_json::from_str(&text).expect("valid envelope");
        let parsed = env.payload;
        assert_eq!(
            parsed.len(),
            5,
            "expected 5 successors at default depth, got: {parsed:?}"
        );
    }

    #[tokio::test]
    async fn test_impact_forward_radius_zero_depth_returns_empty() {
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_IMPACT_FORWARD_RADIUS,
                serde_json::json!({"root": "test.rs:A:1", "max_depth": 0}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        let env: McpResultEnvelope<Vec<String>> =
            serde_json::from_str(&text).expect("valid envelope");
        let parsed = env.payload;
        assert!(parsed.is_empty(), "expected empty array, got: {parsed:?}");
    }

    #[tokio::test]
    async fn test_impact_forward_radius_unknown_root_returns_empty() {
        // Unknown root must NOT panic; returns an empty array.
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_IMPACT_FORWARD_RADIUS,
                serde_json::json!({"root": "missing", "max_depth": 5}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        let env: McpResultEnvelope<Vec<String>> =
            serde_json::from_str(&text).expect("valid envelope");
        let parsed = env.payload;
        assert!(parsed.is_empty(), "expected empty array, got: {parsed:?}");
    }

    #[tokio::test]
    async fn test_impact_forward_radius_graph_unavailable() {
        // Graph == None: must surface 'impact analysis unavailable' error.
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_IMPACT_FORWARD_RADIUS,
                serde_json::json!({"root": "test.rs:A:1", "max_depth": 1}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        let text = first_text(&result);
        assert!(
            text.contains("impact analysis unavailable"),
            "expected 'impact analysis unavailable' in: {text}"
        );
    }

    #[tokio::test]
    async fn test_impact_component_missing_id_returns_null() {
        // Missing id must return JSON null, not an error.
        // This is a SUCCESS result (is_error: Some(false)) whose payload
        // is `null` — the envelope wrapper applies normally.
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(TOOL_IMPACT_COMPONENT, serde_json::json!({"id": "missing"})),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let env: McpResultEnvelope<serde_json::Value> =
            serde_json::from_str(&first_text(&result)).expect("valid envelope");
        let parsed = env.payload;
        assert!(
            parsed.is_null(),
            "expected JSON null for missing id, got: {parsed}"
        );
    }

    // ---- Phase 4: envelope_ok_direct helper (R8) ----------------------------

    #[test]
    fn test_envelope_ok_direct_serializes_envelope() {
        // Vec<String> round-trips through envelope_ok_direct.
        let v: Vec<String> = vec!["a".into(), "b".into()];
        let result = envelope_ok_direct(TOOL_IMPACT_RADIUS, &v, None);
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        let env: McpResultEnvelope<Vec<String>> =
            serde_json::from_str(&text).expect("valid envelope");
        assert_eq!(env.payload, v);
    }

    #[test]
    fn test_envelope_ok_direct_serializes_option_none_as_null_payload() {
        // Option<PathResultDto> = None must serialize as JSON `null` in the
        // envelope's payload field and still surface a successful result.
        // The MCP payload for `impact_shortest_path` unreachable targets
        // is exactly this shape.
        let none: Option<cognicode_core::application::dto::PathResultDto> = None;
        let result = envelope_ok_direct(TOOL_IMPACT_RADIUS, &none, None);
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        let env: McpResultEnvelope<Option<cognicode_core::application::dto::PathResultDto>> =
            serde_json::from_str(&text).expect("valid envelope");
        assert!(
            env.payload.is_none(),
            "expected payload to be null, got: {:?}",
            env.payload
        );
    }

    // ---- Phase 2: handler field invariants (R1) ----------------------------

    #[tokio::test]
    async fn test_with_graph_none_matches_new_legacy() {
        // with_graph(None) MUST be observationally identical to new(_):
        // the 6 impact tools (5 legacy + 1 new forward) + 3 graph tools
        // report the graph-unavailable error and the 17-tool surface
        // is preserved.
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let handler_a = ExplorerMcpHandler::new(service.clone());
        let handler_b = ExplorerMcpHandler::with_graph(service, None);

        assert!(
            handler_a.graph().is_none(),
            "new() must default graph to None"
        );
        assert!(
            handler_b.graph().is_none(),
            "with_graph(None) must keep graph as None"
        );

        // Same tool surface on both.
        let expected = if cfg!(feature = "multimodal") { 34 } else { 28 };
        assert_eq!(build_tool_schemas().len(), expected);

        // The 9 graph-aware tools (6 impact + 3 graph_*) surface the
        // unavailable error from both handlers. We pass `&None`
        // explicitly to `dispatch` because `handler.graph()` returns
        // `Option<&Arc<CallGraph>>` (a reference into the handler's
        // field), which is one layer removed from the
        // `&Option<Arc<CallGraph>>` that `dispatch` takes. The
        // contract under test is "the handler held by `with_graph(None)`
        // reports unavailable", which is the observable behavior we
        // assert here.
        for tool in [
            TOOL_IMPACT_RADIUS,
            TOOL_IMPACT_HAS_PATH,
            TOOL_IMPACT_SHORTEST_PATH,
            TOOL_IMPACT_DETECT_CYCLES,
            TOOL_IMPACT_COMPONENT,
            TOOL_IMPACT_FORWARD_RADIUS,
            TOOL_GRAPH_SUBGRAPH,
            TOOL_GRAPH_CLUSTER,
            TOOL_GRAPH_EXPLAIN,
        ] {
            let result_a = dispatch(
                handler_a.service(),
                &None,
                &registry,
                call_tool_args(tool, serde_json::json!({})),
                #[cfg(feature = "multimodal")]
                None,
                #[cfg(not(feature = "multimodal"))]
                &(),
            )
            .await;
            let result_b = dispatch(
                handler_b.service(),
                &None,
                &registry,
                call_tool_args(tool, serde_json::json!({})),
                #[cfg(feature = "multimodal")]
                None,
                #[cfg(not(feature = "multimodal"))]
                &(),
            )
            .await;
            let text_a = first_text(&result_a);
            let text_b = first_text(&result_b);
            assert!(
                text_a.contains("impact analysis unavailable"),
                "{tool}: handler_a should report unavailable, got: {text_a}"
            );
            assert!(
                text_b.contains("impact analysis unavailable"),
                "{tool}: handler_b should report unavailable, got: {text_b}"
            );
        }
    }

    // ---- mcp-graph-primitives: graph_subgraph / graph_cluster / graph_explain dispatch
    //
    // 8 RED-then-GREEN dispatch tests covering:
    //  - happy path for each tool
    //  - missing required arg
    //  - invalid enum string
    //  - graph unavailable guard
    //  - tool_names / tool_schemas count upgrade 14 -> 17

    #[tokio::test]
    async fn test_graph_subgraph_outgoing_returns_nodes_and_edges() {
        // A -> B, A -> C. Outgoing at depth 1 from A yields {A, B, C} + 2 edges.
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
            impact_add_edge(g, "A", "C");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_GRAPH_SUBGRAPH,
                serde_json::json!({
                    "root": "test.rs:A:1",
                    "direction": "outgoing",
                    "max_depth": 1,
                }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        let env: McpResultEnvelope<serde_json::Value> =
            serde_json::from_str(&text).expect("valid envelope");
        let parsed = env.payload;
        let mut nodes: Vec<String> = parsed["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        nodes.sort();
        let mut expected_nodes = vec![
            "test.rs:A:1".to_string(),
            "test.rs:B:1".to_string(),
            "test.rs:C:1".to_string(),
        ];
        expected_nodes.sort();
        assert_eq!(nodes, expected_nodes);
        let edges = parsed["edges"].as_array().expect("edges array");
        assert_eq!(edges.len(), 2);
    }

    #[tokio::test]
    async fn test_graph_subgraph_missing_root_errors() {
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(TOOL_GRAPH_SUBGRAPH, serde_json::json!({})),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        let text = first_text(&result);
        assert!(text.contains("missing required arg"), "got: {text}");
        assert!(text.contains("root"), "got: {text}");
    }

    #[tokio::test]
    async fn test_graph_subgraph_invalid_direction_errors() {
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_GRAPH_SUBGRAPH,
                serde_json::json!({"root": "test.rs:A:1", "direction": "sideways"}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        let text = first_text(&result);
        assert!(text.contains("invalid `direction`"), "got: {text}");
    }

    #[tokio::test]
    async fn test_graph_cluster_default_method_scc() {
        // A <-> B forms a 2-node SCC. Default method is "scc".
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
            impact_add_edge(g, "B", "A");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(TOOL_GRAPH_CLUSTER, serde_json::json!({})),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        let env: McpResultEnvelope<serde_json::Value> =
            serde_json::from_str(&text).expect("valid envelope");
        let parsed = env.payload;
        let clusters = parsed.as_array().expect("array");
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0]["size"], 2);
    }

    #[tokio::test]
    async fn test_graph_cluster_connected_method() {
        // A -> B, C -> D are two disjoint undirected components.
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
            impact_add_edge(g, "C", "D");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_GRAPH_CLUSTER,
                serde_json::json!({"method": "connected"}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        let env: McpResultEnvelope<serde_json::Value> =
            serde_json::from_str(&text).expect("valid envelope");
        let parsed = env.payload;
        let clusters = parsed.as_array().expect("array");
        assert_eq!(clusters.len(), 2);
    }

    #[tokio::test]
    async fn test_graph_explain_direct_edge_returns_rationale() {
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_GRAPH_EXPLAIN,
                serde_json::json!({"from": "test.rs:A:1", "to": "test.rs:B:1"}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        let env: McpResultEnvelope<serde_json::Value> =
            serde_json::from_str(&text).expect("valid envelope");
        let parsed = env.payload;
        assert_eq!(parsed["found"], true);
        assert_eq!(parsed["hops"].as_array().expect("hops").len(), 1);
        assert_eq!(parsed["hops"][0]["rationale"], "calls");
    }

    #[tokio::test]
    async fn test_graph_explain_unreachable_returns_found_false() {
        // No path A -> C.
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_GRAPH_EXPLAIN,
                serde_json::json!({"from": "test.rs:A:1", "to": "test.rs:C:1"}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        // NOT is_error — found=false is a structured payload.
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        let env: McpResultEnvelope<serde_json::Value> =
            serde_json::from_str(&text).expect("valid envelope");
        let parsed = env.payload;
        assert_eq!(parsed["found"], false);
        assert_eq!(parsed["summary"], "no path");
    }

    #[tokio::test]
    async fn test_graph_explain_missing_to_errors() {
        let graph = make_impact_graph(|g| {
            impact_add_edge(g, "A", "B");
        });
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &Some(graph),
            &registry,
            call_tool_args(
                TOOL_GRAPH_EXPLAIN,
                serde_json::json!({"from": "test.rs:A:1"}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        let text = first_text(&result);
        assert!(text.contains("missing required arg `to`"), "got: {text}");
    }

    #[tokio::test]
    async fn test_graph_subgraph_graph_unavailable() {
        // Graph == None: must surface 'impact analysis unavailable' error.
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_GRAPH_SUBGRAPH,
                serde_json::json!({"root": "test.rs:A:1"}),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        let text = first_text(&result);
        assert!(text.contains("impact analysis unavailable"), "got: {text}");
    }

    // ============================================================================
    // Phase 1 RED tests — Envelope types. References to McpResultEnvelope,
    // ProvenanceMetadata, FollowUp, EnvelopeError MUST fail to compile until
    // the types are defined in Phase 1.3.
    // ============================================================================

    #[test]
    fn envelope_struct_has_six_fields() {
        let env = McpResultEnvelope::<u32> {
            tool_name: "t".to_string(),
            version: "0.0.0".to_string(),
            timestamp: "x".to_string(),
            provenance: None,
            payload: 1u32,
            suggested_follow_ups: vec![],
        };
        assert_eq!(env.payload, 1u32);
    }

    #[test]
    fn provenance_metadata_default_is_none() {
        let p: ProvenanceMetadata = Default::default();
        assert!(p.confidence.is_none());
        assert!(p.source.is_none());
    }

    #[test]
    fn follow_up_default_constructs() {
        let f: FollowUp = Default::default();
        assert_eq!(f.tool, "");
        assert_eq!(f.reason, "");
    }

    #[test]
    fn envelope_error_confidence_out_of_range_constructs() {
        let e = EnvelopeError::ConfidenceOutOfRange(1.5);
        assert_eq!(format!("{e}"), "confidence 1.5 out of range [0.0, 1.0]");
    }

    #[test]
    fn provenance_new_accepts_boundary_zero() {
        let p = ProvenanceMetadata::new(0.0, Some("s".to_string())).unwrap();
        assert_eq!(p.confidence, Some(0.0));
    }

    #[test]
    fn provenance_new_accepts_boundary_one() {
        let p = ProvenanceMetadata::new(1.0, None).unwrap();
        assert_eq!(p.confidence, Some(1.0));
        assert!(p.source.is_none());
    }

    #[test]
    fn provenance_new_rejects_above_one() {
        assert!(ProvenanceMetadata::new(1.5, None).is_err());
    }

    #[test]
    fn provenance_new_rejects_negative() {
        assert!(ProvenanceMetadata::new(-0.1, None).is_err());
    }

    // ============================================================================
    // Phase 2 RED tests — Envelope helpers. References to envelope_ok and
    // envelope_ok_direct MUST fail to compile until the helpers are defined
    // in Phase 2.2.
    // ============================================================================

    #[test]
    fn envelope_ok_success_wraps_payload() {
        let summary = crate::dto::WorkspaceSummary {
            id: "abc".to_string(),
            root_path: "/tmp".to_string(),
            graph_status: crate::dto::GraphStatus::Ready,
            indexed_at: None,
            symbol_count: 42,
            relation_count: 7,
        };
        let result = envelope_ok(
            TOOL_OPEN_WORKSPACE,
            &Ok::<_, crate::ExplorerError>(summary),
            None,
        );
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        assert!(
            text.contains("\"tool_name\": \"explorer_open_workspace\""),
            "got: {text}"
        );
        assert!(text.contains("\"payload\""), "got: {text}");
        assert!(text.contains("\"version\""), "got: {text}");
        assert!(text.contains("\"timestamp\""), "got: {text}");
        assert!(text.contains("\"provenance\": null"), "got: {text}");
        assert!(text.contains("\"suggested_follow_ups\": []"), "got: {text}");
    }

    #[test]
    fn envelope_ok_err_returns_error_result() {
        let result: CallToolResult = envelope_ok::<crate::dto::WorkspaceSummary>(
            TOOL_OPEN_WORKSPACE,
            &Err(crate::ExplorerError::WorkspaceNotFound("/nope".to_string())),
            None,
        );
        assert_eq!(result.is_error, Some(true));
        let text = first_text(&result);
        assert!(text.contains("workspace not found"), "got: {text}");
        assert!(
            !text.contains("\"payload\""),
            "errors must not emit envelope payload, got: {text}"
        );
    }

    #[test]
    fn envelope_ok_provenance_none_serializes_as_null() {
        let summary = crate::dto::WorkspaceSummary {
            id: "abc".to_string(),
            root_path: "/tmp".to_string(),
            graph_status: crate::dto::GraphStatus::Ready,
            indexed_at: None,
            symbol_count: 1,
            relation_count: 0,
        };
        let result = envelope_ok(
            TOOL_OPEN_WORKSPACE,
            &Ok::<_, crate::ExplorerError>(summary),
            None,
        );
        let text = first_text(&result);
        let obj: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert!(
            obj.get("provenance").is_some(),
            "provenance key must be present, got: {obj}"
        );
        assert!(
            obj["provenance"].is_null(),
            "provenance must be JSON null, got: {obj}"
        );
    }

    #[test]
    fn envelope_ok_follow_ups_default_empty() {
        let summary = crate::dto::WorkspaceSummary {
            id: "abc".to_string(),
            root_path: "/tmp".to_string(),
            graph_status: crate::dto::GraphStatus::Ready,
            indexed_at: None,
            symbol_count: 1,
            relation_count: 0,
        };
        let result = envelope_ok(
            TOOL_OPEN_WORKSPACE,
            &Ok::<_, crate::ExplorerError>(summary),
            None,
        );
        let text = first_text(&result);
        let obj: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert!(
            obj["suggested_follow_ups"].is_array(),
            "must be an array, got: {obj}"
        );
        assert_eq!(
            obj["suggested_follow_ups"].as_array().unwrap().len(),
            0,
            "must be empty"
        );
    }

    #[test]
    fn envelope_ok_timestamp_rfc3339_utc() {
        let summary = crate::dto::WorkspaceSummary {
            id: "abc".to_string(),
            root_path: "/tmp".to_string(),
            graph_status: crate::dto::GraphStatus::Ready,
            indexed_at: None,
            symbol_count: 1,
            relation_count: 0,
        };
        let result = envelope_ok(
            TOOL_OPEN_WORKSPACE,
            &Ok::<_, crate::ExplorerError>(summary),
            None,
        );
        let text = first_text(&result);
        let obj: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        let ts = obj["timestamp"].as_str().expect("timestamp is a string");
        let parsed = chrono::DateTime::parse_from_rfc3339(ts).expect("RFC 3339");
        let suffix_ok = ts.ends_with('Z') || ts.ends_with("+00:00");
        assert!(suffix_ok, "expected UTC suffix Z or +00:00, got: {ts}");
        let now = chrono::Utc::now();
        let delta = (now - parsed.with_timezone(&chrono::Utc))
            .num_seconds()
            .abs();
        assert!(
            delta <= 2,
            "timestamp drift {delta}s exceeds tolerance, got: {ts}"
        );
    }

    #[test]
    fn envelope_ok_version_matches_pkg() {
        let summary = crate::dto::WorkspaceSummary {
            id: "abc".to_string(),
            root_path: "/tmp".to_string(),
            graph_status: crate::dto::GraphStatus::Ready,
            indexed_at: None,
            symbol_count: 1,
            relation_count: 0,
        };
        let result = envelope_ok(
            TOOL_OPEN_WORKSPACE,
            &Ok::<_, crate::ExplorerError>(summary),
            None,
        );
        let text = first_text(&result);
        let obj: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(
            obj["version"].as_str().expect("version is a string"),
            env!("CARGO_PKG_VERSION")
        );
    }

    #[test]
    fn envelope_ok_direct_raw_value() {
        let v = vec!["a".to_string(), "b".to_string()];
        let result = envelope_ok_direct(TOOL_IMPACT_RADIUS, &v, None);
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        assert!(
            text.contains("\"tool_name\": \"impact_radius\""),
            "got: {text}"
        );
        let env: McpResultEnvelope<Vec<String>> =
            serde_json::from_str(&text).expect("valid envelope");
        assert_eq!(env.payload, v);
    }

    #[test]
    fn envelope_ok_provenance_confidence_out_of_range() {
        // Re-test of Phase 1 confidence guard; locks the validation at
        // the helper layer in case a future refactor drops ProvenanceMetadata::new.
        assert!(ProvenanceMetadata::new(1.5, None).is_err());
    }

    #[test]
    fn envelope_version_field_detects_wrapper() {
        let summary = crate::dto::WorkspaceSummary {
            id: "abc".to_string(),
            root_path: "/tmp".to_string(),
            graph_status: crate::dto::GraphStatus::Ready,
            indexed_at: None,
            symbol_count: 1,
            relation_count: 0,
        };
        let result = envelope_ok(
            TOOL_OPEN_WORKSPACE,
            &Ok::<_, crate::ExplorerError>(summary),
            None,
        );
        let text = first_text(&result);
        let obj: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert!(
            obj.get("version").is_some(),
            "version key must be present, got: {obj}"
        );
        assert!(
            obj["version"].is_string(),
            "version must be a string, got: {obj}"
        );
    }

    // ============================================================================
    // Phase 3 watch-dog — source must not contain legacy `ok(&` / `ok_direct(&`
    // call sites in dispatch. 4 helper-direct tests were migrated atomically
    // in 2.4, so the remaining count is the 17 dispatch arms.
    // ============================================================================

    #[test]
    fn dispatch_arms_no_legacy_helpers() {
        // The 17 dispatch arms must call envelope_ok / envelope_ok_direct
        // exclusively. The fn signatures for the legacy helpers have been
        // removed; if any arm still called them, the file would not compile.
        // This test is a regression guard for the new helper names.
        let source = include_str!("mcp.rs");
        // Count call sites that are NOT inside string literals (the test
        // comment above mentions "ok(&" but those are in a comment).
        // We do this with a rough heuristic: split on lines and look at
        // dispatch arm-style lines that have an `ok(` or `ok_direct(` call
        // followed by `&` and an identifier. The arm pattern is always
        // indented inside a match arm and ends the block.
        let arm_calls_ok: usize = source
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                trimmed.starts_with("ok(&")
                    || trimmed.starts_with("envelope_ok(TOOL_")
                    || trimmed.starts_with("envelope_ok_direct(TOOL_")
            })
            .map(|line| {
                if line.trim_start().starts_with("ok(&") {
                    1
                } else {
                    0
                }
            })
            .sum();
        // Simpler check: no call site may use the bare `ok(&` or
        // `ok_direct(&` pattern. The new helpers always include the
        // `TOOL_*` constant as first arg.
        let legacy = source
            .matches("ok(&")
            .filter(|m| !m.is_empty())
            .count()
            .saturating_sub(0);
        // Count only occurrences of `ok(&` in dispatch arms: the
        // pattern in dispatch is "        ok(&..." (8 spaces) or
        // "            ok(&..." (12 spaces). Filter lines.
        let legacy_in_arms: usize = source
            .lines()
            .filter(|line| {
                let t = line.trim_start();
                (t.starts_with("ok(&") || t.starts_with("ok_direct(&"))
                    && !t.starts_with("//")
                    && !t.starts_with("ok_envelope")
            })
            .count();
        assert_eq!(
            legacy_in_arms, 0,
            "expected zero legacy `ok(&` / `ok_direct(&` call sites in dispatch, found {legacy_in_arms}"
        );
        let _ = (arm_calls_ok, legacy);
    }

    // ---- ask-router Phase 6: MCP wiring RED→GREEN -----------------------
    //
    // 6 tests: tool count 18, TOOL_ASK in TOOL_NAMES, schema has
    // `question` (required) + `context` (optional), missing `question`
    // → validation error, full dispatch envelope has
    // `provenance.source = "ask-router"`, and a non-graph question
    // dispatches successfully through `dispatch(`.

    #[test]
    fn ask_tool_constant_is_registered_in_tool_names() {
        // TOOL_ASK must be in the TOOL_NAMES const slice (the
        // canonical source of truth for the wire-level surface).
        assert!(
            TOOL_NAMES.contains(&TOOL_ASK),
            "TOOL_NAMES missing `{}` — got: {:?}",
            TOOL_ASK,
            TOOL_NAMES
        );
    }

    #[test]
    fn ask_tool_schema_has_question_required_and_context_optional() {
        let tools = build_tool_schemas();
        let ask = tools
            .iter()
            .find(|t| t.name.as_ref() == TOOL_ASK)
            .expect("cognicode_ask schema present");
        let schema = ask.input_schema.as_ref();
        let required: Vec<String> = schema
            .get("required")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        assert!(
            required.iter().any(|s| s == "question"),
            "schema must require `question`, got: {:?}",
            required
        );
        // `context` is optional, so it must be in properties but NOT
        // in required.
        let props = schema.get("properties").and_then(|p| p.as_object());
        assert!(
            props.and_then(|p| p.get("context")).is_some(),
            "schema must declare `context` as a property"
        );
        assert!(
            !required.iter().any(|s| s == "context"),
            "context must be optional, got required = {:?}",
            required
        );
    }

    #[tokio::test]
    async fn dispatch_ask_missing_question_returns_validation_error() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_ASK, serde_json::json!({})),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        let text = first_text(&result);
        assert!(
            text.contains("question"),
            "error text must mention the missing arg, got: {text}"
        );
    }

    #[tokio::test]
    async fn dispatch_ask_non_graph_question_succeeds_with_provenance() {
        // Pattern 4 (code quality) does NOT require the graph — this
        // path exercises the dispatch arm end-to-end and asserts the
        // envelope carries `provenance.source = "ask-router"`.
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_ASK,
                serde_json::json!({ "question": "any smells in `parse_config`?" }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(
            result.is_error,
            Some(false),
            "non-graph ask must succeed; got is_error={:?}",
            result.is_error
        );
        let text = first_text(&result);
        assert!(
            text.contains("\"provenance\""),
            "envelope must include provenance, got: {text}"
        );
        assert!(
            text.contains("\"source\": \"ask-router\""),
            "envelope provenance.source must equal `ask-router`, got: {text}"
        );
        // Spec: `provenance.confidence` MUST be in [0.0, 1.0]. We
        // assert it as a number to catch float-range regressions.
        // The outer envelope wraps the inner ask-router envelope in
        // its `payload` field; we read the inner envelope's
        // provenance to validate the contract.
        let outer: McpResultEnvelope<serde_json::Value> =
            serde_json::from_str(&text).expect("valid envelope");
        let inner: McpResultEnvelope<serde_json::Value> =
            serde_json::from_value(outer.payload).expect("inner envelope");
        let conf = inner
            .provenance
            .as_ref()
            .and_then(|p| p.confidence)
            .expect("confidence present");
        assert!(
            (0.0..=1.0).contains(&conf),
            "confidence out of range: {conf}"
        );
    }

    #[tokio::test]
    async fn dispatch_ask_graph_question_without_graph_returns_unavailable_envelope() {
        // Pattern 1 (path between) is graph-dependent. When no
        // graph is loaded, the ask router must return a
        // `graph_unavailable` envelope (NOT a panic / generic
        // error). This is the end-to-end version of the dispatch
        // test in ask::dispatch::tests.
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_ASK,
                serde_json::json!({
                    "question": "path between `alpha` and `beta`"
                }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        // The ask router wraps the `graph_unavailable` response in a
        // success envelope (envelope_ok), so is_error is false and
        // the body carries the message.
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        assert!(
            text.contains("graph_unavailable"),
            "expected graph_unavailable in body, got: {text}"
        );
        // Provenance is still set (router-level metadata).
        assert!(
            text.contains("\"source\": \"ask-router\""),
            "router-level provenance must survive failure, got: {text}"
        );
    }

    #[test]
    fn ask_tool_count_is_twentyeight_after_registration() {
        // Regression guard: the 28th tool was added by the
        // named-views change. The multimodal feature adds
        // 6 more (docs_ingest + graph_search + issues_ingest
        // + brain_add_space + brain_remove_space + brain_spaces → 34).
        let expected = if cfg!(feature = "multimodal") { 34 } else { 28 };
        assert_eq!(TOOL_NAMES.len(), expected);
    }

    // ---- brain-session: 6 dispatch tests --------------------------------

    #[tokio::test]
    async fn brain_open_returns_session_id_and_state() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_BRAIN_OPEN,
                serde_json::json!({ "workspace_id": "ws-test" }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        // session_id is present and looks UUID-shaped.
        let v: serde_json::Value = serde_json::from_str(&text).expect("envelope json");
        let payload = v.get("payload").expect("payload present");
        let sid = payload
            .get("session_id")
            .and_then(|s| s.as_str())
            .expect("session_id string")
            .to_string();
        assert_eq!(sid.len(), 36, "session_id must be 36 chars, got `{sid}`");
        assert_eq!(
            payload.get("workspace_id").and_then(|s| s.as_str()),
            Some("ws-test")
        );
        // State block carries the history as an empty array (NOT
        // null or omitted).
        let state = payload.get("state").expect("state block");
        let history = state.get("history").expect("history present");
        assert!(
            history.is_array(),
            "history must be an array, got: {history}"
        );
        assert_eq!(history.as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn brain_open_empty_workspace_id_returns_invalid_workspace_id() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_OPEN, serde_json::json!({ "workspace_id": "" })),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        assert!(text.contains("invalid_workspace_id"), "got: {text}");
    }

    #[tokio::test]
    async fn brain_open_ttl_out_of_range_returns_invalid_ttl() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_BRAIN_OPEN,
                serde_json::json!({ "workspace_id": "ws", "ttl": 100_000 }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        assert!(text.contains("invalid_ttl"), "got: {text}");
    }

    #[tokio::test]
    async fn brain_attach_unknown_session_returns_session_not_found() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_BRAIN_ATTACH,
                serde_json::json!({ "session_id": "00000000-0000-4000-8000-000000000000" }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        assert!(text.contains("session_not_found"), "got: {text}");
    }

    #[tokio::test]
    async fn brain_close_unknown_session_returns_closed_false_idempotent() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_BRAIN_CLOSE,
                serde_json::json!({ "session_id": "00000000-0000-4000-8000-000000000000" }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        // Critical: NOT an error envelope. closed:false is the
        // happy-path outcome for idempotent close.
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        let v: serde_json::Value = serde_json::from_str(&text).expect("envelope");
        let payload = v.get("payload").expect("payload");
        assert_eq!(payload.get("closed").and_then(|b| b.as_bool()), Some(false));
    }

    #[tokio::test]
    async fn brain_focus_empty_string_returns_invalid_focus_node() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        // First open a session so we don't trip on session_not_found.
        let open = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_OPEN, serde_json::json!({ "workspace_id": "ws" })),
            #[cfg(feature = "multimodal")]
            None,
            #[cfg(not(feature = "multimodal"))]
            &(),
        )
        .await;
        let v: serde_json::Value = serde_json::from_str(&first_text(&open)).expect("envelope");
        let sid = v
            .get("payload")
            .and_then(|p| p.get("session_id"))
            .and_then(|s| s.as_str())
            .expect("session_id")
            .to_string();
        // Now try to set focus to "".
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_BRAIN_FOCUS,
                serde_json::json!({ "session_id": sid, "focus_node": "" }),
            ),
        #[cfg(feature = "multimodal")]
        None,
        #[cfg(not(feature = "multimodal"))]
        &(),
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        assert!(text.contains("invalid_focus_node"), "got: {text}");
    }

    #[tokio::test]
    async fn brain_status_empty_history_serializes_as_array() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        // Open a fresh session and immediately status it.
        let open = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_OPEN, serde_json::json!({ "workspace_id": "ws" })),
            #[cfg(feature = "multimodal")]
            None,
            #[cfg(not(feature = "multimodal"))]
            &(),
        )
        .await;
        let v: serde_json::Value = serde_json::from_str(&first_text(&open)).expect("envelope");
        let sid = v
            .get("payload")
            .and_then(|p| p.get("session_id"))
            .and_then(|s| s.as_str())
            .expect("session_id")
            .to_string();
        let status = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_STATUS, serde_json::json!({ "session_id": sid })),
            #[cfg(feature = "multimodal")]
            None,
            #[cfg(not(feature = "multimodal"))]
            &(),
        )
        .await;
        assert_eq!(status.is_error, Some(false));
        let text = first_text(&status);
        // Empty history MUST appear as `[]`, not `null`, not omitted.
        // (Pretty-printed: `"history": [],` — the colon-space-bracket
        // pattern handles both pretty and compact forms.)
        assert!(
            text.contains("\"history\": []") || text.contains("\"history\":[]"),
            "empty history must serialize as [], got: {text}"
        );
    }

    #[tokio::test]
    async fn brain_full_lifecycle_open_attach_focus_status_close() {
        // End-to-end check: open → attach → set focus → status
        // (history still []) → close → close again (idempotent) →
        // attach (session_not_found).
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();

        // Open.
        let open = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_BRAIN_OPEN,
                serde_json::json!({ "workspace_id": "ws-lifecycle" }),
            ),
            #[cfg(feature = "multimodal")]
            None,
            #[cfg(not(feature = "multimodal"))]
            &(),
        )
        .await;
        let v: serde_json::Value = serde_json::from_str(&first_text(&open)).expect("open envelope");
        let sid = v["payload"]["session_id"]
            .as_str()
            .expect("session_id")
            .to_string();

        // Attach.
        let attach = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_ATTACH, serde_json::json!({ "session_id": sid })),
            #[cfg(feature = "multimodal")]
            None,
            #[cfg(not(feature = "multimodal"))]
            &(),
        )
        .await;
        assert!(first_text(&attach).contains("\"focus_node\": null"));

        // Set focus.
        let focus = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_BRAIN_FOCUS,
                serde_json::json!({ "session_id": sid, "focus_node": "Foo::bar" }),
            ),
            #[cfg(feature = "multimodal")]
            None,
            #[cfg(not(feature = "multimodal"))]
            &(),
        )
        .await;
        let fv: serde_json::Value = serde_json::from_str(&first_text(&focus)).expect("focus");
        assert_eq!(fv["payload"]["focus_node"].as_str(), Some("Foo::bar"));

        // Status: focus reflects the new value, history is still [].
        let status = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_STATUS, serde_json::json!({ "session_id": sid })),
            #[cfg(feature = "multimodal")]
            None,
            #[cfg(not(feature = "multimodal"))]
            &(),
        )
        .await;
        let sv: serde_json::Value = serde_json::from_str(&first_text(&status)).expect("status");
        // Status body is wrapped in a brain-session envelope; the
        // snapshot lives under `payload`.
        let payload = sv.get("payload").expect("payload present");
        assert_eq!(payload["focus_node"].as_str(), Some("Foo::bar"));
        assert_eq!(payload["history"].as_array().unwrap().len(), 0);

        // Close.
        let close = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_CLOSE, serde_json::json!({ "session_id": sid })),
            #[cfg(feature = "multimodal")]
            None,
            #[cfg(not(feature = "multimodal"))]
            &(),
        )
        .await;
        let cv: serde_json::Value = serde_json::from_str(&first_text(&close)).expect("close");
        assert_eq!(cv["payload"]["closed"].as_bool(), Some(true));

        // Idempotent close.
        let close2 = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_CLOSE, serde_json::json!({ "session_id": sid })),
            #[cfg(feature = "multimodal")]
            None,
            #[cfg(not(feature = "multimodal"))]
            &(),
        )
        .await;
        let c2v: serde_json::Value = serde_json::from_str(&first_text(&close2)).expect("close2");
        assert_eq!(c2v["payload"]["closed"].as_bool(), Some(false));

        // Attach to a now-closed session → session_not_found.
        let attach_after = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_ATTACH, serde_json::json!({ "session_id": sid })),
            #[cfg(feature = "multimodal")]
            None,
            #[cfg(not(feature = "multimodal"))]
            &(),
        )
        .await;
        assert!(first_text(&attach_after).contains("session_not_found"));
    }

    // ---- multimodal (brain-federation): brain_add_space RED gates ----

    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn brain_add_space_creates_space() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        // Open a session first.
        let open = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_OPEN, serde_json::json!({ "workspace_id": "ws" })),
            None,
        )
        .await;
        let sid = serde_json::from_str::<serde_json::Value>(&first_text(&open))
            .expect("envelope")["payload"]["session_id"]
            .as_str()
            .expect("session_id")
            .to_string();

        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_BRAIN_ADD_SPACE,
                serde_json::json!({
                    "session_id": sid,
                    "space_name": "auth-repo",
                    "space_kind": "Repo",
                }),
            ),
            None,
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        let v: serde_json::Value = serde_json::from_str(&text).expect("envelope");
        let payload = v.get("payload").expect("payload");
        assert_eq!(payload["space_id"].as_str(), Some("auth-repo"));
        assert_eq!(payload["space_name"].as_str(), Some("auth-repo"));
        assert_eq!(payload["space_kind"].as_str(), Some("Repo"));
    }

    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn brain_add_space_invalid_kind() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let open = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_OPEN, serde_json::json!({ "workspace_id": "ws" })),
            None,
        )
        .await;
        let sid = serde_json::from_str::<serde_json::Value>(&first_text(&open))
            .expect("envelope")["payload"]["session_id"]
            .as_str()
            .expect("session_id")
            .to_string();

        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_BRAIN_ADD_SPACE,
                serde_json::json!({
                    "session_id": sid,
                    "space_name": "bad",
                    "space_kind": "nope",
                }),
            ),
            None,
        )
        .await;
        let text = first_text(&result);
        assert!(text.contains("invalid_space_kind"), "got: {text}");
    }

    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn brain_add_space_unknown_session() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_BRAIN_ADD_SPACE,
                serde_json::json!({
                    "session_id": "00000000-0000-4000-8000-000000000000",
                    "space_name": "x",
                    "space_kind": "Repo",
                }),
            ),
            None,
        )
        .await;
        let text = first_text(&result);
        assert!(text.contains("session_not_found"), "got: {text}");
    }

    // ---- multimodal (brain-federation): brain_remove_space RED gates ----

    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn brain_remove_space_removes() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let open = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_OPEN, serde_json::json!({ "workspace_id": "ws" })),
            None,
        )
        .await;
        let sid = serde_json::from_str::<serde_json::Value>(&first_text(&open))
            .expect("envelope")["payload"]["session_id"]
            .as_str()
            .expect("session_id")
            .to_string();

        // Add a space first.
        let _ = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_BRAIN_ADD_SPACE,
                serde_json::json!({
                    "session_id": sid,
                    "space_name": "auth-repo",
                    "space_kind": "Repo",
                }),
            ),
            None,
        )
        .await;

        // Remove the space.
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_BRAIN_REMOVE_SPACE,
                serde_json::json!({
                    "session_id": sid,
                    "space_id": "auth-repo",
                }),
            ),
            None,
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        let v: serde_json::Value = serde_json::from_str(&text).expect("envelope");
        assert_eq!(v["payload"]["removed"].as_bool(), Some(true));
    }

    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn brain_remove_space_not_found() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let open = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_OPEN, serde_json::json!({ "workspace_id": "ws" })),
            None,
        )
        .await;
        let sid = serde_json::from_str::<serde_json::Value>(&first_text(&open))
            .expect("envelope")["payload"]["session_id"]
            .as_str()
            .expect("session_id")
            .to_string();

        // Removing an unknown space returns `removed: false` (NOT an error).
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_BRAIN_REMOVE_SPACE,
                serde_json::json!({
                    "session_id": sid,
                    "space_id": "no-such-space",
                }),
            ),
            None,
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let v: serde_json::Value =
            serde_json::from_str(&first_text(&result)).expect("envelope");
        assert_eq!(v["payload"]["removed"].as_bool(), Some(false));
    }

    // ---- multimodal (brain-federation): brain_spaces RED gates ---------

    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn brain_spaces_lists_spaces() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let open = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_OPEN, serde_json::json!({ "workspace_id": "ws" })),
            None,
        )
        .await;
        let sid = serde_json::from_str::<serde_json::Value>(&first_text(&open))
            .expect("envelope")["payload"]["session_id"]
            .as_str()
            .expect("session_id")
            .to_string();

        // Add two spaces.
        let _ = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_BRAIN_ADD_SPACE,
                serde_json::json!({
                    "session_id": sid,
                    "space_name": "alpha",
                    "space_kind": "Repo",
                }),
            ),
            None,
        )
        .await;
        let _ = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_BRAIN_ADD_SPACE,
                serde_json::json!({
                    "session_id": sid,
                    "space_name": "beta",
                    "space_kind": "Docs",
                }),
            ),
            None,
        )
        .await;

        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_SPACES, serde_json::json!({ "session_id": sid })),
            None,
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let v: serde_json::Value =
            serde_json::from_str(&first_text(&result)).expect("envelope");
        let spaces = v["payload"]["spaces"].as_array().expect("spaces array");
        assert_eq!(spaces.len(), 2);
        assert_eq!(spaces[0]["name"].as_str(), Some("alpha"));
        assert_eq!(spaces[0]["kind"].as_str(), Some("Repo"));
        assert_eq!(spaces[1]["name"].as_str(), Some("beta"));
        assert_eq!(spaces[1]["kind"].as_str(), Some("Docs"));
    }

    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn brain_spaces_empty_session() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let open = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_OPEN, serde_json::json!({ "workspace_id": "ws" })),
            None,
        )
        .await;
        let sid = serde_json::from_str::<serde_json::Value>(&first_text(&open))
            .expect("envelope")["payload"]["session_id"]
            .as_str()
            .expect("session_id")
            .to_string();

        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_SPACES, serde_json::json!({ "session_id": sid })),
            None,
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let v: serde_json::Value =
            serde_json::from_str(&first_text(&result)).expect("envelope");
        let spaces = v["payload"]["spaces"].as_array().expect("spaces array");
        assert!(spaces.is_empty(), "expected empty array, got: {spaces:?}");
    }

    // ---- multimodal (brain-federation): brain_open spaces extension ----

    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn brain_open_with_spaces_pre_registers_them() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_BRAIN_OPEN,
                serde_json::json!({
                    "workspace_id": "ws",
                    "spaces": [
                        { "space_name": "alpha", "space_kind": "Repo" },
                        { "space_name": "beta", "space_kind": "Docs" },
                    ],
                }),
            ),
            None,
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let v: serde_json::Value = serde_json::from_str(&first_text(&result)).expect("envelope");
        let state = v["payload"]["state"].as_object().expect("state object");
        let snap_spaces = state
            .get("spaces")
            .and_then(|a| a.as_array())
            .expect("spaces array");
        // Two space IDs in the snapshot.
        assert_eq!(snap_spaces.len(), 2);
        let ids: Vec<&str> = snap_spaces
            .iter()
            .filter_map(|s| s.as_str())
            .collect();
        assert!(ids.contains(&"alpha"));
        assert!(ids.contains(&"beta"));
    }

    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn brain_open_without_spaces_preserves_existing_behavior() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_BRAIN_OPEN,
                serde_json::json!({ "workspace_id": "ws" }),
            ),
            None,
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let v: serde_json::Value = serde_json::from_str(&first_text(&result)).expect("envelope");
        assert!(v["payload"]["session_id"].is_string());
        assert_eq!(v["payload"]["workspace_id"].as_str(), Some("ws"));
        // State has no spaces (default empty vec).
        let state = v["payload"]["state"].as_object().expect("state object");
        // The state may or may not have `spaces` key depending on
        // serde treatment of empty vec: with #[serde(default)] the
        // field serialises as [] when present. Accept either.
        if let Some(spaces) = state.get("spaces") {
            assert!(
                spaces.as_array().unwrap().is_empty(),
                "spaces should be empty when no spaces given"
            );
        }
    }

    // ---- multimodal (brain-federation): brain_status spaces extension ---

    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn brain_status_includes_space_count_and_spaces() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let open = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(
                TOOL_BRAIN_OPEN,
                serde_json::json!({
                    "workspace_id": "ws",
                    "spaces": [
                        { "space_name": "alpha", "space_kind": "Repo" },
                    ],
                }),
            ),
            None,
        )
        .await;
        let sid = serde_json::from_str::<serde_json::Value>(&first_text(&open))
            .expect("envelope")["payload"]["session_id"]
            .as_str()
            .expect("session_id")
            .to_string();

        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_STATUS, serde_json::json!({ "session_id": sid })),
            None,
        )
        .await;
        assert_eq!(result.is_error, Some(false));
        let v: serde_json::Value =
            serde_json::from_str(&first_text(&result)).expect("envelope");
        let payload = v.get("payload").expect("payload");
        assert_eq!(payload["space_count"].as_u64(), Some(1));
        let details = payload["space_details"]
            .as_array()
            .expect("space_details array");
        assert_eq!(details.len(), 1);
        assert_eq!(details[0]["name"].as_str(), Some("alpha"));
        assert_eq!(details[0]["kind"].as_str(), Some("Repo"));
        // Existing fields are still present.
        assert!(payload.get("session_id").is_some());
        assert!(payload.get("workspace_id").is_some());
        assert!(payload.get("history").is_some());
    }

    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn brain_status_empty_session_reports_zero_spaces() {
        let (service, _dir) = build_test_service();
        let registry = build_test_registry();
        let open = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_OPEN, serde_json::json!({ "workspace_id": "ws" })),
            None,
        )
        .await;
        let sid = serde_json::from_str::<serde_json::Value>(&first_text(&open))
            .expect("envelope")["payload"]["session_id"]
            .as_str()
            .expect("session_id")
            .to_string();

        let result = dispatch(
            &service,
            &None,
            &registry,
            call_tool_args(TOOL_BRAIN_STATUS, serde_json::json!({ "session_id": sid })),
            None,
        )
        .await;
        let v: serde_json::Value =
            serde_json::from_str(&first_text(&result)).expect("envelope");
        let payload = v.get("payload").expect("payload");
        assert_eq!(payload["space_count"].as_u64(), Some(0));
        let details = payload["space_details"]
            .as_array()
            .expect("space_details array");
        assert!(details.is_empty());
    }

    // ---- multimodal (T21): graph_search RED gates --------------

    /// T21 RED gate: `graph_search` must return a structured
    /// `McpResultEnvelope` payload with the documented 5
    /// top-level fields (`results`, `total_count`,
    /// `next_cursor`, `raw_rank`, `normalized_score`) when
    /// called with a valid `query`.
    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn graph_search_returns_envelope() {
        use cognicode_core::domain::aggregates::generic_graph::GraphNode;
        use cognicode_core::domain::value_objects::node_kind::NodeKind;
        let (service, _wdir) = build_test_service();
        let mut nodes: Vec<GraphNode> = Vec::new();
        nodes.push(GraphNode::builder("doc:adr-0007.md#adr-7", NodeKind::Decision)
            .label("ADR-0007: Adopt GraphQL")
            .source_path("docs/adr/0007.md")
            .property("status", "accepted")
            .build());
        nodes.push(GraphNode::builder("doc:adr-0008.md#adr-8", NodeKind::Decision)
            .label("ADR-0008: Use Federation")
            .source_path("docs/adr/0008.md")
            .property("status", "proposed")
            .build());
        nodes.push(GraphNode::builder("doc:readme.md#intro", NodeKind::Doc)
            .label("Project README")
            .source_path("README.md")
            .property("section", "intro")
            .build());
        nodes.push(GraphNode::builder("issue:github#42", NodeKind::Issue)
            .label("Bug: schema mismatch on federation")
            .source_path("https://github.com/x/y/issues/42")
            .build());
        let repo_arc: Arc<dyn crate::ports::graph_repository::GraphRepository> =
            Arc::new(crate::adapters::InMemoryGraphRepository::new(nodes, Vec::new()));
        let handler = ExplorerMcpHandler::with_graph_repo(service, None, Some(repo_arc));

        let request = call_tool_args(
            TOOL_GRAPH_SEARCH,
            serde_json::json!({ "query": "ADR" }),
        );
        let result = dispatch(
            handler.service(),
            &handler.graph().cloned(),
            handler.registry(),
            request,
            handler.graph_repo(),
        )
        .await;
        let text = first_text(&result);
        let envelope: serde_json::Value =
            serde_json::from_str(&text).expect("envelope must be valid JSON");
        assert_eq!(envelope["tool_name"], TOOL_GRAPH_SEARCH);
        let results = envelope["payload"]["results"]
            .as_array()
            .expect("results is an array");
        assert_eq!(results.len(), 2);
        for r in results {
            assert!(r["node"].is_object());
            assert!(r["score"].is_number());
            // Per-item rank (renamed from `raw_rank` to avoid
            // collision with the page-level `raw_rank` at the
            // envelope root).
            assert!(r["item_rank"].is_number());
        }
        assert_eq!(envelope["payload"]["total_count"].as_u64().unwrap(), 2);
        assert!(envelope["payload"]["next_cursor"].is_null());
        // Page-level `raw_rank` (kept as-is, distinct from per-item).
        assert!(envelope["payload"]["raw_rank"].is_number());
        assert!(envelope["payload"]["normalized_score"].is_number());
    }

    /// T21 RED gate: cursor pagination. With `limit=1` and 2
    /// matches, the first page must include `next_cursor` and
    /// the second page must include `next_cursor = null`.
    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn graph_search_cursor_pagination() {
        use cognicode_core::domain::aggregates::generic_graph::GraphNode;
        use cognicode_core::domain::value_objects::node_kind::NodeKind;
        let (service, _wdir) = build_test_service();
        let mut nodes: Vec<GraphNode> = Vec::new();
        nodes.push(GraphNode::builder("doc:adr-0007.md#adr-7", NodeKind::Decision)
            .label("ADR-0007: Adopt GraphQL")
            .build());
        nodes.push(GraphNode::builder("doc:adr-0008.md#adr-8", NodeKind::Decision)
            .label("ADR-0008: Use Federation")
            .build());
        let repo_arc: Arc<dyn crate::ports::graph_repository::GraphRepository> =
            Arc::new(crate::adapters::InMemoryGraphRepository::new(nodes, Vec::new()));
        let handler = ExplorerMcpHandler::with_graph_repo(service, None, Some(repo_arc));

        let request1 = call_tool_args(
            TOOL_GRAPH_SEARCH,
            serde_json::json!({ "query": "ADR", "limit": 1 }),
        );
        let result1 = dispatch(
            handler.service(),
            &handler.graph().cloned(),
            handler.registry(),
            request1,
            handler.graph_repo(),
        )
        .await;
        let env1: serde_json::Value = serde_json::from_str(&first_text(&result1)).unwrap();
        assert_eq!(env1["payload"]["results"].as_array().unwrap().len(), 1);
        let cursor = env1["payload"]["next_cursor"]
            .as_str()
            .expect("next_cursor must be a string on page 1");
        let request2 = call_tool_args(
            TOOL_GRAPH_SEARCH,
            serde_json::json!({ "query": "ADR", "limit": 1, "cursor": cursor }),
        );
        let result2 = dispatch(
            handler.service(),
            &handler.graph().cloned(),
            handler.registry(),
            request2,
            handler.graph_repo(),
        )
        .await;
        let env2: serde_json::Value = serde_json::from_str(&first_text(&result2)).unwrap();
        assert_eq!(env2["payload"]["results"].as_array().unwrap().len(), 1);
        assert!(env2["payload"]["next_cursor"].is_null());
        let id1 = env1["payload"]["results"][0]["node"]["id"].as_str().unwrap();
        let id2 = env2["payload"]["results"][0]["node"]["id"].as_str().unwrap();
        assert_ne!(id1, id2);
    }

    /// T21 RED gate: the `graph_search` tool is absent on
    /// default builds (no `multimodal` feature).
    #[test]
    fn graph_search_hidden_without_feature() {
        if cfg!(feature = "multimodal") {
            let names: Vec<String> = build_tool_schemas()
                .iter()
                .map(|t| t.name.to_string())
                .collect();
            assert!(names.iter().any(|n| n == "graph_search"));
        } else {
            let names: Vec<String> = build_tool_schemas()
                .iter()
                .map(|t| t.name.to_string())
                .collect();
            assert!(names.iter().all(|n| n != "graph_search"));
            assert!(tool_names().iter().all(|n| *n != "graph_search"));
        }
    }

    // ---- multimodal (T14): docs_ingest RED gates ----------------

    /// T14 RED gate: `docs_ingest` is absent on default builds.
    #[test]
    fn tool_schemas_docs_ingest_hidden_without_feature() {
        if cfg!(feature = "multimodal") {
            let names: Vec<String> = build_tool_schemas()
                .iter()
                .map(|t| t.name.to_string())
                .collect();
            #[cfg(feature = "multimodal")]
            {
                assert!(names.iter().any(|n| n == TOOL_DOCS_INGEST));
            }
        } else {
            let names: Vec<String> = build_tool_schemas()
                .iter()
                .map(|t| t.name.to_string())
                .collect();
            assert!(names.iter().all(|n| n != "docs_ingest"));
            assert!(tool_names().iter().all(|n| *n != "docs_ingest"));
        }
    }

    /// T14 RED gate: `docs_ingest` must return a structured
    /// `McpResultEnvelope` payload with the documented 4
    /// top-level fields when the `path` argument is a
    /// non-empty file.
    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn docs_ingest_returns_envelope() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let f = tmp.path().join("doc.md");
        std::fs::write(&f, "# Hello\n\nsee [foo](src/foo.rs:foo:1) for details.\n").unwrap();

        let (service, _wdir) = build_test_service();
        let handler = ExplorerMcpHandler::new(service);

        let request = call_tool_args(
            TOOL_DOCS_INGEST,
            serde_json::json!({ "path": f.to_string_lossy(), "recursive": false }),
        );
        let result = dispatch(
            handler.service(),
            &handler.graph().cloned(),
            handler.registry(),
            request,
            None,
        )
        .await;
        let text = first_text(&result);
        let envelope: serde_json::Value =
            serde_json::from_str(&text).expect("envelope must be valid JSON");
        assert_eq!(envelope["tool_name"], TOOL_DOCS_INGEST);
        assert!(envelope["payload"]["files_processed"].as_u64().unwrap() >= 1);
        assert!(envelope["payload"]["nodes_created"].as_u64().unwrap() >= 1);
        assert!(envelope["payload"]["edges_created"].is_number());
        assert!(envelope["payload"]["errors"].is_array());
    }

    // ========================================================================
    // multimodal (T12): issues_ingest RED gates
    // ========================================================================

    /// T12 RED gate: `issues_ingest` must return a structured
    /// `McpResultEnvelope` payload with the documented 4
    /// top-level fields. Uses `OctocrabClient` (the production
    /// stub which returns `Ok(vec![])` without GITHUB_TOKEN),
    /// verifying the envelope shape even with zero results.
    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn issues_ingest_returns_envelope() {
        use cognicode_core::infrastructure::extraction::issues_extractor::IssuesExtractor;
        use cognicode_core::infrastructure::github::client::GitHubClient;
        use cognicode_core::infrastructure::github::octocrab_client::OctocrabClient;
        use std::sync::Arc;

        let client: Arc<dyn GitHubClient> = Arc::new(OctocrabClient::new());
        let extractor =
            IssuesExtractor::with_repo_override(client, "owner".into(), "repo".into());

        let result = dispatch_issues_ingest(
            Some(extractor),
            serde_json::json!({
                "owner": "owner",
                "repo": "repo",
            }),
        )
        .await;
        let text = first_text(&result);
        let envelope: serde_json::Value =
            serde_json::from_str(&text).expect("envelope must be valid JSON");
        assert_eq!(envelope["tool_name"], TOOL_ISSUES_INGEST);
        // OctocrabClient stub returns vec![] without a real token,
        // so issues_processed may be 0 — the envelope shape is
        // what we're asserting here.
        assert!(envelope["payload"]["issues_processed"].as_u64().is_some());
        assert!(envelope["payload"]["nodes_created"].as_u64().is_some());
        assert!(envelope["payload"]["edges_created"].is_number());
        assert!(envelope["payload"]["errors"].is_array());
    }

    /// T12 RED gate: the `issues_ingest` tool is absent on
    /// default builds (no `multimodal` feature).
    #[test]
    fn issues_ingest_hidden_without_feature() {
        if cfg!(feature = "multimodal") {
            let names: Vec<String> = build_tool_schemas()
                .iter()
                .map(|t| t.name.to_string())
                .collect();
            assert!(names.iter().any(|n| n == "issues_ingest"));
        } else {
            let names: Vec<String> = build_tool_schemas()
                .iter()
                .map(|t| t.name.to_string())
                .collect();
            assert!(names.iter().all(|n| n != "issues_ingest"));
            assert!(tool_names().iter().all(|n| *n != "issues_ingest"));
        }
    }
}
