use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::dto::{
    GenerateArtifactRequest, GodNodeEntry, LandingPayload, OpenWorkspaceRequest,
    SaveExplorationRequest, SaveExplorationSessionRequest,
};
use crate::error::ExplorerError;
use crate::facades::{
    GraphService, MoldQLService, PersistenceService, SearchService,
    SubgraphDirection as FacadeSubgraphDirection, ViewService, WorkspaceService,
};

// ============================================================================
// Style-class taxonomy
// ============================================================================

/// Map a symbol kind to its cytoscape style class.
///
/// Buckets:
/// - `function` / `function` / `method` / `fn` → `"function"`
/// - `module` / `crate` / `trait` → `"module"`
/// - `external` → `"external"`
/// - `decision` (multimodal ADR/RFC) → `"node-decision"`
/// - `doc` (multimodal Markdown) → `"node-doc"`
/// - `issue` (multimodal tracker issue) → `"node-issue"`
/// - `evidence` (multimodal benchmark / fuzzer) → `"node-evidence"`
/// - `component` (C4 — grouping of related symbols) → `"node-component"`
/// - `container` (C4 — deployable unit) → `"node-container"`
/// - `system` (C4 — boundary of related containers) → `"node-system"`
/// - anything else → `"function"` (default)
#[inline]
pub fn style_class_for(kind: &str) -> &'static str {
    match kind.to_ascii_lowercase().as_str() {
        "function" | "method" | "fn" => "function",
        "module" | "crate" | "trait" => "module",
        "external" => "external",
        // ---- multimodal (T16) ----
        // Dashed form (e.g. `node-decision`) so the cytoscape
        // stylesheet can match a single attribute selector and the
        // kind label never collides with the code-only taxonomy
        // (which uses bare words like `function` / `module`).
        "decision" => "node-decision",
        "doc" => "node-doc",
        "issue" => "node-issue",
        "evidence" => "node-evidence",
        // ---- multimodal (C4 architecture — Phase 1) ----
        // C4 architectural node kinds. The C4 spec uses a
        // distinct shape for each (Component / Container /
        // System); the bucket names mirror the cytoscape
        // stylesheet entries 1:1.
        "component" => "node-component",
        "container" => "node-container",
        "system" => "node-system",
        // ---- C4 Code (E6 ADR-039) ----
        "code" => "node-code",
        _ => "function",
    }
}

/// Map an edge relation to its cytoscape style class.
///
/// Buckets:
/// - `calls` / `call` → `"edge.calls"`
/// - `implements` / `impl` → `"edge.implements"`
/// - `uses` / `imports` → `"edge.uses"`
/// - `cites` (multimodal) → `"edge-cites"`
/// - `justifies` (multimodal) → `"edge-justifies"`
/// - `resolves` (multimodal) → `"edge-resolves"`
/// - `corroborated_by` (multimodal) → `"edge-corroborated"`
/// - `part_of` (C4 — `source` is part of `target`) → `"edge-part-of"`
/// - `deployed_as` (C4 — `source` is deployed as `target`) → `"edge-deployed-as"`
/// - `in_system` (C4 — `source` belongs to `target` system) → `"edge-in-system"`
/// - anything else → `"edge.calls"` (default)
#[inline]
pub fn edge_style_class_for(relation: &str) -> &'static str {
    match relation.to_ascii_lowercase().as_str() {
        "calls" | "call" => "edge.calls",
        "implements" | "impl" => "edge.implements",
        "uses" | "imports" => "edge.uses",
        // ---- multimodal (T16) ----
        // Same dashed-form rule as nodes: a single hyphen
        // separates the `edge` prefix from the kind.
        "cites" => "edge-cites",
        "justifies" => "edge-justifies",
        "resolves" => "edge-resolves",
        "corroborated_by" => "edge-corroborated",
        // ---- multimodal (C4 architecture — Phase 1) ----
        // C4 architectural relationship kinds. Dashed form
        // (e.g. `edge-part-of`) is consistent with the existing
        // multimodal edge buckets above.
        "part_of" => "edge-part-of",
        "deployed_as" => "edge-deployed-as",
        "in_system" => "edge-in-system",
        _ => "edge.calls",
    }
}

// ============================================================================
// Subgraph request types
// ============================================================================

/// Direction filter for a sub-graph traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubgraphDirection {
    Incoming,
    Outgoing,
    Both,
}

impl SubgraphDirection {
    fn parse(raw: Option<&str>) -> Result<Self, ExplorerError> {
        match raw.map(str::to_ascii_lowercase).as_deref() {
            None | Some("both") => Ok(Self::Both),
            Some("incoming") => Ok(Self::Incoming),
            Some("outgoing") => Ok(Self::Outgoing),
            Some(other) => Err(ExplorerError::InvalidQuery(format!(
                "direction must be one of: incoming, outgoing, both (got: {other})"
            ))),
        }
    }
}

/// Query params accepted by `GET /api/graph/:id/subgraph`. Defaults are
/// applied in [`Self::validated`].
#[derive(Debug, Clone, Deserialize)]
pub struct SubgraphQuery {
    pub depth: Option<u8>,
    pub direction: Option<String>,
    pub max_nodes: Option<u32>,
}

impl SubgraphQuery {
    /// Defaults + range validation. Returns the canonical triple the
    /// handler will use.
    pub fn validated(&self) -> Result<(u8, SubgraphDirection, u32), ExplorerError> {
        let depth = self.depth.unwrap_or(3);
        if !(1..=10).contains(&depth) {
            return Err(ExplorerError::InvalidQuery(format!(
                "depth must be in 1..=10 (got: {depth})"
            )));
        }
        let direction = SubgraphDirection::parse(self.direction.as_deref())?;
        let max_nodes = self.max_nodes.unwrap_or(500);
        if !(1..=5000).contains(&max_nodes) {
            return Err(ExplorerError::InvalidQuery(format!(
                "max_nodes must be in 1..=5000 (got: {max_nodes})"
            )));
        }
        Ok((depth, direction, max_nodes))
    }
}

/// Validate the path `:id` segment. Non-empty and ≤ 512 chars. We
/// keep the limit generous — the actual id space is set by the
/// repository, not the API.
pub fn validate_id(id: &str) -> Result<&str, ExplorerError> {
    if id.is_empty() {
        return Err(ExplorerError::InvalidId("id must not be empty".to_string()));
    }
    if id.chars().count() > 512 {
        return Err(ExplorerError::InvalidId(
            "id must be 512 chars or fewer".to_string(),
        ));
    }
    Ok(id)
}

// ============================================================================
// Handler
// ============================================================================

async fn subgraph_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Query(q): Query<SubgraphQuery>,
) -> Result<Response, ApiError> {
    let _ = id; // silence unused warning before validation
    let id = validate_id(&id).map_err(ApiError)?;
    let (depth, direction, max_nodes) = q.validated().map_err(ApiError)?;
    let facade_direction = match direction {
        SubgraphDirection::Incoming => FacadeSubgraphDirection::Incoming,
        SubgraphDirection::Outgoing => FacadeSubgraphDirection::Outgoing,
        SubgraphDirection::Both => FacadeSubgraphDirection::Both,
    };
    let response = state
        .graph
        .build_subgraph(id, depth, facade_direction, max_nodes)
        .await
        .map_err(ApiError)?;
    Ok(Json(response).into_response())
}

// ============================================================================
// Contextual Graph — `GET /api/graph/:id/contextual` (Phase 2)
// ============================================================================

/// Query params accepted by `GET /api/graph/:id/contextual`.
///
/// Defaults are applied in [`ContextualQuery::validated`]:
/// - `level`     : `"file"` (only valid value in Phase 1)
/// - `depth`     : `1`
/// - `max_nodes` : `200`
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ContextualQuery {
    pub level: Option<String>,
    pub depth: Option<u8>,
    pub max_nodes: Option<usize>,
}

impl ContextualQuery {
    /// Apply defaults + range validation. Returns the canonical triple
    /// the handler will use. `InvalidQuery` is raised for any out-of-
    /// bound value.
    pub fn validated(&self) -> Result<(&str, u8, usize), ExplorerError> {
        let level = self.level.as_deref().unwrap_or("file");
        if level != "file" {
            return Err(ExplorerError::InvalidQuery(format!(
                "level must be 'file' in Phase 1 (got: {level})"
            )));
        }
        let depth = self.depth.unwrap_or(1);
        if !(1..=2).contains(&depth) {
            return Err(ExplorerError::InvalidQuery(format!(
                "depth must be in 1..=2 (got: {depth})"
            )));
        }
        let max_nodes = self.max_nodes.unwrap_or(200);
        if !(50..=500).contains(&max_nodes) {
            return Err(ExplorerError::InvalidQuery(format!(
                "max_nodes must be in 50..=500 (got: {max_nodes})"
            )));
        }
        Ok((level, depth, max_nodes))
    }
}

/// Handler for `GET /api/graph/:id/contextual`.
///
/// Returns:
/// - `400` on bad query params (depth out of `[1,2]`, max_nodes out
///   of `[50,500]`, unknown `level`)
/// - `404` if the focus id is not in the repository
/// - `200` with the [`crate::dto::ContextualGraphResponse`] JSON
async fn contextual_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Query(q): Query<ContextualQuery>,
) -> Result<Response, ApiError> {
    let id = validate_id(&id).map_err(ApiError)?;
    let (level, depth, max_nodes) = q.validated().map_err(ApiError)?;
    let response = state
        .view
        .build_contextual_graph(id, level, depth, max_nodes)
        .await
        .map_err(ApiError)?;
    Ok(Json(response).into_response())
}

// ============================================================================
// Rationale — `GET /api/graph/:id/rationale` (multimodal-only)
// ============================================================================

/// Query params for the rationale endpoint.
///
/// Defaults: `max_depth = 3`, `max_nodes = 50`.
/// Valid ranges: `max_depth ∈ [1..=5]`, `max_nodes ∈ [1..=200]`.
#[derive(Debug, Clone, Deserialize)]
pub struct RationaleParams {
    pub max_depth: Option<u32>,
    pub max_nodes: Option<usize>,
}

impl RationaleParams {
    /// Apply defaults + range validation.
    pub fn validated(&self) -> Result<(u32, usize), ExplorerError> {
        let max_depth = self.max_depth.unwrap_or(3);
        if !(1..=5).contains(&max_depth) {
            return Err(ExplorerError::InvalidQuery(format!(
                "max_depth out of range [1..=5] (got: {max_depth})"
            )));
        }
        let max_nodes = self.max_nodes.unwrap_or(50);
        if !(1..=200).contains(&max_nodes) {
            return Err(ExplorerError::InvalidQuery(format!(
                "max_nodes out of range [1..=200] (got: {max_nodes})"
            )));
        }
        Ok((max_depth, max_nodes))
    }
}

/// Handler for `GET /api/graph/:id/rationale`.
///
/// Returns a `SubgraphResponse` with `corroboration_scores` populated.
/// Requires the `multimodal` feature.
#[cfg(feature = "multimodal")]
async fn rationale_handler(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Query(q): Query<RationaleParams>,
) -> Result<Response, ApiError> {
    use cognicode_core::domain::aggregates::generic_graph::NodeId;
    use cognicode_core::domain::services::score_subgraph;
    use crate::ports::graph_repository::GraphRepository;

    let id = validate_id(&id).map_err(ApiError)?;
    let (max_depth, max_nodes) = q.validated().map_err(ApiError)?;
    let focus = NodeId::new(id);

    let graph_repo = state
        .graph_repo
        .clone()
        .ok_or_else(|| {
            ExplorerError::FeatureDisabled("multimodal graph repository not wired".to_string())
        })
        .map_err(ApiError)?;

    // 1) BFS rationale subgraph from the repository.
    let (nodes, edges, truncated) = graph_repo
        .rationale_subgraph(&focus, max_depth, max_nodes)
        .map_err(ExplorerError::from)
        .map_err(ApiError)?;

    // 2) Compute corroboration scores.
    let corroboration_scores = score_subgraph(&nodes, &edges);

    // 3) Convert to DTO types.
    let dto_nodes: Vec<crate::dto::GraphNode> = nodes
        .into_iter()
        .map(|n| crate::dto::GraphNode {
            id: n.id.0,
            label: n.label,
            kind: n.kind.as_str().to_string(),
            file: n.source_path.map(|p| p.display().to_string()),
            line: None,
            style_class: crate::api::style_class_for(n.kind.as_str()).to_string(),
        })
        .collect();

    let dto_edges: Vec<crate::dto::GraphEdge> = edges
        .into_iter()
        .map(|e| {
            let rel = e.kind.as_str();
            crate::dto::GraphEdge {
                source: e.source.0,
                target: e.target.0,
                relation: rel.clone(),
                style_class: crate::api::edge_style_class_for(&rel).to_string(),
            }
        })
        .collect();

    let response = SubgraphResponse {
        root: id.to_string(),
        nodes: dto_nodes,
        edges: dto_edges,
        truncated,
        truncated_reason: if truncated {
            Some("max_nodes_exceeded".to_string())
        } else {
            None
        },
        corroboration_scores,
    };
    Ok(Json(response).into_response())
}

#[derive(Clone)]
pub struct ApiState {
    pub workspace: Arc<dyn WorkspaceService>,
    pub search: Arc<dyn SearchService>,
    pub view: Arc<dyn ViewService>,
    pub persistence: Arc<dyn PersistenceService>,
    pub moldql: Arc<dyn MoldQLService>,
    pub graph: Arc<dyn GraphService>,
    #[cfg(feature = "multimodal")]
    pub graph_repo: Option<Arc<dyn GraphRepository>>,
    /// Optional ingest controller for pipeline scan operations.
    pub ingest: Option<Arc<cognicode_core::application::ingest::IngestController>>,
}

impl ApiState {
    pub fn new(
        workspace: Arc<dyn WorkspaceService>,
        search: Arc<dyn SearchService>,
        view: Arc<dyn ViewService>,
        persistence: Arc<dyn PersistenceService>,
        moldql: Arc<dyn MoldQLService>,
        graph: Arc<dyn GraphService>,
    ) -> Self {
        Self {
            workspace,
            search,
            view,
            persistence,
            moldql,
            graph,
            #[cfg(feature = "multimodal")]
            graph_repo: None,
            ingest: None,
        }
    }

    pub fn with_ingest(mut self, ingest: Arc<cognicode_core::application::ingest::IngestController>) -> Self {
        self.ingest = Some(ingest);
        self
    }

    /// Wire a generic graph repository so multimodal endpoints
    /// (rationale, graph_search) can access it.
    #[cfg(feature = "multimodal")]
    pub fn with_graph_repo(mut self, repo: Arc<dyn GraphRepository>) -> Self {
        self.graph_repo = Some(repo);
        self
    }
}

/// Build a router with a pre-constructed `ApiState`. Used by tests
/// that need to wire a `graph_repo` into the state.
#[cfg(feature = "multimodal")]
pub fn router_with_state(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/health", get(health))
        .route("/api/workspaces/open", post(open_workspace))
        .route("/api/workspaces/:workspace_id/index", post(index_workspace))
        .route("/api/workspaces/:workspace_id/spotter", get(spotter))
        .route("/api/workspaces/:workspace_id/scan", post(index_workspace))
        .route("/api/workspaces/:workspace_id/graph/stats", get(graph_stats_handler))
        .route("/api/workspaces/:workspace_id/landing", get(landing_handler))
        .route("/api/workspaces/:workspace_id/architecture", get(architecture_handler))
        .route("/api/workspaces/:workspace_id/drift", get(drift_handler))
        .route("/api/jobs/:job_id", get(job_status))
        .route("/api/objects/:object_id", get(inspect_object))
        .route("/api/objects/:object_id/views", get(available_views))
        .route(
            "/api/objects/:object_id/views/:view_id",
            get(contextual_view),
        )
        .route("/api/objects/:object_id/lenses", get(available_lenses))
        .route("/api/objects/:object_id/lenses/:lens_id", get(apply_lens))
        .route("/api/explorations", post(save_exploration))
        .route(
            "/api/explorations/:exploration_id/artifacts",
            post(generate_artifact),
        )
        .route(
            "/api/explorations/:exploration_id",
            get(get_exploration),
        )
        .route("/api/exploration-sessions", post(save_exploration_session))
        .route(
            "/api/exploration-sessions/:session_id",
            get(get_exploration_session),
        )
        .route("/api/graph/:id/subgraph", get(subgraph_handler))
        .route("/api/graph/:id/contextual", get(contextual_handler))
        .route("/api/graph/:id/rationale", get(rationale_handler))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/health", get(health))
        .route("/api/workspaces/open", post(open_workspace))
        .route("/api/workspaces/:workspace_id/index", post(index_workspace))
        .route("/api/workspaces/:workspace_id/spotter", get(spotter))
        .route("/api/workspaces/:workspace_id/scan", post(index_workspace))
        .route("/api/workspaces/:workspace_id/graph/stats", get(graph_stats_handler))
        .route("/api/workspaces/:workspace_id/landing", get(landing_handler))
        .route("/api/workspaces/:workspace_id/architecture", get(architecture_handler))
        .route("/api/workspaces/:workspace_id/drift", get(drift_handler))
        .route("/api/jobs/:job_id", get(job_status))
        .route("/api/objects/:object_id", get(inspect_object))
        .route("/api/objects/:object_id/views", get(available_views))
        .route(
            "/api/objects/:object_id/views/:view_id",
            get(contextual_view),
        )
        .route("/api/objects/:object_id/lenses", get(available_lenses))
        .route("/api/objects/:object_id/lenses/:lens_id", get(apply_lens))
        .route("/api/explorations", post(save_exploration))
        .route(
            "/api/explorations/:exploration_id/artifacts",
            post(generate_artifact),
        )
        .route(
            "/api/explorations/:exploration_id",
            get(get_exploration),
        )
        .route("/api/exploration-sessions", post(save_exploration_session))
        .route(
            "/api/exploration-sessions/:session_id",
            get(get_exploration_session),
        )
        .route("/api/graph/:id/subgraph", get(subgraph_handler))
        .route("/api/graph/:id/contextual", get(contextual_handler))
        // Rationale endpoint is only mounted when the `multimodal`
        // feature is active — without it, 404 is the correct response.
        .route(
            "/api/graph/:id/rationale",
            #[cfg(feature = "multimodal")]
            get(rationale_handler),
            #[cfg(not(feature = "multimodal"))]
            get(not_found_stub),
        )
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn serve(state: ApiState, addr: SocketAddr) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(state)).await?;
    Ok(())
}

/// Stub handler for routes that are only available behind a feature gate.
/// Returns 404 so the caller gets a clean "not found" rather than a
/// cryptic method-not-allowed.
async fn not_found_stub() -> impl IntoResponse {
    StatusCode::NOT_FOUND
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "service": "cognicode-explorer" }))
}

async fn open_workspace(
    State(state): State<ApiState>,
    Json(request): Json<OpenWorkspaceRequest>,
) -> Result<Response, ApiError> {
    let summary = state.workspace.open_workspace(request).await?;

    // Register the workspace path in the ingest controller so
    // POST /scan can resolve the workspace_id to a root path.
    if let Some(ref ingest) = state.ingest {
        let root_path = std::path::PathBuf::from(&summary.root_path);
        // The ingest controller's workspace resolver is a StaticWorkspaceResolver
        // that we populated via the runtime. We can't easily access it from here.
        // This is a temporary gap — the resolver should be shared state.
        let _ = (ingest, root_path);
    }

    Ok(Json(summary).into_response())
}

async fn index_workspace(
    State(state): State<ApiState>,
    Path(workspace_id): Path<String>,
) -> Result<Response, ApiError> {
    let ingest = state.ingest.as_ref().ok_or_else(|| {
        ApiError(ExplorerError::NotImplemented(
            "ingest controller not wired (PG unavailable)".into(),
        ))
    })?;

    match ingest.start_scan(&workspace_id).await {
        Ok(accepted) => Ok((
            axum::http::StatusCode::ACCEPTED,
            Json(serde_json::json!({
                "job_id": accepted.job_id,
                "status": accepted.status,
                "message": accepted.message,
            })),
        )
            .into_response()),
        Err(e) => Err(ApiError(ExplorerError::InvalidInput(e))),
    }
}

async fn job_status(
    State(state): State<ApiState>,
    Path(job_id): Path<String>,
) -> Result<Response, ApiError> {
    let ingest = state.ingest.as_ref().ok_or_else(|| {
        ApiError(ExplorerError::NotImplemented("ingest controller not wired".into()))
    })?;

    match ingest.get_job(&job_id).await {
        Some(status) => Ok(Json(status).into_response()),
        None => Ok((
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "job not found"})),
        ).into_response()),
    }
}

async fn graph_stats_handler(
    State(state): State<ApiState>,
    Path(workspace_id): Path<String>,
) -> Result<Response, ApiError> {
    let ingest = state.ingest.as_ref().ok_or_else(|| {
        ApiError(ExplorerError::NotImplemented("ingest controller not wired".into()))
    })?;

    let stats = ingest.graph_stats(&workspace_id).await;
    Ok(Json(stats).into_response())
}

/// Handler for `GET /api/workspaces/:workspace_id/landing`.
///
/// Returns a `LandingPayload` with workspace summary, graph nodes/edges,
/// entry points, hot paths, god nodes, and suggested questions.
///
/// The endpoint always returns 200 with `graph_status` populated — even
/// when the graph is missing or still indexing (no 503).
async fn landing_handler(
    State(state): State<ApiState>,
    Path(workspace_id): Path<String>,
) -> Result<Response, ApiError> {
    // Get workspace summary
    let workspace = state
        .workspace
        .current_workspace()
        .map_err(ApiError)?;

    // Get graph stats from ingest controller
    let (symbol_count, relation_count, graph_status) = if let Some(ingest) = &state.ingest {
        let stats = ingest.graph_stats(&workspace_id).await;
        (
            stats.symbol_count,
            stats.edge_count,
            if stats.symbol_count > 0 {
                crate::dto::GraphStatus::Ready
            } else {
                crate::dto::GraphStatus::Missing
            },
        )
    } else {
        (0, 0, crate::dto::GraphStatus::Missing)
    };

    // Build the landing payload with empty stubs for now.
    // TODO: Wire get_entry_points, get_hot_paths, graph_insights from the
    // analysis service once those methods are available on a facade.
    let payload = LandingPayload {
        workspace: crate::dto::WorkspaceSummary {
            id: workspace.id.clone(),
            root_path: workspace.root_path.clone(),
            graph_status,
            indexed_at: None,
            symbol_count,
            relation_count,
        },
        nodes: Vec::new(),
        edges: Vec::new(),
        entry_points: Vec::new(),
        hot_paths: Vec::new(),
        god_nodes: Vec::new(),
        suggested_questions: Vec::new(),
        graph_status,
    };

    Ok(Json(payload).into_response())
}

/// Handler for `GET /api/workspaces/:workspace_id/architecture`.
///
/// Synthesises a C4 component graph from `module_list()` (directories as
/// components with `part_of` edges reflecting directory hierarchy).
/// Returns a `SubgraphResponse` whose nodes use `style_class = "node-component"`.
async fn architecture_handler(
    State(state): State<ApiState>,
    Path(_workspace_id): Path<String>,
) -> Result<Response, ApiError> {
    let workspace = state.workspace.current_workspace().map_err(ApiError)?;
    let response = state
        .graph
        .build_architecture(&workspace.root_path)
        .await?;
    Ok(Json(response).into_response())
}

/// Handler for `GET /api/workspaces/:workspace_id/drift`.
///
/// Compares the inferred C4 architecture against `.cognicode/expected-architecture.yaml`.
/// Returns a `DriftReport` with missing containers, extra containers, and wrong sub_kind findings.
async fn drift_handler(
    State(state): State<ApiState>,
    Path(_workspace_id): Path<String>,
) -> Result<Response, ApiError> {
    let workspace = state.workspace.current_workspace().map_err(ApiError)?;
    let report = state
        .graph
        .compare_architecture(&workspace.root_path)
        .await?;
    Ok(Json(report).into_response())
}

#[derive(Debug, Deserialize)]
struct SpotterQuery {
    q: String,
    kind: Option<String>,
}

async fn spotter(
    State(state): State<ApiState>,
    Path(_workspace_id): Path<String>,
    Query(query): Query<SpotterQuery>,
) -> Result<Response, ApiError> {
    Ok(Json(state.search.spotter_search(&query.q, query.kind.as_deref()).await?).into_response())
}

async fn inspect_object(
    State(state): State<ApiState>,
    Path(object_id): Path<String>,
) -> Result<Response, ApiError> {
    Ok(Json(state.search.inspect_object(&object_id).await?).into_response())
}

async fn available_views(
    State(state): State<ApiState>,
    Path(object_id): Path<String>,
) -> Result<Response, ApiError> {
    Ok(Json(state.view.available_views(&object_id).await?).into_response())
}

async fn contextual_view(
    State(state): State<ApiState>,
    Path((object_id, view_id)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    Ok(Json(state.view.contextual_view(&object_id, &view_id).await?).into_response())
}

async fn available_lenses(
    State(state): State<ApiState>,
    Path(object_id): Path<String>,
) -> Result<Response, ApiError> {
    Ok(Json(state.view.available_lenses(&object_id).await?).into_response())
}

async fn apply_lens(
    State(state): State<ApiState>,
    Path((object_id, lens_id)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    Ok(Json(state.view.apply_lens(&object_id, &lens_id).await?).into_response())
}

async fn save_exploration(
    State(state): State<ApiState>,
    Json(request): Json<SaveExplorationRequest>,
) -> Result<Response, ApiError> {
    Ok(Json(state.persistence.save_exploration(request).await?).into_response())
}

async fn generate_artifact(
    State(state): State<ApiState>,
    Path(exploration_id): Path<String>,
    Json(request): Json<GenerateArtifactRequest>,
) -> Result<Response, ApiError> {
    Ok(Json(state.persistence.generate_artifact(&exploration_id, request).await?).into_response())
}

/// GET /api/explorations/:id — return a previously saved exploration path.
async fn get_exploration(
    State(state): State<ApiState>,
    Path(exploration_id): Path<String>,
) -> Result<Response, ApiError> {
    let session = state.persistence.load_exploration_session(&exploration_id).await?;
    match session {
        Some(s) => Ok(Json(s).into_response()),
        None => Err(ApiError(ExplorerError::NotFound(format!(
            "exploration session {exploration_id} not found"
        )))),
    }
}

/// POST /api/exploration-sessions — save an exploration session.
async fn save_exploration_session(
    State(state): State<ApiState>,
    Json(request): Json<SaveExplorationSessionRequest>,
) -> Result<Response, ApiError> {
    Ok(Json(state.persistence.save_exploration_session(request).await?).into_response())
}

/// GET /api/exploration-sessions/:session_id — load a session by id.
async fn get_exploration_session(
    State(state): State<ApiState>,
    Path(session_id): Path<String>,
) -> Result<Response, ApiError> {
    let session = state.persistence.load_exploration_session(&session_id).await?;
    match session {
        Some(s) => Ok(Json(s).into_response()),
        None => Err(ApiError(ExplorerError::NotFound(format!(
            "exploration session {session_id} not found"
        )))),
    }
}

struct ApiError(ExplorerError);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0 {
            ExplorerError::WorkspaceNotFound(_)
            | ExplorerError::ObjectNotFound(_)
            | ExplorerError::SourceUnavailable { .. } => StatusCode::NOT_FOUND,
            ExplorerError::ViewNotAvailable { .. } => StatusCode::NOT_FOUND,
            ExplorerError::NotFound(_) => StatusCode::NOT_FOUND,
            ExplorerError::SymbolNotFound(_) => StatusCode::NOT_FOUND,
            ExplorerError::ResolutionFailed(_) => StatusCode::BAD_REQUEST,
            ExplorerError::InvalidInput(_) => StatusCode::BAD_REQUEST,
            ExplorerError::InvalidQuery(_) => StatusCode::BAD_REQUEST,
            ExplorerError::InvalidId(_) => StatusCode::BAD_REQUEST,
            ExplorerError::Conflict(_) => StatusCode::CONFLICT,
            ExplorerError::FeatureDisabled(_) => StatusCode::SERVICE_UNAVAILABLE,
            ExplorerError::GraphNotReady => StatusCode::SERVICE_UNAVAILABLE,
            ExplorerError::GraphUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            ExplorerError::NotImplemented(_) => StatusCode::NOT_IMPLEMENTED,
            ExplorerError::Anyhow(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = serde_json::json!({
            "error": self.0.to_string(),
        });

        (status, Json(body)).into_response()
    }
}

impl From<ExplorerError> for ApiError {
    fn from(error: ExplorerError) -> Self {
        Self(error)
    }
}
