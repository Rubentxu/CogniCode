//! B4.W1 — Impact graph with evidence and provenance.
//!
//! Estado pre-B4.W1: las 6 tools de impacto emitían `weight` /
//! `confidence` pero sin discriminador de dirección ni procedencia.
//! Un grafo parcial podía leerse como cobertura completa (R-B4-W1).
//!
//! Lo que este test cierra:
//!   - `Direction` y `Provenance` están presentes en CADA arista.
//!   - Cuando los límites (`max_nodes`) se alcanzan, el resultado
//!     lleva `truncated: true` y `coverage < 1.0`.
//!   - Una arista con `Provenance::Heuristic` NO se serializa como
//!     `Observed` (AAT-B4-03).
//!   - El catálogo `product/tools.json` declara
//!     `impact_direction_floor` para los 5 tools consolidados por B4.W1.
//!   - `from` / `to` son `SymbolId` con `byte_range` (paridad B3.W1).

use cognicode_core::impact::{
    build_impact_result, Direction, ImpactEdge, ImpactLimits, ImpactPath, ImpactResult,
    Provenance,
};
use cognicode_core::interface::catalog::load_tool_catalog;
use cognicode_core::interface::symbol_identity::SymbolId;
use serde_json::Value;
use std::ops::Range;
use std::path::PathBuf;

fn sym(name: &str, file: &str, range: Range<usize>) -> SymbolId {
    SymbolId::from_lsp_location(name.to_string(), PathBuf::from(file), range)
}

fn edge(from: &str, to: &str, dir: Direction, prov: Provenance, weight: f64) -> ImpactEdge {
    ImpactEdge {
        from: sym(from, "src/lib.rs", 0..10),
        to: sym(to, "src/lib.rs", 10..20),
        direction: dir,
        provenance: prov,
        weight,
        evidence: None,
    }
}

#[test]
fn direction_field_is_present_on_every_impact_edge() {
    let edges = vec![
        edge("a", "b", Direction::CallerToCallee, Provenance::Observed, 1.0),
        edge("b", "c", Direction::CalleeToCaller, Provenance::Inferred, 0.7),
    ];
    let limits = ImpactLimits { max_depth: 10, max_nodes: 100, direction: Direction::CallerToCallee };
    let result = build_impact_result(edges, &limits);
    let json = serde_json::to_value(&result).expect("serialize result");
    let paths = json.get("paths").and_then(|v| v.as_array()).expect("paths");
    let first_path = paths.first().expect("at least one path");
    let edges_json = first_path.get("edges").and_then(|v| v.as_array()).expect("edges");
    for edge_json in edges_json.iter() {
        assert!(edge_json.get("direction").is_some());
        let dir = edge_json.get("direction").and_then(|v| v.as_str()).expect("direction string");
        assert!(matches!(dir, "caller_to_callee" | "callee_to_caller" | "bidirectional" | "undirected"));
    }
}

#[test]
fn provenance_field_is_present_on_every_impact_edge() {
    let edges = vec![
        edge("a", "b", Direction::CallerToCallee, Provenance::Observed, 1.0),
        edge("c", "d", Direction::CallerToCallee, Provenance::Heuristic, 0.5),
    ];
    let limits = ImpactLimits { max_depth: 10, max_nodes: 100, direction: Direction::CallerToCallee };
    let result = build_impact_result(edges, &limits);
    let json = serde_json::to_value(&result).expect("serialize result");
    let paths = json.get("paths").and_then(|v| v.as_array()).expect("paths");
    let edges_json = paths[0].get("edges").and_then(|v| v.as_array()).expect("edges");
    for edge_json in edges_json {
        let prov = edge_json.get("provenance").and_then(|v| v.as_str()).unwrap_or("");
        assert!(matches!(prov, "observed" | "inferred" | "heuristic"),
            "provenance must be observed/inferred/heuristic, got '{prov}'");
    }
}

#[test]
fn partial_graph_does_not_claim_no_path() {
    let limits = ImpactLimits { max_depth: 10, max_nodes: 3, direction: Direction::CallerToCallee };
    let edges: Vec<ImpactEdge> = (0..5)
        .map(|i| edge("a", if i == 0 { "b" } else { "c" }, Direction::CallerToCallee, Provenance::Observed, 0.1))
        .collect();
    let result = build_impact_result(edges, &limits);
    assert!(result.truncated);
    let json = serde_json::to_value(&result).expect("serialize");
    assert_eq!(json.get("truncated").and_then(|v| v.as_bool()), Some(true));
    let coverage = result.paths.first().map(|p| p.coverage).unwrap_or(1.0);
    assert!(coverage < 1.0);
}

#[test]
fn heuristic_edge_does_not_advertise_as_observed() {
    let edges = vec![edge("a", "b", Direction::CallerToCallee, Provenance::Heuristic, 0.42)];
    let limits = ImpactLimits { max_depth: 10, max_nodes: 100, direction: Direction::CallerToCallee };
    let result = build_impact_result(edges, &limits);
    let json = serde_json::to_value(&result).expect("serialize");
    let serialized = serde_json::to_string(&json).expect("stringify");
    assert!(serialized.contains("\"provenance\":\"heuristic\""));
}

#[test]
fn catalog_declares_impact_direction_floor_for_every_impact_tool() {
    let catalog = load_tool_catalog();
    let impact_tools = ["analyze_impact", "graph_explain", "trace_path", "graph_query", "graph_query_filtered"];
    for tool_name in impact_tools {
        let descriptor = catalog.iter().find(|t| t.name == tool_name)
            .unwrap_or_else(|| panic!("missing tool {tool_name} in catalog"));
        let floor = &descriptor.impact_direction_floor;
        assert!(matches!(floor.as_str(), "observed" | "inferred" | "heuristic"),
            "tool {tool_name}: floor must be observed/inferred/heuristic; got '{floor}'");
    }
}

#[test]
fn depth_limit_honored() {
    let chain: Vec<ImpactEdge> = (0..5)
        .map(|i| edge(&format!("n{i}"), &format!("n{}", i + 1), Direction::CallerToCallee, Provenance::Observed, 1.0))
        .collect();
    let limits_short = ImpactLimits { max_depth: 2, max_nodes: 100, direction: Direction::CallerToCallee };
    let short_result = build_impact_result(chain.clone(), &limits_short);
    for path in &short_result.paths {
        assert!(path.edges.len() <= 2);
    }
    let limits_loose = ImpactLimits { max_depth: 100, max_nodes: 1000, direction: Direction::CallerToCallee };
    let loose_result = build_impact_result(chain, &limits_loose);
    let loose_total: usize = loose_result.paths.iter().map(|p| p.edges.len()).sum();
    let short_total: usize = short_result.paths.iter().map(|p| p.edges.len()).sum();
    assert!(loose_total >= short_total);
}

#[test]
fn symbol_id_carries_byte_range_not_line_col() {
    let edges = vec![edge("a", "b", Direction::CallerToCallee, Provenance::Observed, 1.0)];
    let limits = ImpactLimits { max_depth: 10, max_nodes: 100, direction: Direction::CallerToCallee };
    let result = build_impact_result(edges, &limits);
    let json = serde_json::to_value(&result).expect("serialize");
    let from = json.get("paths").and_then(|p| p.as_array())
        .and_then(|p| p.first())
        .and_then(|p| p.get("edges"))
        .and_then(|e| e.as_array())
        .and_then(|e| e.first())
        .and_then(|e| e.get("from"))
        .expect("from symbol id");
    assert!(from.get("qualified_name").is_some());
    assert!(from.get("file_path").is_some());
    assert!(from.get("byte_range").is_some());
    assert!(from.get("line").is_none() && from.get("column").is_none());
}

#[test]
fn impact_result_round_trips_through_serde() {
    let edges = vec![
        edge("a", "b", Direction::CallerToCallee, Provenance::Observed, 1.0),
        edge("b", "c", Direction::CalleeToCaller, Provenance::Inferred, 0.5),
        edge("c", "d", Direction::Bidirectional, Provenance::Heuristic, 0.3),
    ];
    let limits = ImpactLimits { max_depth: 10, max_nodes: 100, direction: Direction::CallerToCallee };
    let result = build_impact_result(edges, &limits);
    let json = serde_json::to_string(&result).expect("serialize");
    let back: ImpactResult = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.paths.len(), result.paths.len());
    assert_eq!(back.truncated, result.truncated);
    assert_eq!(back.total_visited, result.total_visited);
}

#[test]
fn direction_wire_strings_are_stable() {
    assert_eq!(serde_json::to_string(&Direction::CallerToCallee).unwrap(), "\"caller_to_callee\"");
    assert_eq!(serde_json::to_string(&Direction::CalleeToCaller).unwrap(), "\"callee_to_caller\"");
    assert_eq!(serde_json::to_string(&Direction::Bidirectional).unwrap(), "\"bidirectional\"");
    assert_eq!(serde_json::to_string(&Direction::Undirected).unwrap(), "\"undirected\"");
}

#[test]
fn provenance_wire_strings_are_stable() {
    assert_eq!(serde_json::to_string(&Provenance::Observed).unwrap(), "\"observed\"");
    assert_eq!(serde_json::to_string(&Provenance::Inferred).unwrap(), "\"inferred\"");
    assert_eq!(serde_json::to_string(&Provenance::Heuristic).unwrap(), "\"heuristic\"");
}

#[test]
fn empty_edges_yield_zero_truncated_false() {
    let limits = ImpactLimits { max_depth: 10, max_nodes: 100, direction: Direction::CallerToCallee };
    let result = build_impact_result(std::iter::empty::<ImpactEdge>(), &limits);
    assert!(result.paths.is_empty());
    assert!(!result.truncated);
    assert_eq!(result.total_visited, 0);
}

#[test]
fn single_edge_single_path_marks_complete_when_limits_loose() {
    let edges = vec![edge("a", "b", Direction::CallerToCallee, Provenance::Observed, 1.0)];
    let limits = ImpactLimits { max_depth: 10, max_nodes: 100, direction: Direction::CallerToCallee };
    let result = build_impact_result(edges, &limits);
    assert!(!result.truncated);
    let path = result.paths.first().expect("one path");
    assert_eq!(path.edges.len(), 1);
    assert!(path.complete);
}

#[test]
fn coverage_is_bounded_in_unit_interval() {
    let edges: Vec<ImpactEdge> = (0..10)
        .map(|i| edge("a", if i == 0 { "b" } else { "c" }, Direction::CallerToCallee, Provenance::Observed, 0.1))
        .collect();
    let limits = ImpactLimits { max_depth: 10, max_nodes: 4, direction: Direction::CallerToCallee };
    let result = build_impact_result(edges, &limits);
    assert!(result.truncated);
    for path in &result.paths {
        assert!((0.0..=1.0).contains(&path.coverage), "coverage must be in [0,1]; got {}", path.coverage);
    }
}

#[test]
fn impact_path_wire_shape_has_complete_and_coverage() {
    let path = ImpactPath {
        edges: vec![edge("a", "b", Direction::CallerToCallee, Provenance::Observed, 1.0)],
        complete: false,
        coverage: 0.5,
    };
    let json = serde_json::to_value(&path).expect("serialize path");
    assert!(json.get("edges").and_then(|v| v.as_array()).is_some());
    assert_eq!(json.get("complete").and_then(|v| v.as_bool()), Some(false));
    let coverage = json.get("coverage").and_then(|v| v.as_f64()).expect("coverage");
    assert!((coverage - 0.5).abs() < f64::EPSILON);
}

#[test]
fn catalog_serializes_impact_direction_floor_field() {
    let catalog = load_tool_catalog();
    let descriptor = catalog.iter().find(|t| t.name == "analyze_impact")
        .expect("analyze_impact must be present");
    let json: Value = serde_json::to_value(descriptor).expect("serialize descriptor");
    assert!(json.get("impact_direction_floor").is_some());
}
