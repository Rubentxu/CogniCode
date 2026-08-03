use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::http::header;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde::Serialize;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use crate::affordance;
use crate::domain::c4_mermaid::{self, C4Level};
use crate::domain::snapshot::{
    SnapshotError as SnapshotRenderError, SnapshotFormat, SnapshotService,
};
use crate::domain::snapshot_dispatch::SnapshotViewKind;
#[cfg(feature = "multimodal")]
use crate::domain::trace_mermaid::decision_trace_to_mermaid;
use crate::domain::trace_mermaid::{
    TraceEmitContext, TraceMermaidViewKind, call_graph_to_mermaid, impact_radius_to_mermaid,
    vertical_slice_to_mermaid,
};
use crate::dto::{
    GenerateArtifactRequest, GodNodeEntry, InspectionTarget, LANDING_NODE_CAP, LandingPayload,
    OpenWorkspaceRequest, SaveExplorationSessionRequest, SubgraphResponse,
};
use crate::error::{ExplorerError, ExplorerResult};
use crate::facades::investigation::{
    AddArtifactRequest, CreateInvestigationRequest, Evidence, Investigation, PinEvidenceRequest,
    UpdateInvestigationRequest,
};
use crate::facades::{
    GraphService, InvestigationFacade, MoldQLService, PersistenceService, SearchService,
    SubgraphDirection as FacadeSubgraphDirection, ViewService, WorkspaceService,
};
#[cfg(feature = "multimodal")]
use crate::ports::graph_repository::GraphRepository;
use crate::ports::symbol_repository::ResolvedSymbol;

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
        "depends_on" => "edge-depends-on",
        _ => "edge.calls",
    }
}

fn resolved_symbol_to_mvp(resolved: &ResolvedSymbol) -> String {
    format!(
        "symbol:{}:{}:{}",
        resolved.file, resolved.name, resolved.line
    )
}

fn resolved_symbol_to_graph_node(resolved: &ResolvedSymbol) -> crate::dto::GraphNode {
    let kind_label = format!("{:?}", resolved.kind).to_lowercase();
    crate::dto::GraphNode {
        id: resolved_symbol_to_mvp(resolved),
        label: resolved.name.clone(),
        kind: kind_label.clone(),
        file: Some(resolved.file.clone()),
        line: Some(resolved.line),
        style_class: style_class_for(&kind_label).to_string(),
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

/// Single source of truth for the landing-page truncation policy.
///
/// Returns `(truncated, truncated_reason)` for a collection of size
/// `total`. Boundary: `total <= LANDING_NODE_CAP` is **not** truncated;
/// `total > LANDING_NODE_CAP` is truncated with reason `"node_cap"`.
///
/// Pure function — no I/O. The `landing_handler` MUST call this
/// helper rather than re-implementing the comparison inline.
///
/// See `openspec/changes/e8b-landing-payload-truncation/specs/graphlanding-affordances/spec.md`
/// Requirement 9 for the contract.
pub fn apply_landing_cap(total: usize) -> (bool, Option<String>) {
    if total > LANDING_NODE_CAP {
        (true, Some("node_cap".to_string()))
    } else {
        (false, None)
    }
}

// ============================================================================
// MoldQL Pattern Profile — T6
// ============================================================================

/// Request body for `POST /api/moldql/pattern`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PatternQueryBody {
    /// The Pattern Profile query string to execute.
    pub query: String,
    /// Optional workspace scope. Defaults to current workspace if omitted.
    pub workspace_id: Option<String>,
    /// Optional revision pin. Defaults to workspace HEAD if omitted.
    pub revision_id: Option<u64>,
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
        .await
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
            style_class: crate::api::style_class_for(&n.kind.as_str()).to_string(),
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

/// Handler for `GET /api/decisions/:id/support-pack`.
///
/// Returns a `DecisionSupportPack` with five panes in stable order:
/// decision_graph, architecture_rationale, evidence_pack, risk_map,
/// change_impact_story. Each pane carries its own `PaneStatus` so partial
/// failure never propagates beyond the pane.
///
/// Requires the `multimodal` feature.
#[cfg(feature = "multimodal")]
async fn get_decision_support_pack(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    use crate::domain::decision_support_pack::DecisionSupportPackBuilder;
    use crate::ports::graph_repository::GraphRepository;

    let id = validate_id(&id).map_err(ApiError)?;

    let graph_repo = state.graph_repo.clone().ok_or_else(|| {
        ExplorerError::FeatureDisabled("multimodal graph repository not wired".to_string())
    })?;

    // Get graph_query from the GraphService for RiskMap/ChangeImpactStory
    let graph_query = state.graph.graph_query();

    let pack = DecisionSupportPackBuilder::build(&id, graph_query, None, Some(graph_repo.as_ref()))
        .await
        .map_err(ExplorerError::from)
        .map_err(ApiError)?;

    Ok(Json(pack).into_response())
}

#[derive(Clone)]
pub struct ApiState {
    pub workspace: Arc<dyn WorkspaceService>,
    pub search: Arc<dyn SearchService>,
    pub view: Arc<dyn ViewService>,
    pub persistence: Arc<dyn PersistenceService>,
    pub moldql: Arc<dyn MoldQLService>,
    pub graph: Arc<dyn GraphService>,
    pub investigation: Option<Arc<dyn InvestigationFacade>>,
    #[cfg(feature = "multimodal")]
    pub graph_repo: Option<Arc<dyn GraphRepository>>,
    /// Optional snapshot rendering service (requires mmdc on PATH).
    pub snapshot: Option<Arc<SnapshotService>>,
    /// Monotonically-increasing counter incremented after each successful ingest.
    /// Used as a fallback `revision_id` for `MoldQLService::execute_query_pinned`
    /// when the caller doesn't supply one.
    pub revision_tracker: Arc<AtomicU64>,
    /// Optional analytics algorithm registry (E28.4).
    /// Wired when a lineage store is available.
    pub analytics_registry:
        Option<Arc<cognicode_core::application::services::graph_analytics::AlgorithmRegistry>>,
    /// Optional analytics lineage store (E28.4).
    /// Wired when a lineage store is available.
    pub analytics_lineage_store:
        Option<Arc<dyn cognicode_core::domain::analytics::RunLineageStore>>,
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
            investigation: None,
            #[cfg(feature = "multimodal")]
            graph_repo: None,
            snapshot: None,
            revision_tracker: Arc::new(AtomicU64::new(1)),
            analytics_registry: None,
            analytics_lineage_store: None,
        }
    }

    /// Wire a generic graph repository so multimodal endpoints
    /// (rationale, graph_search) can access it.
    #[cfg(feature = "multimodal")]
    pub fn with_graph_repo(mut self, repo: Arc<dyn GraphRepository>) -> Self {
        self.graph_repo = Some(repo);
        self
    }

    /// Wire a snapshot rendering service for diagram export.
    pub fn with_snapshot(self, snapshot: Arc<SnapshotService>) -> Self {
        Self {
            snapshot: Some(snapshot),
            ..self
        }
    }

    /// Wire an investigation service (ADR-005 INV-1).
    pub fn with_investigation(self, investigation: Arc<dyn InvestigationFacade>) -> Self {
        Self {
            investigation: Some(investigation),
            ..self
        }
    }

    /// Inject a custom revision tracker (used by tests to verify bump behaviour).
    pub fn with_revision_tracker(self, tracker: Arc<AtomicU64>) -> Self {
        Self {
            revision_tracker: tracker,
            ..self
        }
    }

    /// Wire an analytics algorithm registry and lineage store (E28.4).
    ///
    /// Both arguments are required — pass `None` to explicitly opt out of analytics.
    pub fn with_analytics(
        self,
        registry: Arc<cognicode_core::application::services::graph_analytics::AlgorithmRegistry>,
        lineage_store: Arc<dyn cognicode_core::domain::analytics::RunLineageStore>,
    ) -> Self {
        Self {
            analytics_registry: Some(registry),
            analytics_lineage_store: Some(lineage_store),
            ..self
        }
    }

    /// Returns the current workspace_id (from WorkspaceService) and the latest revision_id.
    /// Falls back to `("default", 1)` if workspace service has no current workspace.
    pub fn current_pin(&self) -> (String, u64) {
        let ws_id = self
            .workspace
            .current_workspace()
            .map(|w| w.id)
            .unwrap_or_else(|_| "default".to_string());
        let rev = self.revision_tracker.load(Ordering::SeqCst);
        (ws_id, rev)
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
        .route("/api/workspaces/:workspace_id/spotter", get(spotter))
        .route(
            "/api/workspaces/:workspace_id/landing",
            get(landing_handler),
        )
        .route(
            "/api/workspaces/:workspace_id/architecture",
            get(architecture_handler),
        )
        .route("/api/workspaces/:workspace_id/drift", get(drift_handler))
        .route("/api/objects/:object_id", get(inspect_object))
        .route(
            "/api/affordances/:object_type",
            get(affordances_by_type_handler),
        )
        .route("/api/objects/:object_id/views", get(available_views))
        .route(
            "/api/objects/:object_id/views/:view_id",
            get(contextual_view),
        )
        .route(
            "/api/objects/:object_id/related-knowledge",
            get(related_knowledge),
        )
        .route("/api/objects/:object_id/lenses", get(available_lenses))
        .route("/api/objects/:object_id/lenses/:lens_id", get(apply_lens))
        .route("/api/exploration-sessions", post(save_exploration_session))
        .route(
            "/api/exploration-sessions/:session_id",
            get(get_exploration_session),
        )
        .route(
            "/api/exploration-sessions/:session_id/artifacts",
            post(generate_artifact),
        )
        .route(
            "/api/workspaces/:workspace_id/explorations",
            get(list_explorations),
        )
        .route("/api/graph/:id/subgraph", get(subgraph_handler))
        .route("/api/graph/:id/contextual", get(contextual_handler))
        .route("/api/graph/:id/rationale", get(rationale_handler))
        // Decision support pack endpoint — E25 PR2
        // Only mounted when the `multimodal` feature is active.
        .route(
            "/api/decisions/:id/support-pack",
            #[cfg(feature = "multimodal")]
            get(get_decision_support_pack),
            #[cfg(not(feature = "multimodal"))]
            get(not_found_stub),
        )
        .route(
            "/api/workspaces/:workspace_id/architecture/mermaid",
            get(mermaid_handler),
        )
        .route(
            "/api/workspaces/:workspace_id/mermaid/trace",
            get(trace_mermaid_handler),
        )
        .route(
            "/api/workspaces/:workspace_id/snapshot",
            get(snapshot_handler),
        )
        // Investigation CRUD — ADR-005 INV-1
        .route("/api/investigations", post(create_investigation))
        .route("/api/investigations", get(list_investigations))
        .route("/api/investigations/:id", get(get_investigation))
        .route("/api/investigations/:id", put(update_investigation))
        .route("/api/investigations/:id", delete(delete_investigation))
        // Pin evidence — ADR-005 E21-2
        .route("/api/investigations/:id/evidence", post(pin_evidence))
        .route(
            "/api/investigations/:id/artifacts",
            post(add_investigation_artifact),
        )
        // Evidence Pack view — ADR-005 E21-3
        .route(
            "/api/investigations/:id/evidence-pack",
            get(get_investigation_evidence_pack),
        )
        // Composed Narrative view — ADR-005 E21-4
        .route(
            "/api/investigations/:id/composed-narrative",
            get(get_investigation_composed_narrative),
        )
        // ADR-010 E24.1: Regenerate diagram artifact
        .route(
            "/api/investigations/:id/artifacts/:aid/regenerate",
            post(regenerate_artifact),
        )
        .route(
            "/api/objects/:object_id/affordances",
            get(affordances_handler),
        )
        // MoldQL Pattern Profile endpoint — T6
        .route("/api/moldql/pattern", post(moldql_pattern_handler))
        // T8: capabilities endpoint
        .route(
            "/api/moldql/pattern/capabilities",
            get(moldql_pattern_capabilities_handler),
        )
        // E28.4 PR5: Analytics surfaces
        .route("/api/analytics/run", post(analytics_run_handler))
        .route("/api/analytics/catalog", get(analytics_catalog_handler))
        .route(
            "/api/analytics/lineage",
            get(analytics_lineage_list_handler),
        )
        .route(
            "/api/analytics/lineage/:run_id",
            get(analytics_lineage_get_handler),
        )
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/health", get(health))
        .route("/api/workspaces/open", post(open_workspace))
        .route("/api/workspaces/:workspace_id/spotter", get(spotter))
        .route(
            "/api/workspaces/:workspace_id/landing",
            get(landing_handler),
        )
        .route(
            "/api/workspaces/:workspace_id/architecture",
            get(architecture_handler),
        )
        .route("/api/workspaces/:workspace_id/drift", get(drift_handler))
        .route("/api/objects/:object_id", get(inspect_object))
        .route(
            "/api/affordances/:object_type",
            get(affordances_by_type_handler),
        )
        .route("/api/objects/:object_id/views", get(available_views))
        .route(
            "/api/objects/:object_id/views/:view_id",
            get(contextual_view),
        )
        .route(
            "/api/objects/:object_id/related-knowledge",
            get(related_knowledge),
        )
        .route("/api/objects/:object_id/lenses", get(available_lenses))
        .route("/api/objects/:object_id/lenses/:lens_id", get(apply_lens))
        .route("/api/exploration-sessions", post(save_exploration_session))
        .route(
            "/api/exploration-sessions/:session_id",
            get(get_exploration_session),
        )
        .route(
            "/api/exploration-sessions/:session_id/artifacts",
            post(generate_artifact),
        )
        .route(
            "/api/workspaces/:workspace_id/explorations",
            get(list_explorations),
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
        // Decision support pack endpoint — E25 PR2
        // Only mounted when the `multimodal` feature is active.
        .route(
            "/api/decisions/:id/support-pack",
            #[cfg(feature = "multimodal")]
            get(get_decision_support_pack),
            #[cfg(not(feature = "multimodal"))]
            get(not_found_stub),
        )
        .route(
            "/api/workspaces/:workspace_id/architecture/mermaid",
            get(mermaid_handler),
        )
        .route(
            "/api/workspaces/:workspace_id/mermaid/trace",
            get(trace_mermaid_handler),
        )
        .route(
            "/api/workspaces/:workspace_id/snapshot",
            get(snapshot_handler),
        )
        // Investigation CRUD — ADR-005 INV-1
        .route("/api/investigations", post(create_investigation))
        .route("/api/investigations", get(list_investigations))
        .route("/api/investigations/:id", get(get_investigation))
        .route("/api/investigations/:id", put(update_investigation))
        .route("/api/investigations/:id", delete(delete_investigation))
        .route(
            "/api/objects/:object_id/affordances",
            get(affordances_handler),
        )
        // MoldQL Pattern Profile endpoint — T6
        .route("/api/moldql/pattern", post(moldql_pattern_handler))
        // T8: capabilities endpoint
        .route(
            "/api/moldql/pattern/capabilities",
            get(moldql_pattern_capabilities_handler),
        )
        // E28.4 PR5: Analytics surfaces
        .route("/api/analytics/run", post(analytics_run_handler))
        .route("/api/analytics/catalog", get(analytics_catalog_handler))
        .route(
            "/api/analytics/lineage",
            get(analytics_lineage_list_handler),
        )
        .route(
            "/api/analytics/lineage/:run_id",
            get(analytics_lineage_get_handler),
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

// ============================================================================
// E28.4 PR5: Analytics REST handlers
// ============================================================================

use crate::dto::{
    AlgorithmDescriptorSummary, AnalyticsCatalogResponse, AnalyticsLineageDetailResponse,
    AnalyticsLineageResponse, LineageEntry, RunAnalyticsRequest, RunAnalyticsResponse,
};

/// Query parameters for analytics lineage list endpoint.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsLineageQuery {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub revision_id: Option<u64>,
    #[serde(default)]
    pub algorithm_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub limit: Option<u64>,
}

async fn analytics_run_handler(
    State(state): State<ApiState>,
    Json(req): Json<RunAnalyticsRequest>,
) -> Result<Response, ApiError> {
    // E28.4: Full integration requires CallGraph access from GraphService.
    // Return stub response for now - MCP handler has full BSP implementation.
    let run_id = format!(
        "run_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let executed_at = chrono::Utc::now().to_rfc3339();

    let result = serde_json::json!({
        "algorithm_id": req.algorithm_id,
        "from_symbol": req.from_symbol,
        "to_symbol": req.to_symbol,
        "max_hops": req.max_hops.unwrap_or(5),
        "paths_found": 0,
        "paths": [],
        "note": "REST handler stub - use MCP analytics_run tool for actual execution"
    });

    let response = RunAnalyticsResponse {
        algorithm_id: req.algorithm_id,
        run_id,
        executed_at,
        lineage_persisted: false,
        result,
    };

    Ok(Json(response).into_response())
}

async fn analytics_catalog_handler(State(state): State<ApiState>) -> Response {
    // E28.4: Return catalog from registry if wired, otherwise return hardcoded list
    let registry = match &state.analytics_registry {
        Some(r) => r,
        None => {
            // Fall back to hardcoded catalog when registry is not wired
            let algorithms = vec![
                AlgorithmDescriptorSummary {
                    id: "bounded_shortest_paths".to_string(),
                    name: "Bounded Shortest Paths".to_string(),
                    version: "1.0.0".to_string(),
                    description: "Find all simple paths between two symbols bounded by max hops"
                        .to_string(),
                    mode: "Stream".to_string(),
                    categories: vec!["pathfinding".to_string(), "graph".to_string()],
                },
                AlgorithmDescriptorSummary {
                    id: "page_rank".to_string(),
                    name: "PageRank".to_string(),
                    version: "1.0.0".to_string(),
                    description: "Compute PageRank scores for all symbols in the call graph"
                        .to_string(),
                    mode: "Stats".to_string(),
                    categories: vec!["centrality".to_string(), "graph".to_string()],
                },
                AlgorithmDescriptorSummary {
                    id: "scc".to_string(),
                    name: "Strongly Connected Components".to_string(),
                    version: "1.0.0".to_string(),
                    description: "Find strongly connected components in the call graph".to_string(),
                    mode: "Stats".to_string(),
                    categories: vec!["clustering".to_string(), "graph".to_string()],
                },
                AlgorithmDescriptorSummary {
                    id: "wcc".to_string(),
                    name: "Weakly Connected Components".to_string(),
                    version: "1.0.0".to_string(),
                    description: "Find weakly connected components in the call graph".to_string(),
                    mode: "Stats".to_string(),
                    categories: vec!["clustering".to_string(), "graph".to_string()],
                },
            ];
            let total = algorithms.len();
            return Json(AnalyticsCatalogResponse { algorithms, total }).into_response();
        }
    };

    // Query the registry for admitted algorithms
    let algorithms: Vec<AlgorithmDescriptorSummary> = registry
        .admitted()
        .map(|d| {
            let identity = d.identity();
            AlgorithmDescriptorSummary {
                id: identity.id.as_str().to_string(),
                name: identity.id.as_str().to_string(),
                version: identity.version.to_string(),
                description: format!("{} v{}", identity.id.as_str(), identity.version),
                mode: d
                    .supported_modes()
                    .first()
                    .map(|m| m.to_string())
                    .unwrap_or_default(),
                categories: vec!["graph".to_string(), "analytics".to_string()],
            }
        })
        .collect();

    let total = algorithms.len();
    Json(AnalyticsCatalogResponse { algorithms, total }).into_response()
}

async fn analytics_lineage_list_handler(
    State(state): State<ApiState>,
    Query(params): Query<AnalyticsLineageQuery>,
) -> Response {
    // E28.4: Wire to lineage store if available
    let store = match &state.analytics_lineage_store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "analytics_not_available",
                    "message": "analytics lineage store not wired"
                })),
            )
                .into_response();
        }
    };

    // Build filter from query params
    use cognicode_core::domain::analytics::{RunLineageFilter, RunStatus};
    use cognicode_core::domain::value_objects::{RevisionId, WorkspaceId};

    let filter = RunLineageFilter {
        workspace_id: params
            .workspace_id
            .as_ref()
            .map(|s| WorkspaceId::try_new(s.clone()).ok())
            .flatten(),
        revision_id: params.revision_id.map(|r| RevisionId::new(r)),
        algorithm_id: params.algorithm_id.as_ref().and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some(cognicode_core::domain::analytics::AlgorithmId::from_string(
                    s.clone(),
                ))
            }
        }),
        status: params.status.as_ref().and_then(|s| match s.as_str() {
            "succeeded" => Some(RunStatus::Succeeded),
            "failed" => Some(RunStatus::Failed),
            "truncated" => Some(RunStatus::Truncated),
            "pending" => Some(RunStatus::Pending),
            "running" => Some(RunStatus::Running),
            _ => None,
        }),
    };

    match store.query(filter, params.limit).await {
        Ok(lineages) => {
            let runs: Vec<LineageEntry> = lineages
                .into_iter()
                .map(|l| LineageEntry {
                    run_id: l.run_id.to_string(),
                    algorithm_id: l.algorithm_id.to_string(),
                    executed_at: l.started_at.to_rfc3339(),
                    parameters: l.params,
                    result_summary: serde_json::json!({
                        "status": l.status.to_string(),
                        "row_count": l.row_count,
                        "mode": l.mode.to_string(),
                    }),
                    mode: l.mode.to_string(),
                })
                .collect();
            let total = runs.len();
            Json(AnalyticsLineageResponse { runs, total }).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "lineage_query_failed",
                "message": format!("failed to query lineage: {}", e)
            })),
        )
            .into_response(),
    }
}

async fn analytics_lineage_get_handler(
    State(state): State<ApiState>,
    Path(run_id): Path<String>,
) -> Response {
    // E28.4: Wire to lineage store if available
    let store = match &state.analytics_lineage_store {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "analytics_not_available",
                    "message": "analytics lineage store not wired"
                })),
            )
                .into_response();
        }
    };

    // Parse run_id into Uuid
    let run_uuid = cognicode_core::domain::analytics::lineage::Uuid::from_string(run_id.clone());

    // Query the lineage store
    match store.get(run_uuid).await {
        Ok(lineage) => {
            let response = AnalyticsLineageDetailResponse {
                run_id: lineage.run_id.to_string(),
                algorithm_id: lineage.algorithm_id.to_string(),
                executed_at: lineage.started_at.to_rfc3339(),
                parameters: lineage.params,
                result_summary: serde_json::json!({
                    "status": lineage.status.to_string(),
                    "row_count": lineage.row_count,
                    "mode": lineage.mode.to_string(),
                }),
                mode: lineage.mode.to_string(),
            };
            Json(response).into_response()
        }
        Err(cognicode_core::domain::analytics::AnalyticsError::RunNotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "not_found",
                "message": format!("run `{}` not found", run_id)
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "lineage_query_failed",
                "message": format!("failed to query lineage: {}", e)
            })),
        )
            .into_response(),
    }
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

    Ok(Json(summary).into_response())
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
    let workspace = state.workspace.current_workspace().map_err(ApiError)?;

    // Graph stats — the PG-backed ingest controller was removed with
    // e29-7; the landing surface reports Missing/0 until a graph is
    // available (the graph facade below computes the semantic payload).
    let (symbol_count, relation_count, graph_status) = (0, 0, crate::dto::GraphStatus::Missing);

    // Landing semantic seeds.
    // NOTE: `GraphService` computes these from the Explorer seam
    // (`all_symbols()` + `GraphQueryPort`) rather than leaking
    // `WorkspaceSession` into the HTTP layer.
    let (entry_point_symbols, total_entry_points) = state
        .graph
        .landing_entry_points(LANDING_NODE_CAP)
        .await
        .map_err(ApiError)?;
    let hot_path_symbols = state
        .graph
        .landing_hot_paths(10, 2)
        .await
        .map_err(ApiError)?;
    let god_nodes = state.graph.landing_god_nodes(10).await.map_err(ApiError)?;

    let (truncated, truncated_reason) = apply_landing_cap(total_entry_points);

    // Materialise entry_points / hot_paths summaries via the existing Search
    // facade so we reuse the canonical `InspectableObjectSummary` shape.
    let mut entry_points = Vec::with_capacity(entry_point_symbols.len());
    for sym in &entry_point_symbols {
        let mvp_id = resolved_symbol_to_mvp(sym);
        entry_points.push(
            state
                .search
                .inspect_object(&mvp_id)
                .await
                .map_err(ApiError)?,
        );
    }

    let mut hot_paths = Vec::with_capacity(hot_path_symbols.len());
    for sym in &hot_path_symbols {
        let mvp_id = resolved_symbol_to_mvp(sym);
        hot_paths.push(
            state
                .search
                .inspect_object(&mvp_id)
                .await
                .map_err(ApiError)?,
        );
    }

    // Deduplicate landing nodes by canonical symbol id, then render them as
    // MVP ids so the frontend can feed them back into `inspect_object()`.
    let mut selected_symbols = Vec::<ResolvedSymbol>::new();
    let mut seen = HashSet::<String>::new();
    for sym in entry_point_symbols.iter().chain(hot_path_symbols.iter()) {
        let key = sym.id.to_string();
        if seen.insert(key) {
            selected_symbols.push(sym.clone());
        }
    }
    for god in &god_nodes {
        if seen.insert(god.id.clone())
            && let Some(sym) = state
                .graph
                .resolve_symbol(&god.id)
                .await
                .map_err(ApiError)?
        {
            selected_symbols.push(sym);
        }
    }

    let nodes: Vec<crate::dto::GraphNode> = selected_symbols
        .iter()
        .map(resolved_symbol_to_graph_node)
        .collect();

    // Edges only between selected nodes; no dangling endpoints.
    let selected_mvp_ids: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
    let Some(graph_query) = state.graph.graph_query() else {
        let payload = LandingPayload {
            workspace: crate::dto::WorkspaceSummary {
                id: workspace.id.clone(),
                root_path: workspace.root_path.clone(),
                graph_status,
                indexed_at: None,
                symbol_count,
                relation_count,
            },
            nodes,
            edges: Vec::new(),
            entry_points,
            hot_paths,
            god_nodes,
            suggested_questions: Vec::new(),
            graph_status,
            truncated,
            truncated_reason,
        };
        return Ok(Json(payload).into_response());
    };

    let mut edges = Vec::<crate::dto::GraphEdge>::new();
    let mut seen_edges = HashSet::<(String, String)>::new();
    for sym in &selected_symbols {
        let src_mvp = resolved_symbol_to_mvp(sym);
        for callee in graph_query.callees(&sym.id) {
            let tgt_resolved = ResolvedSymbol {
                id: callee.id.clone(),
                name: callee.name.clone(),
                kind: callee.kind,
                file: callee.file.clone(),
                line: callee.line,
                signature: callee.signature.clone(),
            };
            let tgt_mvp = resolved_symbol_to_mvp(&tgt_resolved);
            if selected_mvp_ids.contains(&src_mvp)
                && selected_mvp_ids.contains(&tgt_mvp)
                && seen_edges.insert((src_mvp.clone(), tgt_mvp.clone()))
            {
                edges.push(crate::dto::GraphEdge {
                    source: src_mvp.clone(),
                    target: tgt_mvp,
                    relation: "calls".to_string(),
                    style_class: edge_style_class_for("calls").to_string(),
                });
            }
        }
    }

    let payload = LandingPayload {
        workspace: crate::dto::WorkspaceSummary {
            id: workspace.id.clone(),
            root_path: workspace.root_path.clone(),
            graph_status,
            indexed_at: None,
            symbol_count,
            relation_count,
        },
        nodes,
        edges,
        entry_points,
        hot_paths,
        god_nodes,
        suggested_questions: Vec::new(),
        graph_status,
        truncated,
        truncated_reason,
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
    let response = state.graph.build_architecture(&workspace.root_path).await?;
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

// ============================================================================
// Mermaid C4 export — `GET /api/workspaces/:workspace_id/architecture/mermaid`
// ============================================================================

/// Query params accepted by `GET /api/workspaces/:workspace_id/architecture/mermaid`.
#[derive(Debug, Clone, Deserialize)]
pub struct MermaidQuery {
    /// C4 diagram level: "context" | "container" | "component".
    /// Defaults to "context".
    pub level: Option<String>,
}

impl MermaidQuery {
    /// Parse and validate the level, defaulting to "context".
    pub fn validated(&self) -> Result<C4Level, ExplorerError> {
        let level = self.level.as_deref().unwrap_or("context");
        C4Level::parse(level).map_err(|e| ExplorerError::InvalidQuery(e.to_string()))
    }
}

/// Handler for `GET /api/workspaces/:workspace_id/architecture/mermaid`.
///
/// Renders the C4 architecture as a Mermaid C4 diagram string.
/// Returns `text/plain` content type.
async fn mermaid_handler(
    State(state): State<ApiState>,
    Path(_workspace_id): Path<String>,
    Query(q): Query<MermaidQuery>,
) -> Result<Response, ApiError> {
    let level = q.validated().map_err(ApiError)?;
    let workspace = state.workspace.current_workspace().map_err(ApiError)?;
    let architecture = state.graph.build_architecture(&workspace.root_path).await?;
    let mermaid = c4_mermaid::c4_to_mermaid(&architecture.nodes, &architecture.edges, level);
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        mermaid,
    )
        .into_response())
}

// ============================================================================
// Mermaid trace export — `GET /api/workspaces/:workspace_id/mermaid/trace`
// ============================================================================

/// Query params accepted by `GET /api/workspaces/:workspace_id/mermaid/trace`.
#[derive(Debug, Clone, Deserialize)]
pub struct TraceMermaidQuery {
    /// Trace view kind: "call_graph" | "impact_radius" | "decision_trace" | "vertical_slice".
    pub view_kind: String,
    /// Target symbol id or decision id.
    pub target: String,
}

impl TraceMermaidQuery {
    /// Parse and validate the view_kind.
    pub fn validated(&self) -> Result<TraceMermaidViewKind, ExplorerError> {
        TraceMermaidViewKind::from_str(&self.view_kind).map_err(|e| ExplorerError::InvalidQuery(e))
    }
}

/// Handler for `GET /api/workspaces/:workspace_id/mermaid/trace`.
///
/// Renders a trace (call-graph, impact-radius, decision-trace, vertical-slice)
/// as a Mermaid `flowchart` diagram string.
/// Returns `text/plain` content type, or `400` for invalid view_kind.
async fn trace_mermaid_handler(
    State(state): State<ApiState>,
    Path(_workspace_id): Path<String>,
    Query(q): Query<TraceMermaidQuery>,
) -> Result<Response, ApiError> {
    let view_kind = q.validated().map_err(ApiError)?;

    let graph_query = state.graph.graph_query().ok_or_else(|| {
        ApiError(ExplorerError::GraphUnavailable(
            "call graph not loaded".to_string(),
        ))
    })?;

    // Resolve the target symbol
    let resolved = state
        .graph
        .resolve_symbol(&q.target)
        .await
        .map_err(ApiError)?
        .ok_or_else(|| {
            ApiError(ExplorerError::SymbolNotFound(format!(
                "target not found: {}",
                q.target
            )))
        })?;

    let target = InspectionTarget::Symbol(resolved);
    let trace_ctx = TraceEmitContext {
        graph_query: graph_query.as_ref(),
        target: &target,
    };

    let mermaid = match view_kind {
        TraceMermaidViewKind::CallGraph => call_graph_to_mermaid(&trace_ctx, &q.target),
        TraceMermaidViewKind::ImpactRadius => impact_radius_to_mermaid(&trace_ctx, &q.target),
        #[cfg(feature = "multimodal")]
        TraceMermaidViewKind::DecisionTrace => decision_trace_to_mermaid(&trace_ctx, &q.target)
            .map_err(|_| {
                ApiError(ExplorerError::UnsupportedFormat(
                    "decision_trace not implemented (E24.3)".into(),
                ))
            })?,
        TraceMermaidViewKind::VerticalSlice => vertical_slice_to_mermaid(&trace_ctx, &q.target),
    };

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        mermaid,
    )
        .into_response())
}

// ============================================================================
// Snapshot — `GET /api/workspaces/:workspace_id/snapshot`
// ============================================================================

/// Query params for `GET /api/workspaces/:workspace_id/snapshot`.
#[derive(Debug, Clone, Deserialize)]
pub struct SnapshotQuery {
    /// View kind: "c4_context" | "c4_container" | "c4_component" | "call_graph" | "impact_radius" | "vertical_slice".
    pub view_kind: String,
    /// Target symbol id or entry point id.
    pub target: Option<String>,
    /// Output format: "png" or "svg". Defaults to "png".
    pub format: Option<String>,
}

impl SnapshotQuery {
    /// Validate and parse the query params.
    pub fn validated(&self) -> Result<(SnapshotFormat, SnapshotViewKind), ExplorerError> {
        let format = self
            .format
            .as_deref()
            .map(SnapshotFormat::parse)
            .transpose()
            .map_err(|e| ExplorerError::InvalidQuery(e.to_string()))?
            .unwrap_or(SnapshotFormat::Png);

        let view_kind = SnapshotViewKind::from_str(&self.view_kind)
            .map_err(|s| ExplorerError::InvalidQuery(format!("unknown snapshot view_kind: {s}")))?;
        Ok((format, view_kind))
    }
}

/// Handler for `GET /api/workspaces/:workspace_id/snapshot`.
///
/// Renders a diagram (C4 or trace) as a PNG or SVG image using mmdc.
/// Returns `image/png` or `image/svg+xml` with `Content-Disposition: attachment`.
async fn snapshot_handler(
    State(state): State<ApiState>,
    Path(_workspace_id): Path<String>,
    Query(q): Query<SnapshotQuery>,
) -> Result<Response, SnapshotApiError> {
    let (format, view_kind) = q.validated().map_err(SnapshotApiError::from)?;

    // Require snapshot service to be wired
    let snapshot = state
        .snapshot
        .as_ref()
        .ok_or(SnapshotApiError::FeatureDisabled)?;

    // Emit Mermaid text based on view_kind
    let mermaid = emit_mermaid_for_snapshot(&state, &view_kind, q.target.as_deref())
        .await
        .map_err(SnapshotApiError::from)?;

    // Render to image
    let bytes = snapshot
        .render(&mermaid, format)
        .await
        .map_err(SnapshotApiError::from)?;

    // Sanitize filename: lowercase alphanumeric + underscores only
    let safe_name = q
        .view_kind
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .to_lowercase();

    let filename = format!("{}.{}", safe_name, format.extension());

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, format.content_type()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename).as_str(),
            ),
        ],
        bytes,
    )
        .into_response())
}

/// Emit Mermaid text for a given snapshot view kind using the shared dispatch.
async fn emit_mermaid_for_snapshot(
    state: &ApiState,
    view_kind: &SnapshotViewKind,
    target: Option<&str>,
) -> Result<String, ExplorerError> {
    use crate::domain::snapshot::SnapshotError as SE;
    let graph_svc: &dyn GraphService = &*state.graph;
    let workspace_svc: &dyn WorkspaceService = &*state.workspace;

    crate::domain::snapshot_dispatch::emit_mermaid_for_snapshot(
        graph_svc,
        workspace_svc,
        *view_kind,
        target,
    )
    .await
    .map_err(|se| match se {
        SE::TargetRequiredForTrace => {
            ExplorerError::InvalidQuery("target is required for trace view kinds".to_string())
        }
        SE::EmissionFailed(msg) => ExplorerError::InvalidQuery(msg),
        SE::MermaidEmpty => ExplorerError::InvalidQuery("mermaid text is empty".to_string()),
        SE::SizeLimitExceeded { size } => ExplorerError::InvalidQuery(format!(
            "mermaid text exceeds 1 MB size limit ({size} bytes)"
        )),
        SE::Timeout(dur) => ExplorerError::InvalidQuery(format!("render timed out after {dur:?}")),
        SE::GraphServiceNotWired => {
            ExplorerError::InvalidQuery("graph service not wired".to_string())
        }
        SE::WorkspaceNotWired => {
            ExplorerError::InvalidQuery("workspace service not wired".to_string())
        }
        SE::MmdcNotFound | SE::RenderFailed(_) => ExplorerError::InvalidQuery(se.to_string()),
    })
}

/// API error type for snapshot endpoint with richer status mapping.
enum SnapshotApiError {
    /// Snapshot service not wired (mmdc not configured).
    FeatureDisabled,
    /// Mermaid rendering error.
    Render(SnapshotRenderError),
    /// Explorer domain error during Mermaid emission.
    Explorer(ExplorerError),
}

impl From<ExplorerError> for SnapshotApiError {
    fn from(err: ExplorerError) -> Self {
        SnapshotApiError::Explorer(err)
    }
}

impl From<SnapshotRenderError> for SnapshotApiError {
    fn from(err: SnapshotRenderError) -> Self {
        SnapshotApiError::Render(err)
    }
}

impl IntoResponse for SnapshotApiError {
    fn into_response(self) -> Response {
        let (status, error_msg) = match &self {
            SnapshotApiError::FeatureDisabled => (
                StatusCode::SERVICE_UNAVAILABLE,
                "snapshot feature not available (mmdc not configured)".to_string(),
            ),
            SnapshotApiError::Render(err) => {
                let status = match err {
                    SnapshotRenderError::MermaidEmpty => StatusCode::BAD_REQUEST,
                    SnapshotRenderError::SizeLimitExceeded { .. } => StatusCode::PAYLOAD_TOO_LARGE,
                    SnapshotRenderError::MmdcNotFound => StatusCode::SERVICE_UNAVAILABLE,
                    SnapshotRenderError::RenderFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
                    SnapshotRenderError::Timeout(_) => StatusCode::GATEWAY_TIMEOUT,
                    SnapshotRenderError::GraphServiceNotWired
                    | SnapshotRenderError::WorkspaceNotWired
                    | SnapshotRenderError::TargetRequiredForTrace
                    | SnapshotRenderError::EmissionFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
                };
                (status, err.to_string())
            }
            SnapshotApiError::Explorer(err) => {
                // Map ExplorerError to appropriate status
                let status = match err {
                    ExplorerError::SymbolNotFound(_) => StatusCode::NOT_FOUND,
                    ExplorerError::GraphUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
                    ExplorerError::InvalidQuery(_) => StatusCode::BAD_REQUEST,
                    _ => StatusCode::INTERNAL_SERVER_ERROR,
                };
                (status, err.to_string())
            }
        };

        let body = serde_json::json!({ "error": error_msg });
        (status, Json(body)).into_response()
    }
}

#[derive(Debug, Deserialize)]
struct SpotterQuery {
    q: String,
    kind: Option<String>,
}

async fn spotter(
    State(state): State<ApiState>,
    Path(workspace_id): Path<String>,
    Query(query): Query<SpotterQuery>,
) -> Result<Response, ApiError> {
    Ok(Json(
        state
            .search
            .spotter_search_with_viewspecs(&query.q, query.kind.as_deref(), Some(&workspace_id))
            .await?,
    )
    .into_response())
}

async fn inspect_object(
    State(state): State<ApiState>,
    Path(object_id): Path<String>,
) -> Result<Response, ApiError> {
    Ok(Json(state.search.inspect_object(&object_id).await?).into_response())
}

/// Return knowledge objects linked to `object_id`.
///
/// Phase E27.3 stub: returns empty arrays for adrs/docs/evidence. Real
/// linking logic (via graph `cites`/`cites`/`cites_by` edges) is
/// deferred — see Plan 020. The endpoint shape is locked so the
/// frontend can wire against it.
#[derive(Debug, serde::Serialize)]
struct RelatedKnowledge {
    adrs: Vec<serde_json::Value>,
    docs: Vec<serde_json::Value>,
    evidence: Vec<serde_json::Value>,
}

async fn related_knowledge(
    State(_state): State<ApiState>,
    Path(object_id): Path<String>,
) -> Result<Response, ApiError> {
    let _ = object_id;
    Ok(Json(RelatedKnowledge {
        adrs: Vec::new(),
        docs: Vec::new(),
        evidence: Vec::new(),
    })
    .into_response())
}

async fn available_views(
    State(state): State<ApiState>,
    Path(object_id): Path<String>,
) -> Result<Response, ApiError> {
    Ok(Json(state.view.available_views(&object_id).await?).into_response())
}

async fn affordances_handler(Path(object_id): Path<String>) -> Result<Response, ApiError> {
    let object_type = object_id.split(':').next().unwrap_or(&object_id);
    let affordances = affordance::get_affordances(object_type);
    Ok(Json(affordances).into_response())
}

async fn affordances_by_type_handler(
    Path(object_type): Path<String>,
) -> Result<Response, ApiError> {
    let affordances = affordance::get_affordances(&object_type);
    Ok(Json(affordances).into_response())
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

// ============================================================================
// MoldQL Pattern Profile REST endpoint — T6
// ============================================================================

/// `POST /api/moldql/pattern` — execute a Pattern Profile query.
///
/// Accepts a JSON body with `query`, optional `workspace_id`, and optional
/// `revision_id`. Response mirrors the shape of `/api/moldql/query`.
pub async fn moldql_pattern_handler(
    State(state): State<ApiState>,
    Json(body): Json<PatternQueryBody>,
) -> Result<Response, ApiError> {
    // Resolve pin: explicit body > runtime current_pin > default fallback.
    let (ws_id, rev_id) = match (&body.workspace_id, body.revision_id) {
        (Some(ws), Some(rev)) => (ws.clone(), rev),
        (Some(ws), None) => (ws.clone(), state.revision_tracker.load(Ordering::SeqCst)),
        (None, Some(rev)) => {
            let (ws, _) = state.current_pin();
            (ws, rev)
        }
        (None, None) => state.current_pin(),
    };
    let result = state
        .moldql
        .execute_query_pinned(&body.query, ws_id, rev_id)
        .await
        .map_err(ApiError)?;
    Ok(Json(crate::dto::MoldQLResultDto::from(result)).into_response())
}

/// `GET /api/moldql/pattern/capabilities` — return the v1 supported-feature matrix.
///
/// Returns the same hard-coded matrix as the MCP `moldql_pattern_capabilities` tool.
/// The matrix is defined in openspec/changes/e28-3-moldql-pattern-profile-v1/specs/
/// moldql-pattern-profile/spec.md §"Supported-feature matrix".
async fn moldql_pattern_capabilities_handler() -> Json<serde_json::Value> {
    // Hard-coded v1 supported-feature matrix — mirrors PatternCapabilitiesHandler in MCP.
    Json(serde_json::json!({
        "version": "1.0",
        "profile": "Pattern Profile",
        "features": [
            {"construct": "MATCH (node:Label)", "status": "supported", "notes": "Typed node patterns with label"},
            {"construct": "MATCH (a)-[e:EdgeType]->(b)", "status": "supported", "notes": "Directed edge patterns"},
            {"construct": "MATCH (a)-[e:EdgeType*1..N]->(b)", "status": "supported", "notes": "Bounded path quantifier; N must be finite"},
            {"construct": "MATCH (a)-[e?]->(b)", "status": "supported", "notes": "Zero-or-one quantifier maps to 0..1"},
            {"construct": "MATCH (a)-[e+]->(b)", "status": "supported", "notes": "One-or-more quantifier maps to 1..profile_max_hops"},
            {"construct": "RETURN PATH(a,b)", "status": "supported", "notes": "Path result shape with bindings"},
            {"construct": "RETURN COUNT(e)", "status": "supported", "notes": "Aggregation with ordering and limit"},
            {"construct": "SHORTEST path", "status": "supported", "notes": "Bounded shortest path selection"},
            {"construct": "CREATE/DELETE/SET/MERGE", "status": "unsupported", "notes": "Pattern Profile is read-only; mutations rejected as UnsupportedConstruct"}
        ],
        "compatibility_claims": {
            "cypher": "not_claimed",
            "opencypher": "not_claimed",
            "iso_gql": "not_claimed"
        }
    }))
}

/// GET /api/workspaces/:workspace_id/explorations — list saved explorations for a workspace.
async fn list_explorations(
    State(state): State<ApiState>,
    Path(workspace_id): Path<String>,
) -> Result<Response, ApiError> {
    Ok(Json(state.persistence.list_explorations(&workspace_id).await?).into_response())
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
    let session = state
        .persistence
        .load_exploration_session(&session_id)
        .await?;
    match session {
        Some(s) => Ok(Json(s).into_response()),
        None => Err(ApiError(ExplorerError::NotFound(format!(
            "exploration session {session_id} not found"
        )))),
    }
}

/// POST /api/exploration-sessions/:session_id/artifacts — generate a
/// decision artifact (markdown / html / json replay) for a saved session.
async fn generate_artifact(
    State(state): State<ApiState>,
    Path(session_id): Path<String>,
    Json(request): Json<GenerateArtifactRequest>,
) -> Result<Response, ApiError> {
    let artifact = state
        .persistence
        .generate_artifact(&session_id, request)
        .await?;
    Ok(Json(artifact).into_response())
}

// ============================================================================
// Investigation handlers — ADR-005 INV-1
// ============================================================================

/// POST /api/investigations — create a new investigation.
async fn create_investigation(
    State(state): State<ApiState>,
    Json(request): Json<CreateInvestigationRequest>,
) -> Result<Response, ApiError> {
    let investigation = state
        .investigation
        .as_ref()
        .ok_or_else(|| ApiError(ExplorerError::FeatureDisabled("investigation".into())))?
        .create_investigation(&request.workspace_id, &request.title, &request.goal)
        .await?;
    Ok(Json(investigation).into_response())
}

/// GET /api/investigations — list investigations for a workspace.
async fn list_investigations(
    State(state): State<ApiState>,
    Query(params): Query<ListInvestigationsQuery>,
) -> Result<Response, ApiError> {
    let investigations = state
        .investigation
        .as_ref()
        .ok_or_else(|| ApiError(ExplorerError::FeatureDisabled("investigation".into())))?
        .list_investigations(&params.workspace_id)
        .await?;
    Ok(Json(investigations).into_response())
}

#[derive(serde::Deserialize)]
struct ListInvestigationsQuery {
    workspace_id: String,
}

/// GET /api/investigations/:id — get an investigation by id.
async fn get_investigation(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let investigation = state
        .investigation
        .as_ref()
        .ok_or_else(|| ApiError(ExplorerError::FeatureDisabled("investigation".into())))?
        .get_investigation(&id)
        .await?;
    match investigation {
        Some(inv) => Ok(Json(inv).into_response()),
        None => Err(ApiError(ExplorerError::NotFound(format!(
            "investigation {id} not found"
        )))),
    }
}

/// PUT /api/investigations/:id — update an existing investigation.
async fn update_investigation(
    State(state): State<ApiState>,
    Json(request): Json<UpdateInvestigationRequest>,
) -> Result<Response, ApiError> {
    // Fetch existing to preserve created_at and workspace_id.
    let existing = state
        .investigation
        .as_ref()
        .ok_or_else(|| ApiError(ExplorerError::FeatureDisabled("investigation".into())))?
        .get_investigation(&request.id)
        .await?
        .ok_or_else(|| {
            ApiError(ExplorerError::NotFound(format!(
                "investigation {} not found",
                request.id
            )))
        })?;

    let investigation = Investigation {
        id: request.id,
        workspace_id: request.workspace_id,
        title: request.title,
        goal: request.goal,
        status: request.status.into(),
        entry_point: request.entry_point,
        panes: request.panes,
        evidence: request.evidence,
        artifacts: request.artifacts,
        narrative: request.narrative,
        related_adrs: request.related_adrs,
        created_at: existing.created_at,
        updated_at: time::OffsetDateTime::now_utc(),
    };
    state
        .investigation
        .as_ref()
        .ok_or_else(|| ApiError(ExplorerError::FeatureDisabled("investigation".into())))?
        .update_investigation(investigation)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })).into_response())
}

/// DELETE /api/investigations/:id — delete an investigation.
async fn delete_investigation(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    state
        .investigation
        .as_ref()
        .ok_or_else(|| ApiError(ExplorerError::FeatureDisabled("investigation".into())))?
        .delete_investigation(&id)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })).into_response())
}

/// POST /api/investigations/:id/evidence — pin evidence to an investigation.
async fn pin_evidence(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(request): Json<PinEvidenceRequest>,
) -> Result<Response, ApiError> {
    let _ = state
        .investigation
        .as_ref()
        .ok_or_else(|| ApiError(ExplorerError::FeatureDisabled("investigation".into())))?
        .get_investigation(&id)
        .await?
        .ok_or_else(|| {
            ApiError(ExplorerError::NotFound(format!(
                "investigation {} not found",
                id
            )))
        })?;

    let evidence = Evidence {
        id: format!(
            "evi_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ),
        object_id: request.object_id,
        view_id: request.view_id,
        note: request.note,
        pinned_at: time::OffsetDateTime::now_utc(),
    };

    state
        .investigation
        .as_ref()
        .ok_or_else(|| ApiError(ExplorerError::FeatureDisabled("investigation".into())))?
        .add_evidence(&id, evidence)
        .await?;

    Ok(Json(serde_json::json!({ "ok": true })).into_response())
}

/// POST /api/investigations/:id/artifacts — add an artifact to an investigation (ADR-005 E21-6 + ADR-010 E24.1).
async fn add_investigation_artifact(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(request): Json<AddArtifactRequest>,
) -> Result<Response, ApiError> {
    let facade = state
        .investigation
        .as_ref()
        .ok_or_else(|| ApiError(ExplorerError::FeatureDisabled("investigation".into())))?;

    // Delegate to the layered add_artifact method (mirrors add_evidence pattern).
    let artifact = facade.add_artifact(&id, request).await.map_err(|e| {
        let msg = e.to_string();
        if msg.contains("not found") {
            ApiError(ExplorerError::NotFound(format!(
                "investigation {} not found",
                id
            )))
        } else {
            ApiError(e)
        }
    })?;

    Ok(Json(serde_json::json!({ "ok": true, "artifact": artifact })).into_response())
}

/// GET /api/investigations/:id/evidence-pack — get evidence pack view for an investigation (ADR-005 E21-3).
async fn get_investigation_evidence_pack(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let investigation = state
        .investigation
        .as_ref()
        .ok_or_else(|| ApiError(ExplorerError::FeatureDisabled("investigation".into())))?
        .get_investigation(&id)
        .await?
        .ok_or_else(|| {
            ApiError(ExplorerError::NotFound(format!(
                "investigation {} not found",
                id
            )))
        })?;

    let view = crate::domain::views::build_evidence_pack(&investigation);
    Ok(Json(view).into_response())
}

/// GET /api/investigations/:id/composed-narrative — get composed narrative view for an investigation (ADR-005 E21-4).
async fn get_investigation_composed_narrative(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let investigation = state
        .investigation
        .as_ref()
        .ok_or_else(|| ApiError(ExplorerError::FeatureDisabled("investigation".into())))?
        .get_investigation(&id)
        .await?
        .ok_or_else(|| {
            ApiError(ExplorerError::NotFound(format!(
                "investigation {} not found",
                id
            )))
        })?;

    let view = crate::domain::views::build_investigation_narrative(&investigation);
    Ok(Json(view).into_response())
}

/// POST /api/investigations/:id/artifacts/:aid/regenerate — ADR-010 E24.1.
/// Regenerates a diagram artifact from its provenance and persists the new content.
async fn regenerate_artifact(
    State(state): State<ApiState>,
    Path((id, aid)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    use crate::domain::diagram_regen::DiagramRegenerator;

    let facade = state
        .investigation
        .as_ref()
        .ok_or_else(|| ApiError(ExplorerError::FeatureDisabled("investigation".into())))?;

    // Fetch the investigation to locate the artifact.
    let mut investigation = facade.get_investigation(&id).await?.ok_or_else(|| {
        ApiError(ExplorerError::NotFound(format!(
            "investigation {} not found",
            id
        )))
    })?;

    // Find the artifact by id.
    let artifact_idx = investigation
        .artifacts
        .iter()
        .position(|a| a.id == aid)
        .ok_or_else(|| {
            ApiError(ExplorerError::NotFound(format!(
                "artifact {} not found in investigation {}",
                aid, id
            )))
        })?;

    let provenance = investigation.artifacts[artifact_idx]
        .provenance
        .as_ref()
        .ok_or_else(|| {
            ApiError(ExplorerError::InvalidInput(
                "artifact does not have provenance metadata — cannot regenerate".into(),
            ))
        })?
        .clone();

    // Regenerate using the dispatch table.
    let graph_svc: &dyn GraphService = &*state.graph;
    let workspace_svc: &dyn WorkspaceService = &*state.workspace;
    let new_content = DiagramRegenerator::regenerate(&provenance, graph_svc, workspace_svc)
        .await
        .map_err(|e| ApiError(ExplorerError::InvalidInput(e.to_string())))?;

    // Update the artifact content and persist.
    investigation.artifacts[artifact_idx].content = new_content.clone();
    investigation.updated_at = time::OffsetDateTime::now_utc();
    facade
        .update_investigation(investigation.clone())
        .await
        .map_err(ApiError::from)?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "artifact_id": aid,
        "content": new_content,
    }))
    .into_response())
}

pub struct ApiError(ExplorerError);

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
            ExplorerError::UnsupportedFormat(_) => StatusCode::BAD_REQUEST,
            ExplorerError::Conflict(_) => StatusCode::CONFLICT,
            ExplorerError::FeatureDisabled(_) => StatusCode::SERVICE_UNAVAILABLE,
            ExplorerError::GraphNotReady => StatusCode::SERVICE_UNAVAILABLE,
            ExplorerError::GraphUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            ExplorerError::QualityUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
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
