//! CP1.0 WU4 — `GET /control-plane/workspaces/:workspace_id/architecture`.
//!
//! Fail-closed HTTP contract tests (C1–C5 semantics carried from
//! `ControlQueryService`). Mocks mirror `e28_3_runtime_wiring.rs`.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(deprecated, unused_imports, dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use tower::ServiceExt;

use axum::Router as AxumRouter;
use cognicode_core::application::architecture::{
    ArchitectureAdmissionService, ArchitectureRegistry, ControlQueryService,
    SystemArchitectureClock,
};
use cognicode_core::domain::architecture::{
    Admitter, AdmitterRole, ArchitectureConstraintId, ArchitectureConstraintKind,
    ConstraintCandidate, LayerDependencyRule, LayerId,
};
use cognicode_explorer::api::{ApiState, router};
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
// CP1.0 WU4 — control-plane architecture endpoint
// ============================================================================

fn layer_candidate(id: &str) -> ConstraintCandidate {
    ConstraintCandidate {
        id: ArchitectureConstraintId::new(id).unwrap(),
        kind: ArchitectureConstraintKind::LayerDependency(LayerDependencyRule {
            from_layer: LayerId::Domain,
            forbidden_targets: vec![LayerId::Infrastructure],
            rationale: "test".into(),
        }),
        adr_ref: Some("ADR-007".into()),
        proposed_by: "human:test".into(),
    }
}

fn admitted_registry(id: &str) -> ArchitectureRegistry {
    let mut registry = ArchitectureRegistry::new();
    let admitter = Admitter {
        id: "human:test".into(),
        role: AdmitterRole::HumanPromoter,
    };
    let out = registry
        .admission
        .admit(layer_candidate(id), &admitter, &SystemArchitectureClock);
    assert!(out.result.is_ok(), "fixture admission failed");
    registry
}

async fn get(state: ApiState, path: &str) -> (axum::http::StatusCode, serde_json::Value) {
    let app: AxumRouter = router(state);
    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .uri(path)
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({}));
    (status, json)
}

/// Small, bounded source root (never scan the whole temp dir).
fn empty_source_root(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("cp1-wu4-empty-{}-{}", tag, std::process::id()));
    std::fs::create_dir_all(dir.join("src")).unwrap();
    dir
}

fn temp_source_root(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("cp1-wu4-{}-{}", tag, std::process::id()));
    let src = dir.join("src/domain");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        src.join("service.rs"),
        "use crate::infrastructure::db::Pool;\npub struct S;\n",
    )
    .unwrap();
    dir
}

fn base_ws(ws: &str) -> ApiState {
    ApiState::new(
        Arc::new(MockWorkspaceForPin::new(ws)),
        Arc::new(MockSearchService),
        Arc::new(MockViewService),
        Arc::new(MockPersistenceService),
        Arc::new(RecordingMoldQLService::new()),
        Arc::new(MockGraphService),
    )
}

trait CloneState {
    fn clone_state(&self) -> ApiState;
}

impl CloneState for ApiState {
    fn clone_state(&self) -> ApiState {
        ApiState {
            workspace: self.workspace.clone(),
            search: self.search.clone(),
            view: self.view.clone(),
            persistence: self.persistence.clone(),
            moldql: self.moldql.clone(),
            graph: self.graph.clone(),
            investigation: self.investigation.clone(),
            #[cfg(feature = "multimodal")]
            graph_repo: self.graph_repo.clone(),
            snapshot: self.snapshot.clone(),
            revision_tracker: self.revision_tracker.clone(),
            control_query: self.control_query.clone(),
            control_source_root: self.control_source_root.clone(),
            analytics_registry: self.analytics_registry.clone(),
            analytics_lineage_store: self.analytics_lineage_store.clone(),
        }
    }
}

/// C1 — no ControlQueryService wired → fail-closed incomplete, never clean.
#[tokio::test]
async fn c1_not_wired_reads_incomplete_never_clean() {
    let (status, body) = get(
        base_ws("ws-1"),
        "/control-plane/workspaces/ws-1/architecture",
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(body["status"], "incomplete");
    assert_eq!(body["reason"], "control_query_service_not_wired");
    assert_eq!(body["violations"], serde_json::json!([]));
}

/// C2 — wired but empty admitted set → incomplete, NOT a clean verdict.
#[tokio::test]
async fn c2_empty_admission_reads_incomplete() {
    let state = base_ws("ws-1").with_control_query(
        Some(Arc::new(ControlQueryService::new(
            ArchitectureRegistry::new(),
        ))),
        empty_source_root("c2"),
    );
    let (status, body) = get(state, "/control-plane/workspaces/ws-1/architecture").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(body["status"], "incomplete");
    assert_eq!(body["constraints"], serde_json::json!([]));
}

/// C3 — real evaluation over the workspace source root → evaluated.
#[tokio::test]
async fn c3_real_evaluation_is_evaluated() {
    let dir = temp_source_root("c3");
    let state = base_ws("ws-1").with_control_query(
        Some(Arc::new(ControlQueryService::new(admitted_registry(
            "arch.no_infra_in_domain",
        )))),
        dir.clone(),
    );
    let (status, body) = get(state, "/control-plane/workspaces/ws-1/architecture").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    assert_eq!(body["status"], "evaluated", "body: {body}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// C4 — the violation is projected with references and coordinates,
/// never a copy of canonical truth.
#[tokio::test]
async fn c4_violation_projected_with_reference_not_truth() {
    let dir = temp_source_root("c4");
    let state = base_ws("ws-1").with_control_query(
        Some(Arc::new(ControlQueryService::new(admitted_registry(
            "arch.no_infra_in_domain",
        )))),
        dir.clone(),
    );
    let (_, body) = get(state, "/control-plane/workspaces/ws-1/architecture").await;
    let violations = body["violations"].as_array().unwrap();
    assert_eq!(violations.len(), 1, "body: {body}");
    let v = &violations[0];
    assert_eq!(v["constraint_id"], "arch.no_infra_in_domain");
    // parser normalizes leading crate:: away in dependency_path
    assert_eq!(v["dependency_path"], "infrastructure::db::Pool");
    assert_eq!(v["line"], 1);
    let constraints = body["constraints"].as_array().unwrap();
    assert_eq!(constraints[0]["id"], "arch.no_infra_in_domain");
    assert_eq!(constraints[0]["kind"], "layer_dependency");
    assert_eq!(constraints[0]["adr_ref"], "ADR-007");
    let _ = std::fs::remove_dir_all(&dir);
}

/// C5 — repeated queries: the endpoint never grows the admission set and
/// never persists anything; the query path is stateless and read-only.
#[tokio::test]
async fn c5_endpoint_path_is_read_only() {
    let registry = admitted_registry("arch.readonly");
    let admitted_before = registry.admission.admitted().len();
    let state = base_ws("ws").with_control_query(
        Some(Arc::new(ControlQueryService::new(registry))),
        empty_source_root("c5"),
    );
    for _ in 0..3 {
        let (_, body) = get(
            state.clone_state(),
            "/control-plane/workspaces/ws/architecture",
        )
        .await;
        assert_eq!(body["status"], "evaluated");
        assert_eq!(
            body["constraints"].as_array().unwrap().len(),
            admitted_before
        );
    }
}

/// T12 regression guard — `workspace_id` may contain URL-encoded path
/// traversal characters (`../../etc/passwd`). The handler must NOT
/// resolve it as a filesystem path; it must echo the URL-decoded input
/// verbatim in the `workspace_ref` field and otherwise behave like any
/// other workspace (fail-closed if not wired, evaluated if wired).
///
/// Currently safe because `ControlQueryService` does not use the
/// workspace_ref as a path. If a future consumer introduces filesystem
/// resolution, this test fails loudly (workspace_ref starts with "../").
#[tokio::test]
async fn c6_path_traversal_in_workspace_id_is_echoed_not_resolved() {
    let registry = admitted_registry("arch.path_safety");
    let state = base_ws("ws").with_control_query(
        Some(Arc::new(ControlQueryService::new(registry))),
        empty_source_root("c6"),
    );
    // The path is URL-encoded by the test client; axum decodes it to
    // "../../etc/passwd" before our handler sees it.
    let (_, body) = get(
        state.clone_state(),
        "/control-plane/workspaces/..%2F..%2Fetc%2Fpasswd/architecture",
    )
    .await;
    let workspace_ref = body["workspace_ref"]
        .as_str()
        .expect("workspace_ref must be a string");
    assert!(
        workspace_ref.starts_with("../"),
        "T12 watch: workspace_ref echoes the URL-decoded input verbatim; \
         it must NOT be resolved as a filesystem path. Got: {workspace_ref:?}"
    );
    // Status is still evaluated (the handler does not check the workspace_ref).
    assert_eq!(
        body["status"], "evaluated",
        "T12 handler must not change status based on workspace_ref content"
    );
}

/// E2.W1 — CP1 first consumer: the **production** wiring helper
/// `wire_canonical_control_query` must produce a `ControlQueryService`
/// whose endpoint response is `evaluated` with the three canonical
/// CogniCode architecture constraints, even when the source tree is
/// the canonical CogniCode workspace itself (the self-host gate).
///
/// This pins the production code path (not a hand-built fixture) and
/// proves that `wire_canonical_control_query` is a drop-in for the
/// mock registries used in C1–C6.
#[tokio::test]
async fn c7_real_wiring_uses_canonical_constraints() {
    use cognicode_core::application::architecture::control_query::wire_canonical_control_query;

    // The real helper. Constructed via the same path a production
    // binary would take; the canonical constraints are admitted by the
    // canonical promoted admitter.
    let cq = wire_canonical_control_query();

    // Point the source root at the empty tempdir. With zero source
    // files the evaluator examines zero statements but still returns
    // `evaluated` (no parse errors) and reports the three admitted
    // constraints — the load-bearing property: the endpoint never
    // returns `incomplete` when the wiring is real.
    let dir = empty_source_root("c7");
    let state = base_ws("ws-canonical").with_control_query(Some(Arc::new(cq)), dir.clone());

    let (_, body) = get(
        state,
        "/control-plane/workspaces/ws-canonical/architecture",
    )
    .await;

    assert_eq!(
        body["status"], "evaluated",
        "wire_canonical_control_query must produce an evaluated endpoint; body: {body}"
    );

    let constraints = body["constraints"].as_array().unwrap();
    let ids: Vec<&str> = constraints
        .iter()
        .map(|c| c["id"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(
        ids,
        vec![
            "architecture.domain_no_infrastructure",
            "architecture.domain_no_application",
            "architecture.evidence_kernel_no_presentation",
        ],
        "endpoint must surface the three canonical constraints admitted by wire_canonical_control_query"
    );

    // The synthetic-drift source root is the inverse case: it
    // produces exactly one violation against the first canonical
    // constraint. Together with C3/C4 this proves the production
    // wiring is functionally equivalent to the hand-built fixtures.
    let dir2 = temp_source_root("c7");
    let cq2 = wire_canonical_control_query();
    let state2 = base_ws("ws-canonical-2").with_control_query(Some(Arc::new(cq2)), dir2.clone());
    let (_, body2) = get(
        state2,
        "/control-plane/workspaces/ws-canonical-2/architecture",
    )
    .await;
    let violations = body2["violations"].as_array().unwrap();
    assert_eq!(
        violations.len(),
        1,
        "canonical wiring must detect the synthetic domain->infra drift; body: {body2}"
    );
    assert_eq!(
        violations[0]["constraint_id"], "architecture.domain_no_infrastructure",
        "violation must be attributed to the canonical layer-dependency rule"
    );

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&dir2);
}

// ============================================================================
// E2.W2 — `control_plane_router` boots and serves real HTTP requests
// ============================================================================
//
// The previous CP1 tests use `axum::Router::oneshot`, which exercises
// the router without binding a port. E2.W2 promotes that to a real
// TCP listener bound to port 0 (OS-assigned), sends an HTTP request,
// and asserts on the response status and body. This is the closest
// we can get to running `cognicode-control-plane` from inside a test
// without spawning a child process.

use cognicode_explorer::api::{ControlPlaneState, control_plane_router};

/// Bind the control-plane router to a random port and run a single
/// request against it. Returns the HTTP status and parsed JSON body.
async fn control_plane_request(
    state: ControlPlaneState,
    path: &str,
) -> (StatusCode, serde_json::Value) {
    let app = control_plane_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local_addr");
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("axum serve");
    });

    let url = format!("http://{addr}{path}");
    let resp = reqwest::get(&url).await.expect("http get");
    let status = resp.status();
    let body = resp
        .json::<serde_json::Value>()
        .await
        .expect("parse json body");

    server.abort();
    let _ = server.await;
    (status, body)
}

#[tokio::test]
async fn e2_w2_control_plane_router_serves_real_http_request() {
    // Empty source root → no source files, but the registry still
    // carries 3 admitted constraints; the evaluator examines 0
    // statements and reports `evaluated` (no parse errors).
    let dir = empty_source_root("e2w2-real");
    let state = ControlPlaneState::canonical(dir.clone());

    let (status, body) = control_plane_request(
        state,
        "/control-plane/workspaces/e2-w2-workspace/architecture",
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "control_plane_router must respond 200 over real TCP; body: {body}"
    );
    assert_eq!(body["status"], "evaluated");
    let constraints = body["constraints"].as_array().unwrap();
    assert_eq!(
        constraints.len(),
        3,
        "all 3 canonical constraints must be in the response"
    );
    assert!(body["violations"].as_array().unwrap().is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn e2_w2_control_plane_router_detects_synthetic_drift() {
    // Source root with a `domain::*` module that imports
    // `infrastructure::*` → one violation against
    // `architecture.domain_no_infrastructure`. Same synthetic drift
    // as `temp_source_root` (used by C4), but exercised over a real
    // TCP listener rather than `oneshot`.
    let dir = temp_source_root("e2w2-drift");
    let state = ControlPlaneState::canonical(dir.clone());

    let (status, body) = control_plane_request(
        state,
        "/control-plane/workspaces/drift-workspace/architecture",
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "evaluated");
    let violations = body["violations"].as_array().unwrap();
    assert_eq!(
        violations.len(),
        1,
        "real HTTP listener must detect the synthetic domain→infra drift; body: {body}"
    );
    assert_eq!(
        violations[0]["constraint_id"], "architecture.domain_no_infrastructure",
        "violation must be attributed to the canonical layer-dependency rule"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn e2_w2_control_plane_router_404_for_non_cp_routes() {
    // `/api/...` and `/control-plane/probe` are not mounted on the
    // CP1-only router. A real HTTP request must return 404 with an
    // empty body — never a panic, never a 500.
    let dir = empty_source_root("e2w2-404");
    let state = ControlPlaneState::canonical(dir.clone());

    let app = control_plane_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("axum serve");
    });

    for path in [
        "/api/workspaces/foo/landing",
        "/api/workspaces/foo/architecture",
        "/control-plane/probe",
        "/totally/unknown",
    ] {
        let url = format!("http://{addr}{path}");
        let resp = reqwest::get(&url).await.expect("http get");
        assert_eq!(
            resp.status(),
            StatusCode::NOT_FOUND,
            "non-CP route {path} must return 404; got {}",
            resp.status()
        );
    }

    server.abort();
    let _ = server.await;
    let _ = std::fs::remove_dir_all(&dir);
}
