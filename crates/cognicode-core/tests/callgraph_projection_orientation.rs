//! Regression test: CallGraphProjection::build_adjacency orientation contract.
//!
//! The `GraphBuilder` trait contract requires:
//! - `in_neighbors[v]` contains every `u` such that `u → v` (callers of `v`)
//! - `out_degree[v]` is the count of edges `v → w` (callees of `v`)
//!
//! A previous implementation used `graph.edges(ni)` which defaults to
//! `Outgoing` for directed graphs, storing outgoing targets as
//! `in_neighbors` — silently violating the contract on non-uniform graphs.
//!
//! This test creates a `CallGraph` with a non-uniform structure and verifies
//! that `build_adjacency` returns correct caller/callee separation.

use cognicode_core::domain::aggregates::SymbolId;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::services::ExtractionContext;
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_core::infrastructure::graph::CallGraphProjection;
use cognicode_graph_algos::GraphBuilder;

/// Diamond graph: A↔B (mutual calls), B→C, A→C.
/// ```
///   A ──┬── B
///    │  │
///    ▼  ▼
///    C ◄─┘ (B→C, A→C)
/// ```
///
/// In our edge model, `A → B` means "A calls B" (A is caller, B is callee).
///
/// For PageRank to accumulate rank on callees (god-node semantics),
/// in_neighbors must contain callers:
///   - in_neighbors[A] = {B}  (A is called by B)
///   - in_neighbors[B] = {A}  (B is called by A)
///   - in_neighbors[C] = {A,B}(C is called by A and B)
///   - out_degree[A] = 2  (A calls B and C)
///   - out_degree[B] = 1  (B calls C)
///   - out_degree[C] = 0  (C calls nobody)

fn sym(name: &str) -> Symbol {
    Symbol::new(name, SymbolKind::Function, Location::new("test.rs", 1, 1))
}

fn id(name: &str) -> SymbolId {
    SymbolId::new(format!("test.rs:{name}:1"))
}

fn add_edge(g: &mut CallGraph, a: &str, b: &str) {
    g.add_symbol(sym(a));
    g.add_symbol(sym(b));
    let _ = g.add_dependency_with_provenance(
        &id(a),
        &id(b),
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    );
}

#[test]
fn in_neighbors_contains_actual_callers() {
    let mut g = CallGraph::new();
    // A→C, B→C, A↔B
    add_edge(&mut g, "A", "B"); // A calls B
    add_edge(&mut g, "B", "A"); // B calls A (mutual)
    add_edge(&mut g, "A", "C"); // A calls C
    add_edge(&mut g, "B", "C"); // B calls C

    let projection = CallGraphProjection::from_call_graph(&g);
    let (in_neighbors, out_degree) = projection.build_adjacency();

    let a_idx = projection.id_to_index().get(&id("A")).unwrap().index();
    let b_idx = projection.id_to_index().get(&id("B")).unwrap().index();
    let c_idx = projection.id_to_index().get(&id("C")).unwrap().index();

    // C's callers must be A and B (they both call C)
    let c_callers = &in_neighbors[c_idx];
    assert!(
        c_callers.contains(&a_idx) && c_callers.contains(&b_idx),
        "C must have A and B as callers (in_neighbors), but has {:?}",
        c_callers
    );

    // A's callers must include B (B calls A)
    let a_callers = &in_neighbors[a_idx];
    assert!(
        a_callers.contains(&b_idx),
        "A must be called by B (in_neighbors), but has {:?}",
        a_callers
    );

    // B's callers must include A (A calls B)
    let b_callers = &in_neighbors[b_idx];
    assert!(
        b_callers.contains(&a_idx),
        "B must be called by A (in_neighbors), but has {:?}",
        b_callers
    );

    // C calls nobody
    assert_eq!(
        out_degree[c_idx], 0,
        "C must have out_degree 0 (calls nobody), but has {}",
        out_degree[c_idx]
    );

    // A calls B and C
    assert_eq!(
        out_degree[a_idx], 2,
        "A must have out_degree 2, but has {}",
        out_degree[a_idx]
    );

    // B calls A (mutual) and C
    assert_eq!(
        out_degree[b_idx], 2,
        "B must have out_degree 2 (calls A and C), but has {}",
        out_degree[b_idx]
    );
}

/// Non-uniform graph: A→B→C chain with extra edges.
/// This catches orientation bugs where out_neighbors are stored as in_neighbors.
#[test]
fn chain_orientation_distinguishes_callers_from_callees() {
    let mut g = CallGraph::new();
    // A→B→C (A calls B, B calls C)
    add_edge(&mut g, "A", "B");
    add_edge(&mut g, "B", "C");

    let projection = CallGraphProjection::from_call_graph(&g);
    let (in_neighbors, out_degree) = projection.build_adjacency();

    let a_idx = projection.id_to_index().get(&id("A")).unwrap().index();
    let b_idx = projection.id_to_index().get(&id("B")).unwrap().index();
    let c_idx = projection.id_to_index().get(&id("C")).unwrap().index();

    // A has no callers (A only calls B, nobody calls A)
    assert!(
        in_neighbors[a_idx].is_empty(),
        "A must have no callers (in_neighbors empty), but has {:?}",
        in_neighbors[a_idx]
    );
    // A calls B
    assert_eq!(out_degree[a_idx], 1, "A must have out_degree 1");

    // B is called by A, calls C
    assert_eq!(
        in_neighbors[b_idx],
        vec![a_idx],
        "B must be called by A (in_neighbors)"
    );
    assert_eq!(out_degree[b_idx], 1, "B must have out_degree 1");

    // C is called by B, calls nobody
    assert_eq!(
        in_neighbors[c_idx],
        vec![b_idx],
        "C must be called by B (in_neighbors)"
    );
    assert_eq!(out_degree[c_idx], 0, "C must have out_degree 0");
}
