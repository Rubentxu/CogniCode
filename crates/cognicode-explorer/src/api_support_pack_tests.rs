//! Integration tests for `GET /api/decisions/:id/support-pack`.
//!
//! Uses an in-memory `GraphRepository` with multimodal nodes + edges.
//! Tests cover handler success, five-pane response shape, and partial
//! failure scenarios.
//!
//! This module is gated on `#[cfg(feature = "multimodal")]` in
//! `lib.rs`, so all items here can assume multimodal is active.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use cognicode_core::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};
use cognicode_core::domain::value_objects::edge_kind::EdgeKind;
use cognicode_core::domain::value_objects::node_kind::NodeKind;
use cognicode_core::domain::value_objects::provenance::Provenance;
use cognicode_core::domain::value_objects::SymbolKind;
use tower::ServiceExt;

use crate::adapters::InMemoryGraphRepository;
use crate::api::{ApiState, router_with_state};
use crate::facades::{
    GraphService, MoldQLService, PersistenceService, SearchService, ViewService, WorkspaceService,
    graph::GraphServiceImpl,
};
use crate::ports::graph_repository::GraphRepository;
use crate::ports::source_reader::SourceReader;
use crate::ports::symbol_repository::{
    GraphStats, SymbolRepository,
};
use cognicode_core::domain::aggregates::SymbolId;

// ---------------------------------------------------------------------------
// Stub implementations
// ---------------------------------------------------------------------------

struct EmptySymbolRepo;

impl SymbolRepository for EmptySymbolRepo {
    fn resolve(&self, _id: &SymbolId) -> crate::error::ExplorerResult<Option<crate::ports::symbol_repository::ResolvedSymbol>> {
        Ok(None)
    }
    fn find_symbols_by_name(
        &self,
        _name: &str,
    ) -> crate::error::ExplorerResult<Vec<crate::ports::symbol_repository::ResolvedSymbol>> {
        Ok(Vec::new())
    }
    fn find_symbols_by_file(
        &self,
        _file: &str,
    ) -> crate::error::ExplorerResult<Vec<crate::ports::symbol_repository::ResolvedSymbol>> {
        Ok(Vec::new())
    }
    fn module_list(&self) -> Vec<String> {
        Vec::new()
    }
    fn all_symbols(&self) -> crate::error::ExplorerResult<Vec<crate::ports::symbol_repository::ResolvedSymbol>> {
        Ok(Vec::new())
    }
    fn graph_stats(&self) -> GraphStats {
        GraphStats::default()
    }
}

struct EmptyReader;
impl SourceReader for EmptyReader {
    fn read_source(&self, _file: &str) -> crate::error::ExplorerResult<String> {
        Ok(String::new())
    }
    fn read_lines(
        &self,
        _file: &str,
        _start: u32,
        _end: u32,
    ) -> crate::error::ExplorerResult<Vec<(u32, String)>> {
        Ok(Vec::new())
    }
}

// Minimal mocks for the 5 non-graph facades needed by ApiState
#[derive(Clone)]
struct MockWorkspaceService;
#[async_trait]
impl WorkspaceService for MockWorkspaceService {
    async fn open_workspace(
        &self,
        _request: crate::dto::OpenWorkspaceRequest,
    ) -> crate::ExplorerResult<crate::dto::WorkspaceSummary> {
        Err(crate::error::ExplorerError::WorkspaceNotFound("mock".into()))
    }
    fn current_workspace(&self) -> crate::ExplorerResult<crate::dto::WorkspaceSummary> {
        Err(crate::error::ExplorerError::WorkspaceNotFound("mock".into()))
    }
}

#[derive(Clone)]
struct MockSearchService;
#[async_trait]
impl SearchService for MockSearchService {
    async fn spotter_search(
        &self,
        _query: &str,
        _kind: Option<&str>,
    ) -> crate::ExplorerResult<Vec<crate::dto::SpotterResult>> {
        Ok(vec![])
    }
    async fn spotter_search_with_viewspecs(
        &self,
        _query: &str,
        _kind: Option<&str>,
        _workspace_id: Option<&str>,
    ) -> crate::ExplorerResult<Vec<crate::dto::SpotterSearchResult>> {
        Ok(vec![])
    }
    async fn inspect_object(
        &self,
        _object_id: &str,
    ) -> crate::ExplorerResult<crate::dto::InspectableObjectSummary> {
        Err(crate::error::ExplorerError::ObjectNotFound("mock".into()))
    }
}

#[derive(Clone)]
struct MockViewService;
#[async_trait]
impl ViewService for MockViewService {
    async fn available_views(
        &self,
        _object_id: &str,
    ) -> crate::ExplorerResult<Vec<crate::dto::ViewDescriptorDto>> {
        Ok(vec![])
    }
    async fn contextual_view(
        &self,
        _object_id: &str,
        _view_id: &str,
    ) -> crate::ExplorerResult<crate::dto::ContextualView> {
        Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn build_contextual_graph(
        &self,
        _focus_id: &str,
        _level: &str,
        _depth: u8,
        _max_nodes: usize,
    ) -> crate::ExplorerResult<crate::dto::ContextualGraphResponse> {
        Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn available_lenses(
        &self,
        _object_id: &str,
    ) -> crate::ExplorerResult<Vec<crate::dto::LensDescriptor>> {
        Ok(vec![])
    }
    async fn apply_lens(
        &self,
        _object_id: &str,
        _lens_id: &str,
    ) -> crate::ExplorerResult<crate::dto::LensResult> {
        Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn execute_view_spec(
        &self,
        _spec: &crate::dto::ViewSpec,
        _object_id: &str,
    ) -> crate::ExplorerResult<crate::dto::ContextualView> {
        Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
    }
}

#[derive(Clone)]
struct MockPersistenceService;
#[async_trait]
impl PersistenceService for MockPersistenceService {
    async fn generate_artifact(
        &self,
        _exploration_id: &str,
        _request: crate::dto::GenerateArtifactRequest,
    ) -> crate::ExplorerResult<crate::dto::DecisionArtifactSummary> {
        Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn save_view_spec(
        &self,
        _spec: &crate::dto::ViewSpec,
        _workspace_id: &str,
        _owner: &str,
    ) -> crate::ExplorerResult<()> {
        Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn load_view_spec(
        &self,
        _id: &str,
        _workspace_id: &str,
        _owner: &str,
    ) -> crate::ExplorerResult<Option<crate::dto::ViewSpec>> {
        Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn list_view_specs(
        &self,
        _workspace_id: &str,
        _owner: &str,
    ) -> crate::ExplorerResult<Vec<crate::dto::ViewSpec>> {
        Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn delete_view_spec(
        &self,
        _id: &str,
        _workspace_id: &str,
        _owner: &str,
    ) -> crate::ExplorerResult<bool> {
        Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn save_exploration_session(
        &self,
        _request: crate::dto::SaveExplorationSessionRequest,
    ) -> crate::ExplorerResult<crate::dto::ExplorationSession> {
        Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn load_exploration_session(
        &self,
        _session_id: &str,
    ) -> crate::ExplorerResult<Option<crate::dto::ExplorationSession>> {
        Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn list_explorations(
        &self,
        _workspace_id: &str,
    ) -> crate::ExplorerResult<Vec<crate::dto::ExplorationSession>> {
        Ok(vec![])
    }
}

#[derive(Clone)]
struct MockMoldQLService;
#[async_trait]
impl MoldQLService for MockMoldQLService {
    async fn execute_query(
        &self,
        _query: &str,
    ) -> crate::ExplorerResult<crate::moldql::MoldQLResult> {
        Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn execute_query_with_target(
        &self,
        _query: &str,
        _target: crate::moldql::compile::CompileTarget,
    ) -> crate::ExplorerResult<crate::moldql::MoldQLResult> {
        Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn execute_query_pinned(
        &self,
        _query: &str,
        _workspace_id: String,
        _revision_id: u64,
    ) -> crate::ExplorerResult<crate::moldql::MoldQLResult> {
        Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
    }
}

// ---------------------------------------------------------------------------
// Fixture: a decision node with supporting symbol, doc, and evidence nodes
// ---------------------------------------------------------------------------

/// Graph fixture for support-pack tests:
///   Decision A ──Justifies(0.9)──► Symbol X (primary, highest confidence)
///   Decision A ──Cites──► Doc Y
///   Decision A ──CorroboratedBy──► Evidence Z
fn support_pack_fixture() -> (Vec<GraphNode>, Vec<GraphEdge>) {
    let nodes = vec![
        GraphNode {
            id: NodeId::new("A"),
            kind: NodeKind::Decision,
            label: "Decision A".to_string(),
            source_path: None,
            properties: HashMap::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
        GraphNode {
            id: NodeId::new("X"),
            kind: NodeKind::Symbol(SymbolKind::Function),
            label: "Symbol X".to_string(),
            source_path: Some(std::path::PathBuf::from("src/x.rs")),
            properties: HashMap::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
        GraphNode {
            id: NodeId::new("Y"),
            kind: NodeKind::Doc,
            label: "Doc Y".to_string(),
            source_path: None,
            properties: HashMap::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
        GraphNode {
            id: NodeId::new("Z"),
            kind: NodeKind::Evidence,
            label: "Evidence Z".to_string(),
            source_path: None,
            properties: HashMap::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
    ];
    let edges = vec![
        GraphEdge {
            source: NodeId::new("A"),
            target: NodeId::new("X"),
            kind: EdgeKind::Justifies,
            provenance: Provenance::Extracted,
            confidence: 0.9,
            metadata: HashMap::new(),
        },
        GraphEdge {
            source: NodeId::new("A"),
            target: NodeId::new("Y"),
            kind: EdgeKind::Cites,
            provenance: Provenance::Extracted,
            confidence: 0.8,
            metadata: HashMap::new(),
        },
        GraphEdge {
            source: NodeId::new("A"),
            target: NodeId::new("Z"),
            kind: EdgeKind::CorroboratedBy,
            provenance: Provenance::Tested,
            confidence: 0.7,
            metadata: HashMap::new(),
        },
    ];
    (nodes, edges)
}

/// Build the Axum router for support-pack integration tests.
fn support_pack_app() -> axum::Router {
    let (nodes, edges) = support_pack_fixture();
    let graph_repo: Arc<dyn GraphRepository> = Arc::new(InMemoryGraphRepository::new(nodes, edges));
    let empty_repo = Arc::new(EmptySymbolRepo);
    // graph_query = None is fine; RiskMap and ChangeImpactStory panes will be
    // degraded but the overall pack still returns 200 with five panes.
    let graph = Arc::new(GraphServiceImpl::new(empty_repo, None));
    let state = ApiState::new(
        Arc::new(MockWorkspaceService),
        Arc::new(MockSearchService),
        Arc::new(MockViewService),
        Arc::new(MockPersistenceService),
        Arc::new(MockMoldQLService),
        graph,
    )
    .with_graph_repo(graph_repo);
    router_with_state(state)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn support_pack_returns_five_panes_in_stable_order() {
    let app = support_pack_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/decisions/A/support-pack")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    let pack: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");

    let panes = pack.get("panes").expect("panes field").as_array().expect("panes array");
    assert_eq!(panes.len(), 5, "expected exactly five panes");

    // Stable order is defined by the builder:
    // decision_graph, architecture_rationale, evidence_pack, risk_map, change_impact_story
    let expected_order = [
        "decision_graph",
        "architecture_rationale",
        "evidence_pack",
        "risk_map",
        "change_impact_story",
    ];
    for (i, expected_id) in expected_order.iter().enumerate() {
        let pane = panes.get(i).expect("pane exists");
        assert_eq!(
            pane.get("view_id").expect("view_id field").as_str().expect("view_id string"),
            *expected_id,
            "pane {} should be {}",
            i,
            expected_id
        );
    }
}

#[tokio::test]
async fn support_pack_returns_404_for_unknown_decision_id() {
    // When the decision is not found in the graph repository,
    // the handler returns 404 Not Found.
    let app = support_pack_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/decisions/nonexistent/support-pack")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn support_pack_panes_have_valid_status_field() {
    let app = support_pack_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/decisions/A/support-pack")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    let pack: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");

    let panes = pack.get("panes").expect("panes field").as_array().expect("panes array");
    for pane in panes {
        // Every pane must have a `status` field: "ok", "degraded", or "failed"
        // (snake_case via PaneStatus Serialize with rename_all = "snake_case")
        let status = pane.get("status").expect("status field");
        assert!(
            status.is_object(),
            "status should be an object: {:?}",
            status
        );
        let status_obj = status.as_object().unwrap();
        let inner_status = status_obj.get("status").expect("inner status field");
        assert!(
            inner_status.is_string(),
            "inner status should be a string: {:?}",
            inner_status
        );
        let status_str = inner_status.as_str().unwrap();
        assert!(
            ["ok", "degraded", "failed"].contains(&status_str),
            "status must be ok|degraded|failed, got: {}",
            status_str
        );
    }
}

// --- API payload field shape tests ---

#[tokio::test]
async fn support_pack_top_level_has_decision_id_and_panes() {
    let app = support_pack_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/decisions/A/support-pack")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    let pack: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");

    // Top-level decision_id is required
    let decision_id = pack.get("decision_id").expect("decision_id field at top level");
    assert!(
        decision_id.is_string(),
        "decision_id should be a string, got: {:?}",
        decision_id
    );
    assert_eq!(
        decision_id.as_str().unwrap(),
        "A",
        "top-level decision_id should be A"
    );

    // Top-level panes array is required
    let panes = pack.get("panes").expect("panes field at top level");
    assert!(
        panes.is_array(),
        "panes should be an array, got: {:?}",
        panes
    );
}

#[tokio::test]
async fn support_pack_pane_fields_are_complete() {
    let app = support_pack_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/decisions/A/support-pack")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    let pack: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");

    let panes = pack.get("panes").expect("panes field").as_array().expect("panes array");
    assert_eq!(panes.len(), 5, "expected exactly five panes");

    // Check the first pane (decision_graph) has all required fields
    // PackPane fields: view_id, title, view, status
    let first_pane = panes.get(0).expect("first pane");
    let pane_obj = first_pane.as_object().expect("pane should be an object");

    // Required pane-level fields per DecisionSupportPack struct
    assert!(
        pane_obj.contains_key("view_id"),
        "pane should have view_id field"
    );
    assert!(
        pane_obj.contains_key("title"),
        "pane should have title field"
    );
    assert!(
        pane_obj.contains_key("view"),
        "pane should have view field (Option<ContextualView>)"
    );
    assert!(
        pane_obj.contains_key("status"),
        "pane should have status field"
    );

    // view_id should be "decision_graph"
    let view_id = pane_obj.get("view_id").unwrap();
    assert_eq!(
        view_id.as_str().unwrap(),
        "decision_graph",
        "first pane view_id should be decision_graph"
    );

    // title should be a string
    let title = pane_obj.get("title").unwrap();
    assert!(
        title.is_string(),
        "title should be a string, got: {:?}",
        title
    );

    // view should be an object (the ContextualView for successful panes)
    let view = pane_obj.get("view").unwrap();
    assert!(
        view.is_object(),
        "view should be the ContextualView object, got: {:?}",
        view
    );

    // The view object should have view_id and view_kind
    let view_obj = view.as_object().unwrap();
    assert!(
        view_obj.contains_key("view_id"),
        "ContextualView should have view_id"
    );
    assert!(
        view_obj.contains_key("view_kind"),
        "ContextualView should have view_kind"
    );
    assert!(
        view_obj.contains_key("renderer_kind"),
        "ContextualView should have renderer_kind"
    );
}

#[tokio::test]
async fn support_pack_panes_have_valid_status_object() {
    let app = support_pack_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/decisions/A/support-pack")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);

    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    let pack: serde_json::Value = serde_json::from_slice(&body).expect("valid JSON");

    let panes = pack.get("panes").expect("panes field").as_array().expect("panes array");
    for pane in panes {
        let pane_obj = pane.as_object().expect("pane must be object");
        // status field is required and should be an object with inner "status" string
        let status = pane_obj.get("status").expect("status field required");
        let status_obj = status.as_object().expect("status must be object");
        let inner = status_obj.get("status").expect("inner status string required");
        assert!(
            inner.is_string(),
            "inner status must be string, got: {:?}",
            inner
        );
        let inner_str = inner.as_str().unwrap();
        assert!(
            ["ok", "degraded", "failed"].contains(&inner_str),
            "status must be ok|degraded|failed, got: {}",
            inner_str
        );
    }
}
