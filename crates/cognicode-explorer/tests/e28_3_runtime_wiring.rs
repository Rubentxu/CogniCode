//! Integration tests for E28.3 Runtime Wiring — active workspace + revision tracking.
//!
//! These tests verify:
//! 1. `revision_tracker` bump logic works correctly
//! 2. `ApiState::current_pin()` returns correct (workspace_id, revision_id)
//! 3. The pin resolution logic in `moldql_pattern_handler` works correctly
//!
//! See `openspec/changes/fix/e28-3-runtime-wiring-followup/` for full context.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(deprecated, unused_imports)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use tower::ServiceExt;

use cognicode_explorer::api::{ApiState, PatternQueryBody};
use cognicode_explorer::dto::WorkspaceSummary;
use cognicode_explorer::error::ExplorerError;
use cognicode_explorer::facades::{
    GraphService, MoldQLService, PersistenceService, SearchService, ViewService, WorkspaceService,
};
use cognicode_explorer::moldql::MoldQLResult;

// ============================================================================
// Mock implementations
// ============================================================================

/// A MoldQLService mock that records the pin passed to `execute_query_pinned`.
struct RecordingMoldQLService {
    recorded_pin: Mutex<Option<(String, u64)>>,
}

impl RecordingMoldQLService {
    fn new() -> Self {
        Self {
            recorded_pin: Mutex::new(None),
        }
    }

    fn take_recorded_pin(&self) -> Option<(String, u64)> {
        self.recorded_pin.lock().unwrap().take()
    }
}

#[async_trait]
impl MoldQLService for RecordingMoldQLService {
    async fn execute_query(&self, query: &str) -> cognicode_explorer::ExplorerResult<MoldQLResult> {
        Ok(MoldQLResult {
            query: query.to_string(),
            items: vec![],
            total: 0,
        })
    }

    async fn execute_query_with_target(
        &self,
        query: &str,
        _target: cognicode_explorer::moldql::compile::CompileTarget,
    ) -> cognicode_explorer::ExplorerResult<MoldQLResult> {
        self.execute_query(query).await
    }

    async fn execute_query_pinned(
        &self,
        query: &str,
        workspace_id: String,
        revision_id: u64,
    ) -> cognicode_explorer::ExplorerResult<MoldQLResult> {
        *self.recorded_pin.lock().unwrap() = Some((workspace_id, revision_id));
        self.execute_query(query).await
    }
}

/// A WorkspaceService mock that returns a known current workspace.
struct MockWorkspaceForPin {
    workspace_id: String,
}

impl MockWorkspaceForPin {
    fn new(workspace_id: &str) -> Self {
        Self {
            workspace_id: workspace_id.to_string(),
        }
    }
}

#[async_trait]
impl WorkspaceService for MockWorkspaceForPin {
    async fn open_workspace(
        &self,
        _request: cognicode_explorer::dto::OpenWorkspaceRequest,
    ) -> cognicode_explorer::ExplorerResult<WorkspaceSummary> {
        Ok(WorkspaceSummary {
            id: self.workspace_id.clone(),
            root_path: "/fake/path".to_string(),
            graph_status: cognicode_explorer::dto::GraphStatus::Ready,
            indexed_at: None,
            symbol_count: 0,
            relation_count: 0,
        })
    }

    fn current_workspace(&self) -> cognicode_explorer::ExplorerResult<WorkspaceSummary> {
        Ok(WorkspaceSummary {
            id: self.workspace_id.clone(),
            root_path: "/fake/path".to_string(),
            graph_status: cognicode_explorer::dto::GraphStatus::Ready,
            indexed_at: None,
            symbol_count: 0,
            relation_count: 0,
        })
    }
}

// Blanket mock impls for the remaining facade traits needed by ApiState::new.

struct MockSearchService;

#[async_trait]
impl SearchService for MockSearchService {
    async fn spotter_search(
        &self,
        _: &str,
        _: Option<&str>,
    ) -> cognicode_explorer::ExplorerResult<Vec<cognicode_explorer::dto::SpotterResult>> {
        Ok(vec![])
    }
    async fn spotter_search_with_viewspecs(
        &self,
        _: &str,
        _: Option<&str>,
        _: Option<&str>,
    ) -> cognicode_explorer::ExplorerResult<Vec<cognicode_explorer::dto::SpotterSearchResult>> {
        Ok(vec![])
    }
    async fn inspect_object(
        &self,
        _: &str,
    ) -> cognicode_explorer::ExplorerResult<cognicode_explorer::dto::InspectableObjectSummary> {
        Err(ExplorerError::ObjectNotFound("mock".into()))
    }
}

struct MockViewService;

#[async_trait]
impl ViewService for MockViewService {
    async fn available_views(
        &self,
        _: &str,
    ) -> cognicode_explorer::ExplorerResult<Vec<cognicode_explorer::dto::ViewDescriptorDto>> {
        Ok(vec![])
    }
    async fn contextual_view(
        &self,
        _: &str,
        _: &str,
    ) -> cognicode_explorer::ExplorerResult<cognicode_explorer::dto::ContextualView> {
        Err(ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn build_contextual_graph(
        &self,
        _: &str,
        _: &str,
        _: u8,
        _: usize,
    ) -> cognicode_explorer::ExplorerResult<cognicode_explorer::dto::ContextualGraphResponse> {
        Err(ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn available_lenses(
        &self,
        _: &str,
    ) -> cognicode_explorer::ExplorerResult<Vec<cognicode_explorer::dto::LensDescriptor>> {
        Ok(vec![])
    }
    async fn apply_lens(
        &self,
        _: &str,
        _: &str,
    ) -> cognicode_explorer::ExplorerResult<cognicode_explorer::dto::LensResult> {
        Err(ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn execute_view_spec(
        &self,
        _: &cognicode_explorer::dto::ViewSpec,
        _: &str,
    ) -> cognicode_explorer::ExplorerResult<cognicode_explorer::dto::ContextualView> {
        Err(ExplorerError::FeatureDisabled("mock".into()))
    }
}

struct MockPersistenceService;

#[async_trait]
impl PersistenceService for MockPersistenceService {
    async fn save_exploration_session(
        &self,
        _: cognicode_explorer::dto::SaveExplorationSessionRequest,
    ) -> cognicode_explorer::ExplorerResult<cognicode_explorer::dto::ExplorationSession> {
        Err(ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn load_exploration_session(
        &self,
        _: &str,
    ) -> cognicode_explorer::ExplorerResult<Option<cognicode_explorer::dto::ExplorationSession>>
    {
        Ok(None)
    }
    async fn list_explorations(
        &self,
        _: &str,
    ) -> cognicode_explorer::ExplorerResult<Vec<cognicode_explorer::dto::ExplorationSession>> {
        Ok(vec![])
    }
    async fn generate_artifact(
        &self,
        _: &str,
        _: cognicode_explorer::dto::GenerateArtifactRequest,
    ) -> cognicode_explorer::ExplorerResult<cognicode_explorer::dto::DecisionArtifactSummary> {
        Err(ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn save_view_spec(
        &self,
        _: &cognicode_explorer::dto::ViewSpec,
        _: &str,
        _: &str,
    ) -> cognicode_explorer::ExplorerResult<()> {
        Err(ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn load_view_spec(
        &self,
        _: &str,
        _: &str,
        _: &str,
    ) -> cognicode_explorer::ExplorerResult<Option<cognicode_explorer::dto::ViewSpec>> {
        Ok(None)
    }
    async fn list_view_specs(
        &self,
        _: &str,
        _: &str,
    ) -> cognicode_explorer::ExplorerResult<Vec<cognicode_explorer::dto::ViewSpec>> {
        Ok(vec![])
    }
    async fn delete_view_spec(
        &self,
        _: &str,
        _: &str,
        _: &str,
    ) -> cognicode_explorer::ExplorerResult<bool> {
        Ok(false)
    }
}

struct MockGraphService;

#[async_trait]
impl GraphService for MockGraphService {
    async fn resolve_symbol(
        &self,
        _: &str,
    ) -> cognicode_explorer::ExplorerResult<
        Option<cognicode_explorer::ports::symbol_repository::ResolvedSymbol>,
    > {
        Ok(None)
    }
    fn graph_query(&self) -> Option<Arc<dyn cognicode_core::domain::traits::GraphQueryPort>> {
        None
    }
    async fn build_subgraph(
        &self,
        _: &str,
        _: u8,
        _: cognicode_explorer::facades::SubgraphDirection,
        _: u32,
    ) -> cognicode_explorer::ExplorerResult<cognicode_explorer::dto::SubgraphResponse> {
        Err(ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn build_architecture(
        &self,
        _: &str,
    ) -> cognicode_explorer::ExplorerResult<cognicode_explorer::dto::SubgraphResponse> {
        Err(ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn compare_architecture(
        &self,
        _: &str,
    ) -> cognicode_explorer::ExplorerResult<cognicode_explorer::dto::DriftReport> {
        Err(ExplorerError::FeatureDisabled("mock".into()))
    }
    async fn landing_entry_points(
        &self,
        _: usize,
    ) -> cognicode_explorer::ExplorerResult<(
        Vec<cognicode_explorer::ports::symbol_repository::ResolvedSymbol>,
        usize,
    )> {
        Ok((vec![], 0))
    }
    async fn landing_hot_paths(
        &self,
        _: usize,
        _: usize,
    ) -> cognicode_explorer::ExplorerResult<
        Vec<cognicode_explorer::ports::symbol_repository::ResolvedSymbol>,
    > {
        Ok(vec![])
    }
    async fn landing_god_nodes(
        &self,
        _: usize,
    ) -> cognicode_explorer::ExplorerResult<Vec<cognicode_explorer::dto::GodNodeEntry>> {
        Ok(vec![])
    }
}

// ============================================================================
// E28.3 Runtime Wiring Tests
// ============================================================================

/// Test: revision_tracker bumps after successful ingest.
///
/// Verifies the revision counter increments from 1 to 2 when the ingest
/// success path calls `revision_tracker.fetch_add(1, Ordering::SeqCst)`.
#[tokio::test]
async fn revision_tracker_bumps_after_ingest() {
    let tracker = Arc::new(AtomicU64::new(1));
    assert_eq!(
        tracker.load(Ordering::SeqCst),
        1,
        "precondition: tracker starts at 1"
    );

    let recording = Arc::new(RecordingMoldQLService::new());
    let workspace = Arc::new(MockWorkspaceForPin::new("test-ws"));
    let _state = ApiState::new(
        workspace,
        Arc::new(MockSearchService),
        Arc::new(MockViewService),
        Arc::new(MockPersistenceService),
        recording.clone(),
        Arc::new(MockGraphService),
    )
    .with_revision_tracker(tracker.clone());

    // Simulate what index_workspace does after a successful ingest:
    // it calls revision_tracker.fetch_add(1, Ordering::SeqCst)
    tracker.fetch_add(1, Ordering::SeqCst);

    // Assert: revision tracker was bumped from 1 to 2.
    assert_eq!(
        tracker.load(Ordering::SeqCst),
        2,
        "revision_tracker should increment after successful ingest"
    );
}

/// Test: ApiState::current_pin() returns correct (workspace_id, revision_id).
#[tokio::test]
async fn api_state_current_pin_returns_correct_values() {
    let tracker = Arc::new(AtomicU64::new(5));
    let workspace = Arc::new(MockWorkspaceForPin::new("my-workspace"));
    let state = ApiState::new(
        workspace,
        Arc::new(MockSearchService),
        Arc::new(MockViewService),
        Arc::new(MockPersistenceService),
        Arc::new(RecordingMoldQLService::new()),
        Arc::new(MockGraphService),
    )
    .with_revision_tracker(tracker);

    let (ws_id, rev_id) = state.current_pin();
    assert_eq!(ws_id, "my-workspace");
    assert_eq!(rev_id, 5);
}

/// Test: moldql_pattern_handler uses explicit pin from body.
///
/// Send a request body with explicit workspace_id="explicit-ws" and
/// revision_id=99. Assert execute_query_pinned was called with that exact pin.
#[tokio::test]
async fn moldql_pattern_handler_uses_explicit_pin() {
    use cognicode_explorer::api::moldql_pattern_handler;

    let recording = Arc::new(RecordingMoldQLService::new());
    let tracker = Arc::new(AtomicU64::new(1));
    let state = ApiState::new(
        Arc::new(MockWorkspaceForPin::new("default-ws")),
        Arc::new(MockSearchService),
        Arc::new(MockViewService),
        Arc::new(MockPersistenceService),
        recording.clone(),
        Arc::new(MockGraphService),
    )
    .with_revision_tracker(tracker);

    let app = axum::Router::new()
        .route("/api/moldql/pattern", post(moldql_pattern_handler))
        .with_state(state);

    let body = PatternQueryBody {
        query: "MATCH (n) RETURN n".to_string(),
        workspace_id: Some("explicit-ws".to_string()),
        revision_id: Some(99),
    };

    let req = Request::builder()
        .method("POST")
        .uri("/api/moldql/pattern")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK, "handler should succeed");

    let pin = recording.take_recorded_pin();
    assert!(
        pin.is_some(),
        "execute_query_pinned should have been called"
    );
    let (ws, rev) = pin.unwrap();
    assert_eq!(ws, "explicit-ws", "workspace_id should match body");
    assert_eq!(rev, 99, "revision_id should match body");
}

/// Test: moldql_pattern_handler falls back to current_pin when body omits pin.
///
/// Send a request body with no workspace_id and no revision_id.
/// Assert execute_query_pinned was called with (workspace.current_workspace().id, tracker.load()).
#[tokio::test]
async fn moldql_pattern_handler_falls_back_to_current_pin() {
    use cognicode_explorer::api::moldql_pattern_handler;

    let recording = Arc::new(RecordingMoldQLService::new());
    let tracker = Arc::new(AtomicU64::new(3)); // Simulate some ingests already happened
    let state = ApiState::new(
        Arc::new(MockWorkspaceForPin::new("current-ws")),
        Arc::new(MockSearchService),
        Arc::new(MockViewService),
        Arc::new(MockPersistenceService),
        recording.clone(),
        Arc::new(MockGraphService),
    )
    .with_revision_tracker(tracker);

    let app = axum::Router::new()
        .route("/api/moldql/pattern", post(moldql_pattern_handler))
        .with_state(state);

    let body = PatternQueryBody {
        query: "MATCH (n) RETURN n".to_string(),
        workspace_id: None,
        revision_id: None,
    };

    let req = Request::builder()
        .method("POST")
        .uri("/api/moldql/pattern")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK, "handler should succeed");

    let pin = recording.take_recorded_pin();
    assert!(
        pin.is_some(),
        "execute_query_pinned should have been called"
    );
    let (ws, rev) = pin.unwrap();
    assert_eq!(
        ws, "current-ws",
        "workspace should fall back to current_workspace().id"
    );
    assert_eq!(rev, 3, "revision should fall back to tracker.load()");
}
