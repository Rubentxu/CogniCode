use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use cognicode_core::domain::aggregates::SymbolId;
use serde::Deserialize;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::ExplorerError;
use crate::dto::{
    GenerateArtifactRequest, GraphEdge, GraphNode, OpenWorkspaceRequest, SaveExplorationRequest,
    SubgraphResponse,
};
use crate::service::ExplorerService;

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
// Traversal helper
// ============================================================================

/// Walk the graph from `root` and return a `SubgraphResponse`.
///
/// Traversal is a BFS bounded by `depth` hops and `max_nodes`
/// collected symbols. When the reachable set would exceed
/// `max_nodes`, we trim to the cap and set `truncated = true` /
/// `truncated_reason = Some("node_cap")`. Edges are filtered so
/// `source` and `target` always survive in `nodes`.
fn build_subgraph(
    service: &ExplorerService,
    root_id: &str,
    depth: u8,
    direction: SubgraphDirection,
    max_nodes: u32,
) -> Result<SubgraphResponse, ExplorerError> {
    let root_symbol_id = SymbolId::new(root_id);

    // Resolve the root — the canonical "symbol_not_found" branch.
    let root_resolved = service
        .symbol_repo()
        .resolve(&root_symbol_id)
        .map_err(map_repo_unavailable)?
        .ok_or_else(|| ExplorerError::SymbolNotFound(root_id.to_string()))?;

    // BFS, deduplicated by id. Bounded by `depth` AND `max_nodes`.
    let max_nodes_usize = max_nodes as usize;
    let mut visited_ids: Vec<String> = Vec::with_capacity(max_nodes_usize.min(1024));
    let mut visited_set: HashSet<String> = HashSet::new();
    let mut nodes: Vec<GraphNode> = Vec::with_capacity(max_nodes_usize.min(1024));
    let mut edges: Vec<GraphEdge> = Vec::new();

    // 1-entry queue of (symbol_id, current_depth). We keep ids in
    // `visited` order so the response is stable for a given graph.
    let mut queue: Vec<(String, u8)> = Vec::new();
    let root_str = root_id.to_string();
    queue.push((root_str.clone(), 0));
    visited_set.insert(root_str.clone());
    visited_ids.push(root_str.clone());
    nodes.push(symbol_to_node(&root_resolved.id.to_string(), &root_resolved, "function"));

    let mut truncated = false;

    while let Some((current_id, current_depth)) = queue.first().cloned() {
        queue.remove(0);
        if current_depth >= depth {
            continue;
        }
        // Truncation check before expanding — keeps `nodes.len()`
        // at or below `max_nodes` (we already enqueued the root).
        if nodes.len() >= max_nodes_usize {
            truncated = true;
            break;
        }
        let current_sym = SymbolId::new(&current_id);
        let (incoming, outgoing) = match direction {
            SubgraphDirection::Incoming => {
                (service.symbol_repo().callers(&current_sym), Vec::new())
            }
            SubgraphDirection::Outgoing => {
                (Vec::new(), service.symbol_repo().callees(&current_sym))
            }
            SubgraphDirection::Both => (
                service.symbol_repo().callers(&current_sym),
                service.symbol_repo().callees(&current_sym),
            ),
        };

        for (rel_label, neighbour) in incoming
            .into_iter()
            .map(|t| ("calls", t))
            .chain(outgoing.into_iter().map(|t| ("calls", t)))
        {
            if nodes.len() >= max_nodes_usize {
                truncated = true;
                break;
            }
            let neighbour_id = neighbour.id.to_string();
            let is_new = visited_set.insert(neighbour_id.clone());
            if is_new {
                visited_ids.push(neighbour_id.clone());
                let style = style_class_for(&format!("{:?}", neighbour.kind).to_lowercase());
                // Use the underlying kind label where possible; the
                // symbol kind's `Debug` representation is stable
                // enough for the style_class bucket.
                let kind_label = format!("{:?}", neighbour.kind).to_lowercase();
                let kind_label = match kind_label.as_str() {
                    "function" | "method" | "fn" => "function".to_string(),
                    "module" | "crate" | "trait" => "module".to_string(),
                    "external" => "external".to_string(),
                    other => other.to_string(),
                };
                let _ = style; // we use the resolved kind_label above
                nodes.push(GraphNode {
                    id: neighbour_id.clone(),
                    label: neighbour.name.clone(),
                    kind: kind_label,
                    file: Some(neighbour.file.clone()),
                    line: Some(neighbour.line),
                    style_class: style_class_for(&format!(
                        "{:?}",
                        neighbour.kind
                    )
                    .to_lowercase())
                    .to_string(),
                });
                queue.push((neighbour_id.clone(), current_depth + 1));
            }
            edges.push(GraphEdge {
                source: current_id.clone(),
                target: neighbour_id,
                relation: rel_label.to_string(),
                style_class: edge_style_class_for(rel_label).to_string(),
            });
        }
        if truncated {
            break;
        }
    }

    // If we never hit the cap, `truncated` stays false.
    if truncated {
        // Drop edges whose endpoints are not in the kept set — keep
        // the response self-consistent (no dangling references).
        let kept: HashSet<&String> = nodes.iter().map(|n| &n.id).collect();
        edges.retain(|e| kept.contains(&e.source) && kept.contains(&e.target));
    }

    Ok(SubgraphResponse {
        root: root_id.to_string(),
        nodes,
        edges,
        truncated,
        truncated_reason: if truncated {
            Some("node_cap".to_string())
        } else {
            None
        },
        corroboration_scores: HashMap::new(),
    })
}

fn symbol_to_node(id: &str, s: &crate::ports::symbol_repository::ResolvedSymbol, _style_hint: &str) -> GraphNode {
    let kind_label = format!("{:?}", s.kind).to_lowercase();
    GraphNode {
        id: id.to_string(),
        label: s.name.clone(),
        kind: kind_label.clone(),
        file: Some(s.file.clone()),
        line: Some(s.line),
        style_class: style_class_for(&kind_label).to_string(),
    }
}

fn map_repo_unavailable(e: ExplorerError) -> ExplorerError {
    match e {
        ExplorerError::GraphNotReady => {
            ExplorerError::GraphUnavailable("call graph is not loaded yet".to_string())
        }
        other => other,
    }
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
    let response = build_subgraph(&state.service, id, depth, direction, max_nodes)
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
    let focus = SymbolId::new(id);
    let response = state
        .service
        .build_contextual_graph(&focus, level, depth, max_nodes)
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

#[cfg(feature = "multimodal")]
use cognicode_core::domain::aggregates::generic_graph::NodeId;
#[cfg(feature = "multimodal")]
use cognicode_core::domain::services::{score_subgraph};
#[cfg(feature = "multimodal")]
use crate::ports::graph_repository::GraphRepository;

#[derive(Clone)]
pub struct ApiState {
    service: Arc<ExplorerService>,
    /// Optional generic graph repository for multimodal endpoints
    /// (rationale, graph_search, etc.).
    #[cfg(feature = "multimodal")]
    pub graph_repo: Option<Arc<dyn GraphRepository>>,
}

impl ApiState {
    pub fn new(service: ExplorerService) -> Self {
        Self {
            service: Arc::new(service),
            #[cfg(feature = "multimodal")]
            graph_repo: None,
        }
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
        .route("/api/workspaces/open", post(open_workspace))
        .route("/api/workspaces/:workspace_id/index", post(index_workspace))
        .route("/api/workspaces/:workspace_id/spotter", get(spotter))
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
        .route("/api/graph/:id/subgraph", get(subgraph_handler))
        .route("/api/graph/:id/contextual", get(contextual_handler))
        .route("/api/graph/:id/rationale", get(rationale_handler))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub fn router(service: ExplorerService) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/workspaces/open", post(open_workspace))
        .route("/api/workspaces/:workspace_id/index", post(index_workspace))
        .route("/api/workspaces/:workspace_id/spotter", get(spotter))
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
        .with_state(ApiState::new(service))
}

pub async fn serve(service: ExplorerService, addr: SocketAddr) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(service)).await?;
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
    Ok(Json(state.service.open_workspace(request)?).into_response())
}

async fn index_workspace(Path(_workspace_id): Path<String>) -> Result<Response, ApiError> {
    Err(ApiError(ExplorerError::NotImplemented(
        "workspace indexing will delegate to CogniCode graph/index builders",
    )))
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
    Ok(Json(
        state
            .service
            .spotter_search(&query.q, query.kind.as_deref())?,
    )
    .into_response())
}

async fn inspect_object(
    State(state): State<ApiState>,
    Path(object_id): Path<String>,
) -> Result<Response, ApiError> {
    Ok(Json(state.service.inspect_object(&object_id)?).into_response())
}

async fn available_views(
    State(state): State<ApiState>,
    Path(object_id): Path<String>,
) -> Result<Response, ApiError> {
    Ok(Json(state.service.available_views(&object_id)?).into_response())
}

async fn contextual_view(
    State(state): State<ApiState>,
    Path((object_id, view_id)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    Ok(Json(state.service.contextual_view(&object_id, &view_id)?).into_response())
}

async fn available_lenses(
    State(state): State<ApiState>,
    Path(object_id): Path<String>,
) -> Result<Response, ApiError> {
    Ok(Json(state.service.available_lenses(&object_id)?).into_response())
}

async fn apply_lens(
    State(state): State<ApiState>,
    Path((object_id, lens_id)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    Ok(Json(state.service.apply_lens(&object_id, &lens_id)?).into_response())
}

async fn save_exploration(
    State(state): State<ApiState>,
    Json(request): Json<SaveExplorationRequest>,
) -> Result<Response, ApiError> {
    Ok(Json(state.service.save_exploration(request)?).into_response())
}

async fn generate_artifact(
    State(state): State<ApiState>,
    Path(exploration_id): Path<String>,
    Json(request): Json<GenerateArtifactRequest>,
) -> Result<Response, ApiError> {
    Ok(Json(state.service.generate_artifact(&exploration_id, request)?).into_response())
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
