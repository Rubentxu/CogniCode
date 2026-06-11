//! Integration tests for `GET /api/graph/:id/rationale`.
//!
//! Uses an in-memory `GraphRepository` with multimodal nodes + edges.
//! Tests cover query validation, handler success, truncation, and
//! corroboration scoring.
//!
//! TDD contract: every block here is RED before any matching GREEN
//! implementation lands.
//!
//! This module is gated on `#[cfg(feature = "multimodal")]` in
//! `lib.rs`, so all items here can assume multimodal is active.

use std::collections::HashMap;
use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use cognicode_core::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};
use cognicode_core::domain::value_objects::edge_kind::EdgeKind;
use cognicode_core::domain::value_objects::node_kind::NodeKind;
use cognicode_core::domain::value_objects::dependency_type::DependencyType;
use cognicode_core::domain::value_objects::provenance::Provenance;
use tower::ServiceExt;

use crate::adapters::InMemoryGraphRepository;
use crate::api::{router_with_state, ApiState};
use crate::dto::SubgraphResponse;
use crate::ports::graph_repository::GraphRepository;
use crate::service::ExplorerService;
use crate::ports::source_reader::SourceReader;
use crate::ports::symbol_repository::{GraphStats, RelationTarget, ResolvedSymbol, SymbolRepository};
use cognicode_core::domain::aggregates::SymbolId;
use cognicode_core::domain::value_objects::SymbolKind;

// ---- helper fixtures ----

/// Simple multimodal graph:
///   A ──Justifies──► D ──Cites──► X
///   D ──CorroboratedBy──► Y
///   Z ──Justifies──► D
fn rationale_fixture() -> (Vec<GraphNode>, Vec<GraphEdge>) {
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
            id: NodeId::new("D"),
            kind: NodeKind::Decision,
            label: "Decision D".to_string(),
            source_path: None,
            properties: HashMap::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
        GraphNode {
            id: NodeId::new("X"),
            kind: NodeKind::Doc,
            label: "Doc X".to_string(),
            source_path: None,
            properties: HashMap::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
        GraphNode {
            id: NodeId::new("Y"),
            kind: NodeKind::Evidence,
            label: "Evidence Y".to_string(),
            source_path: None,
            properties: HashMap::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
        GraphNode {
            id: NodeId::new("Z"),
            kind: NodeKind::Decision,
            label: "Decision Z".to_string(),
            source_path: None,
            properties: HashMap::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
    ];

    let edges = vec![
        GraphEdge {
            source: NodeId::new("A"),
            target: NodeId::new("D"),
            kind: EdgeKind::Justifies,
            provenance: Provenance::Manual,
            confidence: 0.9,
            metadata: HashMap::new(),
        },
        GraphEdge {
            source: NodeId::new("D"),
            target: NodeId::new("X"),
            kind: EdgeKind::Cites,
            provenance: Provenance::Extracted,
            confidence: 0.8,
            metadata: HashMap::new(),
        },
        GraphEdge {
            source: NodeId::new("D"),
            target: NodeId::new("Y"),
            kind: EdgeKind::CorroboratedBy,
            provenance: Provenance::Tested,
            confidence: 0.7,
            metadata: HashMap::new(),
        },
        GraphEdge {
            source: NodeId::new("Z"),
            target: NodeId::new("D"),
            kind: EdgeKind::Justifies,
            provenance: Provenance::Inferred,
            confidence: 0.5,
            metadata: HashMap::new(),
        },
    ];

    (nodes, edges)
}

struct EmptySymbolRepo;

impl SymbolRepository for EmptySymbolRepo {
    fn resolve(&self, _id: &SymbolId) -> crate::error::ExplorerResult<Option<ResolvedSymbol>> {
        Ok(None)
    }
    fn callers(&self, _id: &SymbolId) -> Vec<RelationTarget> { Vec::new() }
    fn callees(&self, _id: &SymbolId) -> Vec<RelationTarget> { Vec::new() }
    fn fan_in(&self, _id: &SymbolId) -> usize { 0 }
    fn fan_out(&self, _id: &SymbolId) -> usize { 0 }
    fn find_symbols_by_name(&self, _name: &str) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> { Ok(Vec::new()) }
    fn find_symbols_by_file(&self, _file: &str) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> { Ok(Vec::new()) }
    fn module_list(&self) -> Vec<String> { Vec::new() }
    fn all_symbols(&self) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> { Ok(Vec::new()) }
    fn graph_stats(&self) -> GraphStats { GraphStats::default() }
}

struct EmptyReader;
impl SourceReader for EmptyReader {
    fn read_source(&self, _file: &str) -> crate::error::ExplorerResult<String> { Ok(String::new()) }
    fn read_lines(&self, _file: &str, _start: u32, _end: u32) -> crate::error::ExplorerResult<Vec<(u32, String)>> { Ok(Vec::new()) }
}

fn rationale_app() -> axum::Router {
    let (nodes, edges) = rationale_fixture();
    let graph_repo: Arc<dyn GraphRepository> = Arc::new(InMemoryGraphRepository::new(nodes, edges));
    let service = ExplorerService::new(Arc::new(EmptySymbolRepo), Arc::new(EmptyReader), "/tmp/rationale");
    let state = ApiState::new(service).with_graph_repo(graph_repo);
    router_with_state(state)
}

// ========================================================================
// 1.15 — query param validation
// ========================================================================

#[tokio::test]
async fn query_max_depth_zero_is_invalid() {
    let app = rationale_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/A/rationale?max_depth=0")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn query_max_depth_six_is_invalid() {
    let app = rationale_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/A/rationale?max_depth=6")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn query_max_nodes_zero_is_invalid() {
    let app = rationale_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/A/rationale?max_nodes=0")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn query_max_nodes_above_200_is_invalid() {
    let app = rationale_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/A/rationale?max_nodes=201")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

// ========================================================================
// 1.17 — handler success path
// ========================================================================

#[tokio::test]
async fn handler_returns_200_with_valid_payload() {
    let app = rationale_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/A/rationale")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    let parsed: SubgraphResponse = serde_json::from_slice(&body).expect("SubgraphResponse");
    assert_eq!(parsed.root, "A");
    assert!(!parsed.nodes.is_empty());
    assert!(!parsed.edges.is_empty());
    assert!(!parsed.corroboration_scores.is_empty());
}

#[tokio::test]
async fn handler_returns_content_type_json() {
    let app = rationale_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/A/rationale")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response.headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(content_type.contains("application/json"));
}

#[tokio::test]
async fn handler_corroboration_scores_present() {
    let app = rationale_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/A/rationale")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    let parsed: SubgraphResponse = serde_json::from_slice(&body).expect("SubgraphResponse");
    // A->D (Manual 0.9) → 1.0 * 0.9 = 0.9
    let key = format!("{}->{}", "A", "D");
    let score = parsed.corroboration_scores.get(&key);
    assert!(score.is_some(), "expected score for {key}");
    assert!((*score.unwrap() - 0.9).abs() < 0.01);
}

#[tokio::test]
async fn handler_empty_id_is_invalid() {
    assert!(crate::api::validate_id("").is_err());
}

#[tokio::test]
async fn handler_long_id_is_invalid() {
    let too_long: String = "a".repeat(513);
    assert!(crate::api::validate_id(&too_long).is_err());
}

// ========================================================================
// 1.19 — DTO serde backward compat + new field
// ========================================================================

#[test]
fn subgraph_response_backward_compat() {
    // JSON without corroboration_scores should still parse.
    let json = r#"{"root":"A","nodes":[],"edges":[],"truncated":false}"#;
    let parsed: SubgraphResponse = serde_json::from_str(json).expect("backward compat");
    assert!(parsed.corroboration_scores.is_empty());
}

#[test]
fn subgraph_response_with_scores_round_trips() {
    let mut scores = HashMap::new();
    scores.insert("A->D".to_string(), 0.9);
    scores.insert("D->X".to_string(), 0.72);
    let resp = SubgraphResponse {
        root: "A".to_string(),
        nodes: Vec::new(),
        edges: Vec::new(),
        truncated: false,
        truncated_reason: None,
        corroboration_scores: scores,
    };
    let json = serde_json::to_string(&resp).expect("serialize");
    let parsed: SubgraphResponse = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed.corroboration_scores.len(), 2);
    assert!((parsed.corroboration_scores["A->D"] - 0.9).abs() < 0.01);
}
