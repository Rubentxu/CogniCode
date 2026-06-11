//! Integration tests for the `GET /api/graph/:id/subgraph` endpoint
//! and the supporting `style_class_for` / `edge_style_class_for`
//! helpers in `api.rs`.
//!
//! These tests live in a separate file from `api.rs` so the production
//! module stays focused on wiring and the test surface is concentrated
//! in one place — the same pattern used by `tests/integration.rs` for
//! the higher-level flows.
//!
//! TDD contract: every block here is RED before any matching GREEN
//! implementation lands. Helpers and DTOs come from `super::*`
//! (`api.rs`) and `crate::dto`/`crate::error`.
//!
//! Mirrors the spec at
//! `openspec/changes/visualization-stack/specs/graph-data-endpoint/`.

use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use cognicode_core::domain::aggregates::SymbolId;
use cognicode_core::domain::value_objects::SymbolKind;
use tower::ServiceExt;

use crate::api::router;
use crate::error::ExplorerError;
use crate::ports::source_reader::SourceReader;
use crate::ports::symbol_repository::{
    GraphStats, RelationTarget, ResolvedSymbol, SymbolRepository,
};
use crate::service::ExplorerService;

// ============================================================================
// style_class_for (node)
// ============================================================================

#[test]
fn style_class_for_function() {
    assert_eq!(crate::api::style_class_for("function"), "function");
}

#[test]
fn style_class_for_method_is_function() {
    assert_eq!(crate::api::style_class_for("method"), "function");
}

#[test]
fn style_class_for_module() {
    assert_eq!(crate::api::style_class_for("module"), "module");
}

#[test]
fn style_class_for_crate_is_module() {
    assert_eq!(crate::api::style_class_for("crate"), "module");
}

#[test]
fn style_class_for_trait_is_module() {
    assert_eq!(crate::api::style_class_for("trait"), "module");
}

#[test]
fn style_class_for_external() {
    assert_eq!(crate::api::style_class_for("external"), "external");
}

#[test]
fn style_class_for_unknown_falls_back_to_function() {
    // Unknown kinds must not panic; they map to the default.
    assert_eq!(crate::api::style_class_for("wat"), "function");
    assert_eq!(crate::api::style_class_for(""), "function");
}

// ============================================================================
// edge_style_class_for (edge)
// ============================================================================

#[test]
fn edge_style_class_for_calls() {
    assert_eq!(
        crate::api::edge_style_class_for("calls"),
        "edge.calls"
    );
}

#[test]
fn edge_style_class_for_implements() {
    assert_eq!(
        crate::api::edge_style_class_for("implements"),
        "edge.implements"
    );
}

#[test]
fn edge_style_class_for_uses() {
    assert_eq!(crate::api::edge_style_class_for("uses"), "edge.uses");
}

#[test]
fn edge_style_class_for_imports_is_uses() {
    assert_eq!(
        crate::api::edge_style_class_for("imports"),
        "edge.uses"
    );
}

#[test]
fn edge_style_class_for_unknown_falls_back_to_calls() {
    assert_eq!(
        crate::api::edge_style_class_for("wires"),
        "edge.calls"
    );
}

// ============================================================================
// T16 — multimodal `style_class_for` / `edge_style_class_for` extensions
// ============================================================================
//
// RED gate: these tests assert the wire-level style class for the four
// new multimodal node kinds (Decision, Doc, Issue, Evidence) and the
// four new multimodal edge kinds (cites, justifies, resolves,
// corroborated). The buckets are a strict mirror of the Zod enum in
// `apps/explorer-ui/src/api/schemas.ts` and the cytoscape stylesheet
// blocks in `apps/explorer-ui/src/components/InteractiveGraph/stylesheet.ts`.

/// `decision` (an ADR / RFC node) maps to `"node-decision"`.
#[test]
fn style_class_decision() {
    assert_eq!(crate::api::style_class_for("decision"), "node-decision");
}

/// `doc` (a Markdown documentation node) maps to `"node-doc"`.
#[test]
fn style_class_doc() {
    assert_eq!(crate::api::style_class_for("doc"), "node-doc");
}

/// `issue` (a tracker issue) maps to `"node-issue"`.
#[test]
fn style_class_issue() {
    assert_eq!(crate::api::style_class_for("issue"), "node-issue");
}

/// `evidence` (a benchmark / fuzzer result) maps to `"node-evidence"`.
#[test]
fn style_class_evidence() {
    assert_eq!(crate::api::style_class_for("evidence"), "node-evidence");
}

// ============================================================================
// T-Phase-1 — C4 architecture `style_class_for` / `edge_style_class_for`
// extensions
// ============================================================================
//
// RED gate: these tests assert the wire-level style class for the three
// new C4-model node kinds (Component, Container, System) and the three
// new C4-model edge kinds (part_of, deployed_as, in_system). The
// buckets mirror the cytoscape stylesheet blocks in
// `apps/explorer-ui/src/components/InteractiveGraph/stylesheet.ts`.

/// `component` (a C4-model component — grouping of related symbols)
/// maps to `"node-component"`.
#[test]
fn style_class_component() {
    assert_eq!(crate::api::style_class_for("component"), "node-component");
}

/// `container` (a C4-model container — deployable unit) maps to
/// `"node-container"`.
#[test]
fn style_class_container() {
    assert_eq!(crate::api::style_class_for("container"), "node-container");
}

/// `system` (a C4-model system — boundary of related containers)
/// maps to `"node-system"`.
#[test]
fn style_class_system() {
    assert_eq!(crate::api::style_class_for("system"), "node-system");
}

/// `cites` (a doc → code reference) maps to `"edge-cites"`.
#[test]
fn edge_style_cites() {
    assert_eq!(
        crate::api::edge_style_class_for("cites"),
        "edge-cites"
    );
}

/// `justifies` (an ADR → architectural choice) maps to `"edge-justifies"`.
#[test]
fn edge_style_justifies() {
    assert_eq!(
        crate::api::edge_style_class_for("justifies"),
        "edge-justifies"
    );
}

/// `resolves` (a PR → issue) maps to `"edge-resolves"`.
#[test]
fn edge_style_resolves() {
    assert_eq!(
        crate::api::edge_style_class_for("resolves"),
        "edge-resolves"
    );
}

/// `corroborated_by` (a test result → design claim) maps to `"edge-corroborated"`.
#[test]
fn edge_style_corroborated() {
    assert_eq!(
        crate::api::edge_style_class_for("corroborated_by"),
        "edge-corroborated"
    );
}

/// `part_of` (a component → container) maps to `"edge-part-of"`.
#[test]
fn edge_style_part_of() {
    assert_eq!(
        crate::api::edge_style_class_for("part_of"),
        "edge-part-of"
    );
}

/// `deployed_as` (a container → service) maps to `"edge-deployed-as"`.
#[test]
fn edge_style_deployed_as() {
    assert_eq!(
        crate::api::edge_style_class_for("deployed_as"),
        "edge-deployed-as"
    );
}

/// `in_system` (a container → system) maps to `"edge-in-system"`.
#[test]
fn edge_style_in_system() {
    assert_eq!(
        crate::api::edge_style_class_for("in_system"),
        "edge-in-system"
    );
}

// ============================================================================
// T16 regression — the 3+3 pre-existing buckets must keep their classes.
// ============================================================================

/// `function` / `method` / `fn` keep the `"function"` bucket after the
/// multimodal extension.
#[test]
fn style_class_for_function_regression() {
    assert_eq!(crate::api::style_class_for("function"), "function");
    assert_eq!(crate::api::style_class_for("method"), "function");
    assert_eq!(crate::api::style_class_for("fn"), "function");
}

/// `module` / `crate` / `trait` keep the `"module"` bucket.
#[test]
fn style_class_for_module_regression() {
    assert_eq!(crate::api::style_class_for("module"), "module");
    assert_eq!(crate::api::style_class_for("crate"), "module");
    assert_eq!(crate::api::style_class_for("trait"), "module");
}

/// `external` keeps the `"external"` bucket.
#[test]
fn style_class_for_external_regression() {
    assert_eq!(crate::api::style_class_for("external"), "external");
}

/// `edge.calls` / `edge.implements` / `edge.uses` keep their buckets.
#[test]
fn edge_style_class_calls_regression() {
    assert_eq!(
        crate::api::edge_style_class_for("calls"),
        "edge.calls"
    );
    assert_eq!(
        crate::api::edge_style_class_for("implements"),
        "edge.implements"
    );
    assert_eq!(crate::api::edge_style_class_for("uses"), "edge.uses");
}

// ============================================================================
// query param validation
// ============================================================================

#[test]
fn query_depth_zero_is_invalid() {
    let v = crate::api::SubgraphQuery {
        depth: Some(0),
        direction: None,
        max_nodes: None,
    };
    let err = v.validated().expect_err("should reject depth=0");
    assert!(matches!(err, ExplorerError::InvalidQuery(_)));
}

#[test]
fn query_depth_eleven_is_invalid() {
    let v = crate::api::SubgraphQuery {
        depth: Some(11),
        direction: None,
        max_nodes: None,
    };
    let err = v.validated().expect_err("should reject depth=11");
    assert!(matches!(err, ExplorerError::InvalidQuery(_)));
}

#[test]
fn query_max_nodes_zero_is_invalid() {
    let v = crate::api::SubgraphQuery {
        depth: None,
        direction: None,
        max_nodes: Some(0),
    };
    let err = v.validated().expect_err("should reject max_nodes=0");
    assert!(matches!(err, ExplorerError::InvalidQuery(_)));
}

#[test]
fn query_max_nodes_above_5000_is_invalid() {
    let v = crate::api::SubgraphQuery {
        depth: None,
        direction: None,
        max_nodes: Some(5001),
    };
    let err = v.validated().expect_err("should reject max_nodes=5001");
    assert!(matches!(err, ExplorerError::InvalidQuery(_)));
}

#[test]
fn query_direction_invalid_is_rejected() {
    let v = crate::api::SubgraphQuery {
        depth: None,
        direction: Some("sideways".to_string()),
        max_nodes: None,
    };
    let err = v.validated().expect_err("should reject sideways");
    assert!(matches!(err, ExplorerError::InvalidQuery(_)));
}

#[test]
fn query_defaults_applied_when_missing() {
    let v = crate::api::SubgraphQuery {
        depth: None,
        direction: None,
        max_nodes: None,
    };
    let (depth, direction, max_nodes) = v.validated().expect("default values");
    assert_eq!(depth, 3);
    assert!(matches!(direction, crate::api::SubgraphDirection::Both));
    assert_eq!(max_nodes, 500);
}

#[test]
fn query_explicit_values_accepted() {
    let v = crate::api::SubgraphQuery {
        depth: Some(5),
        direction: Some("incoming".to_string()),
        max_nodes: Some(100),
    };
    let (depth, direction, max_nodes) = v.validated().expect("explicit values");
    assert_eq!(depth, 5);
    assert!(matches!(direction, crate::api::SubgraphDirection::Incoming));
    assert_eq!(max_nodes, 100);
}

// ============================================================================
// handler success path
// ============================================================================

/// Minimal in-memory repository. Holds a hand-built graph:
///   sym:foo::bar  ──calls──►  sym:foo::baz
///                          ╲
///                           ──calls──►  sym:ext::lib
struct TinyRepo;

impl SymbolRepository for TinyRepo {
    fn resolve(
        &self,
        id: &SymbolId,
    ) -> crate::error::ExplorerResult<Option<ResolvedSymbol>> {
        if id.as_str() == "sym:foo::bar" {
            return Ok(Some(ResolvedSymbol {
                id: id.clone(),
                name: "bar".to_string(),
                kind: SymbolKind::Function,
                file: "foo.rs".to_string(),
                line: 10,
                signature: Some("fn bar()".to_string()),
            }));
        }
        if id.as_str() == "sym:foo::baz" {
            return Ok(Some(ResolvedSymbol {
                id: id.clone(),
                name: "baz".to_string(),
                kind: SymbolKind::Function,
                file: "foo.rs".to_string(),
                line: 20,
                signature: Some("fn baz()".to_string()),
            }));
        }
        if id.as_str() == "sym:ext::lib" {
            return Ok(Some(ResolvedSymbol {
                id: id.clone(),
                name: "lib".to_string(),
                kind: SymbolKind::Module,
                file: "ext/lib.rs".to_string(),
                line: 1,
                signature: None,
            }));
        }
        Ok(None)
    }

    fn callers(&self, _id: &SymbolId) -> Vec<RelationTarget> {
        Vec::new()
    }

    fn callees(&self, id: &SymbolId) -> Vec<RelationTarget> {
        if id.as_str() == "sym:foo::bar" {
            return vec![RelationTarget {
                id: SymbolId::new("sym:foo::baz"),
                name: "baz".to_string(),
                kind: SymbolKind::Function,
                file: "foo.rs".to_string(),
                line: 20,
                signature: Some("fn baz()".to_string()),
            }];
        }
        if id.as_str() == "sym:foo::baz" {
            return vec![RelationTarget {
                id: SymbolId::new("sym:ext::lib"),
                name: "lib".to_string(),
                kind: SymbolKind::Module,
                file: "ext/lib.rs".to_string(),
                line: 1,
                signature: None,
            }];
        }
        Vec::new()
    }

    fn fan_in(&self, _id: &SymbolId) -> usize {
        0
    }

    fn fan_out(&self, id: &SymbolId) -> usize {
        self.callees(id).len()
    }

    fn find_symbols_by_name(
        &self,
        _name: &str,
    ) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
        Ok(Vec::new())
    }

    fn find_symbols_by_file(
        &self,
        _file: &str,
    ) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
        Ok(Vec::new())
    }

    fn module_list(&self) -> Vec<String> {
        vec!["foo.rs".to_string(), "ext/lib.rs".to_string()]
    }

    fn all_symbols(&self) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
        Ok(Vec::new())
    }

    fn graph_stats(&self) -> GraphStats {
        GraphStats {
            symbol_count: 3,
            relation_count: 2,
        }
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

fn test_app() -> axum::Router {
    let repo = Arc::new(TinyRepo);
    let reader = Arc::new(EmptyReader);
    let service = ExplorerService::new(repo, reader, "/tmp/test");
    router(service)
}

#[tokio::test]
async fn handler_returns_200_with_root_and_nodes() {
    let app = test_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/sym:foo::bar/subgraph")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["root"], "sym:foo::bar");
    assert!(json["nodes"].is_array());
    assert!(json["edges"].is_array());
    assert_eq!(json["truncated"], false);
}

#[tokio::test]
async fn handler_nodes_have_valid_style_class() {
    let app = test_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/sym:foo::bar/subgraph")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    let allowed = ["function", "module", "external"];
    for n in json["nodes"].as_array().unwrap() {
        let cls = n["style_class"].as_str().expect("string");
        assert!(allowed.contains(&cls), "unexpected style_class: {cls}");
    }
}

#[tokio::test]
async fn handler_edges_have_valid_style_class() {
    let app = test_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/sym:foo::bar/subgraph")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    let allowed = ["edge.calls", "edge.implements", "edge.uses"];
    for e in json["edges"].as_array().unwrap() {
        let cls = e["style_class"].as_str().expect("string");
        assert!(allowed.contains(&cls), "unexpected edge style_class: {cls}");
    }
}

#[tokio::test]
async fn handler_response_round_trips_via_dto() {
    let app = test_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/sym:foo::bar/subgraph")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    let parsed: crate::dto::SubgraphResponse =
        serde_json::from_slice(&body).expect("SubgraphResponse round-trip");
    assert_eq!(parsed.root, "sym:foo::bar");
    assert!(!parsed.nodes.is_empty());
    assert!(!parsed.edges.is_empty());
}

#[tokio::test]
async fn handler_edge_sources_and_targets_exist_in_nodes() {
    let app = test_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/sym:foo::bar/subgraph")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    let ids: Vec<String> = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_string())
        .collect();
    for e in json["edges"].as_array().unwrap() {
        let src = e["source"].as_str().unwrap();
        let tgt = e["target"].as_str().unwrap();
        assert!(ids.contains(&src.to_string()), "dangling source: {src}");
        assert!(ids.contains(&tgt.to_string()), "dangling target: {tgt}");
    }
}

// ============================================================================
// truncation + error paths
// ============================================================================

/// Mock repository that yields many neighbours, used to test the
/// `max_nodes` truncation branch.
struct WideRepo;

impl SymbolRepository for WideRepo {
    fn resolve(
        &self,
        id: &SymbolId,
    ) -> crate::error::ExplorerResult<Option<ResolvedSymbol>> {
        Ok(Some(ResolvedSymbol {
            id: id.clone(),
            name: id.as_str().to_string(),
            kind: SymbolKind::Function,
            file: "wide.rs".to_string(),
            line: 1,
            signature: None,
        }))
    }

    fn callers(&self, _id: &SymbolId) -> Vec<RelationTarget> {
        Vec::new()
    }

    fn callees(&self, _id: &SymbolId) -> Vec<RelationTarget> {
        (1..=600)
            .map(|i| RelationTarget {
                id: SymbolId::new(format!("sym:wide::n{i:04}")),
                name: format!("n{i:04}"),
                kind: SymbolKind::Function,
                file: "wide.rs".to_string(),
                line: i as u32,
                signature: None,
            })
            .collect()
    }

    fn fan_in(&self, _id: &SymbolId) -> usize {
        0
    }

    fn fan_out(&self, id: &SymbolId) -> usize {
        self.callees(id).len()
    }

    fn find_symbols_by_name(
        &self,
        _name: &str,
    ) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
        Ok(Vec::new())
    }

    fn find_symbols_by_file(
        &self,
        _file: &str,
    ) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
        Ok(Vec::new())
    }

    fn module_list(&self) -> Vec<String> {
        vec!["wide.rs".to_string()]
    }

    fn all_symbols(&self) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
        Ok(Vec::new())
    }

    fn graph_stats(&self) -> GraphStats {
        GraphStats {
            symbol_count: 601,
            relation_count: 600,
        }
    }
}

fn wide_app() -> axum::Router {
    let repo = Arc::new(WideRepo);
    let reader = Arc::new(EmptyReader);
    let service = ExplorerService::new(repo, reader, "/tmp/wide");
    router(service)
}

#[tokio::test]
async fn handler_truncates_when_max_nodes_exceeded() {
    let app = wide_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/sym:wide::root/subgraph?max_nodes=500")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .expect("body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["truncated"], true);
    assert_eq!(json["truncated_reason"], "node_cap");
    assert_eq!(json["nodes"].as_array().unwrap().len(), 500);
}

#[tokio::test]
async fn handler_truncated_response_has_no_dangling_edges() {
    let app = wide_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/sym:wide::root/subgraph?max_nodes=500")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    let body = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .expect("body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    let ids: std::collections::HashSet<String> = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["id"].as_str().unwrap().to_string())
        .collect();
    for e in json["edges"].as_array().unwrap() {
        let src = e["source"].as_str().unwrap().to_string();
        let tgt = e["target"].as_str().unwrap().to_string();
        assert!(ids.contains(&src), "dangling source after truncation: {src}");
        assert!(ids.contains(&tgt), "dangling target after truncation: {tgt}");
    }
}

#[tokio::test]
async fn handler_empty_id_is_invalid() {
    // axum's Path<String> won't match an empty segment, so we exercise
    // the validator via a manual "id" that the service treats as
    // missing. Use the validate_id helper directly.
    assert!(crate::api::validate_id("").is_err());
}

#[tokio::test]
async fn handler_id_too_long_is_invalid() {
    let too_long: String = "a".repeat(513);
    assert!(crate::api::validate_id(&too_long).is_err());
}

#[tokio::test]
async fn handler_id_at_limit_is_valid() {
    let at_limit: String = "a".repeat(512);
    assert!(crate::api::validate_id(&at_limit).is_ok());
}

#[tokio::test]
async fn handler_unknown_symbol_returns_404() {
    let app = test_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/sym:does::not::exist/subgraph")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    let err = json["error"].as_str().unwrap_or_default();
    assert!(
        err.contains("symbol_not_found") || err.contains("not found"),
        "expected symbol_not_found, got: {err}"
    );
    // The body must NOT contain Rust-specific text or stack traces.
    let body_str = String::from_utf8_lossy(&body);
    assert!(!body_str.contains("panicked at"));
    assert!(!body_str.contains("RUST_BACKTRACE"));
}

#[tokio::test]
async fn handler_graph_unavailable_returns_503() {
    struct UnavailableRepo;
    impl SymbolRepository for UnavailableRepo {
        fn resolve(
            &self,
            _id: &SymbolId,
        ) -> crate::error::ExplorerResult<Option<ResolvedSymbol>> {
            Err(ExplorerError::GraphNotReady)
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
        fn find_symbols_by_name(
            &self,
            _name: &str,
        ) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(Vec::new())
        }
        fn find_symbols_by_file(
            &self,
            _file: &str,
        ) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(Vec::new())
        }
        fn module_list(&self) -> Vec<String> {
            Vec::new()
        }
        fn all_symbols(&self) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(Vec::new())
        }
        fn graph_stats(&self) -> GraphStats {
            GraphStats::default()
        }
    }

    let app = {
        let repo = Arc::new(UnavailableRepo);
        let reader = Arc::new(EmptyReader);
        let service = ExplorerService::new(repo, reader, "/tmp/unavail");
        router(service)
    };
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/sym:any::id/subgraph")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    let err = json["error"].as_str().unwrap_or_default();
    assert!(
        err.contains("graph_unavailable") || err.contains("not loaded"),
        "expected graph_unavailable, got: {err}"
    );
}

// ============================================================================
// Contextual Graph — `GET /api/graph/:id/contextual` (Phase 2 of
// visualization-stack: Contextual Views).
// ============================================================================
//
// TDD contract: every block here is RED before the route + handler
// exist. After they do, the tests pass.

use crate::dto::{
    ChildrenSection, ContextualGraphResponse, ParentSection, SameLevelSection,
};

/// Mock repository that powers the contextual-handler tests. It is a
/// minimal subset of `TinyRepo` + a `find_symbols_by_file` so the
/// service can build a real `ContextualGraphResponse`.
struct ContextualRepo;

impl SymbolRepository for ContextualRepo {
    fn resolve(
        &self,
        id: &SymbolId,
    ) -> crate::error::ExplorerResult<Option<ResolvedSymbol>> {
        if id.as_str() == "sym:ctx::alpha" {
            Ok(Some(ResolvedSymbol {
                id: id.clone(),
                name: "alpha".to_string(),
                kind: SymbolKind::Function,
                file: "src/ctx.rs".to_string(),
                line: 1,
                signature: Some("fn alpha()".to_string()),
            }))
        } else if id.as_str() == "sym:ctx::beta" {
            Ok(Some(ResolvedSymbol {
                id: id.clone(),
                name: "beta".to_string(),
                kind: SymbolKind::Function,
                file: "src/ctx.rs".to_string(),
                line: 10,
                signature: Some("fn beta()".to_string()),
            }))
        } else {
            Ok(None)
        }
    }

    fn callers(&self, _id: &SymbolId) -> Vec<RelationTarget> {
        Vec::new()
    }

    fn callees(&self, id: &SymbolId) -> Vec<RelationTarget> {
        if id.as_str() == "sym:ctx::alpha" {
            vec![RelationTarget {
                id: SymbolId::new("sym:ctx::beta"),
                name: "beta".to_string(),
                kind: SymbolKind::Function,
                file: "src/ctx.rs".to_string(),
                line: 10,
                signature: Some("fn beta()".to_string()),
            }]
        } else {
            Vec::new()
        }
    }

    fn fan_in(&self, _id: &SymbolId) -> usize {
        0
    }
    fn fan_out(&self, _id: &SymbolId) -> usize {
        0
    }
    fn find_symbols_by_name(
        &self,
        _name: &str,
    ) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
        Ok(Vec::new())
    }
    fn find_symbols_by_file(
        &self,
        file: &str,
    ) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
        if file != "src/ctx.rs" {
            return Ok(Vec::new());
        }
        Ok(vec![
            ResolvedSymbol {
                id: SymbolId::new("sym:ctx::alpha"),
                name: "alpha".to_string(),
                kind: SymbolKind::Function,
                file: file.to_string(),
                line: 1,
                signature: Some("fn alpha()".to_string()),
            },
            ResolvedSymbol {
                id: SymbolId::new("sym:ctx::beta"),
                name: "beta".to_string(),
                kind: SymbolKind::Function,
                file: file.to_string(),
                line: 10,
                signature: Some("fn beta()".to_string()),
            },
        ])
    }
    fn module_list(&self) -> Vec<String> {
        vec!["src/ctx.rs".to_string()]
    }
    fn all_symbols(&self) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
        Ok(Vec::new())
    }
    fn graph_stats(&self) -> GraphStats {
        GraphStats {
            symbol_count: 2,
            relation_count: 1,
        }
    }
}

fn contextual_app() -> axum::Router {
    let repo = Arc::new(ContextualRepo);
    let reader = Arc::new(EmptyReader);
    let service = ExplorerService::new(repo, reader, "/tmp/ctx");
    router(service)
}

#[tokio::test]
async fn contextual_handler_returns_200_with_valid_payload() {
    let app = contextual_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/sym:ctx::alpha/contextual?level=file&depth=1&max_nodes=200")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    // The body must round-trip into a typed `ContextualGraphResponse`.
    let parsed: ContextualGraphResponse =
        serde_json::from_slice(&body).expect("ContextualGraphResponse round-trip");
    assert_eq!(parsed.focus_node.id, "sym:ctx::alpha");
    assert_eq!(parsed.level, "file");
    // `parent` is present (alpha lives in src/ctx.rs).
    let parent = parsed.parent.as_ref().expect("parent present");
    assert_eq!(parent.node.id, "file:src/ctx.rs");
    // `children` contains beta (the other symbol in the file).
    let children = parsed.children.as_ref().expect("children present");
    let child_ids: Vec<&str> = children.nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(child_ids.contains(&"sym:ctx::beta"));
    // `sameLevel` is non-null and includes beta.
    let same_ids: Vec<&str> = parsed.same_level.nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(same_ids.contains(&"sym:ctx::beta"));
    // JSON shape: `focusNode` camelCase, not `focus_node`.
    assert!(json["focusNode"].is_object());
    assert!(json["focus_node"].is_null());
}

#[tokio::test]
async fn contextual_handler_returns_400_for_depth_out_of_bounds() {
    let app = contextual_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/sym:ctx::alpha/contextual?level=file&depth=5&max_nodes=200")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    let err = json["error"].as_str().unwrap_or_default();
    assert!(
        err.contains("depth") || err.contains("invalid_query"),
        "expected depth validation error, got: {err}"
    );
}

#[tokio::test]
async fn contextual_handler_returns_400_for_max_nodes_out_of_bounds() {
    let app = contextual_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/sym:ctx::alpha/contextual?level=file&depth=1&max_nodes=10")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    let err = json["error"].as_str().unwrap_or_default();
    assert!(
        err.contains("max_nodes") || err.contains("invalid_query"),
        "expected max_nodes validation error, got: {err}"
    );
}

#[tokio::test]
async fn contextual_handler_returns_404_for_unknown_symbol() {
    let app = contextual_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/sym:does::not::exist/contextual")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    let err = json["error"].as_str().unwrap_or_default();
    assert!(
        err.contains("symbol_not_found") || err.contains("not found"),
        "expected symbol_not_found, got: {err}"
    );
}

#[tokio::test]
async fn contextual_handler_passes_query_params_to_service() {
    // Verifies the handler forwards the parsed query params to the
    // service method. We set `max_nodes=50` (the floor) — the service
    // accepts it; if the handler passed something else, the test
    // would still pass since the repo has only 2 symbols, so we
    // additionally assert the response shape carries the requested
    // `level` value.
    let app = contextual_app();
    let req = Request::builder()
        .method("GET")
        .uri("/api/graph/sym:ctx::alpha/contextual?level=file&depth=2&max_nodes=50")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(json["level"], "file");
}

// Suppress unused-import warnings when other tests in the file move
// around.
#[allow(unused_imports)]
use crate::dto::GraphNode as _GraphNode;
#[allow(dead_code)]
fn _force_sections_in_scope() -> (
    Option<ParentSection>,
    Option<ChildrenSection>,
    SameLevelSection,
) {
    (
        None,
        None,
        SameLevelSection {
            nodes: Vec::new(),
            edges: Vec::new(),
        },
    )
}
