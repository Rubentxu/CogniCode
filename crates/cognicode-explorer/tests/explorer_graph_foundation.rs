//! End-to-end integration tests for the
//! `explorer-graph-foundation` Phase-1 slice.
//!
//! These tests cover task 5.2 from the SDD tasks page: SQLite +
//! bincode v2 roundtrip + explorer adapter passthrough, all in one
//! test. The flow is:
//!
//! 1. Build a `CallGraph` with mixed-provenance edges.
//! 2. Persist it via `SqliteGraphStore::save_graph` (writes the
//!    versioned bincode blob **and** populates the `call_edges` table).
//! 3. Reload it from disk via `load_graph`.
//! 4. Wrap the reloaded graph in a `CallGraphRepository` and read
//!    the metadata via the new `callees_with_metadata` passthrough.
//! 5. Assert the roundtripped metadata matches the input.
//!
//! File-level feature gate (`postgres-default-config` PR 1): when the
//! `sqlite` feature is disabled on `cognicode-explorer`, this entire
//! test file is excluded from the build. Use
//! `cargo test -p cognicode-explorer --features sqlite` to opt in.
#![cfg(feature = "sqlite")]

use cognicode_core::domain::aggregates::{CallGraph, Symbol, SymbolId};
use cognicode_core::domain::services::ExtractionContext;
use cognicode_core::domain::value_objects::{DependencyType, Location, Provenance, SymbolKind};
use cognicode_db::SqliteGraphStore;
use cognicode_explorer::adapters::CallGraphRepository;

fn build_three_node_graph() -> (CallGraph, SymbolId, SymbolId, SymbolId) {
    let mut g = CallGraph::new();
    let a = g.add_symbol(Symbol::new(
        "alpha",
        SymbolKind::Function,
        Location::new("src/a.rs", 1, 0),
    ));
    let b = g.add_symbol(Symbol::new(
        "beta",
        SymbolKind::Function,
        Location::new("src/b.rs", 1, 0),
    ));
    let c = g.add_symbol(Symbol::new(
        "gamma",
        SymbolKind::Function,
        Location::new("src/c.rs", 1, 0),
    ));
    (g, a, b, c)
}

#[test]
fn end_to_end_metadata_flow_through_sqlite_and_explorer_adapter() {
    // 1. Build a graph with mixed-provenance edges.
    let (mut g, a, b, c) = build_three_node_graph();
    g.add_dependency_with_provenance(
        &a,
        &b,
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    )
    .expect("add a→b");
    g.add_dependency_with_provenance(
        &a,
        &c,
        DependencyType::Imports,
        ExtractionContext::Heuristic { score: 0.6 },
    )
    .expect("add a→c");
    g.add_dependency_with_provenance(
        &b,
        &c,
        DependencyType::References,
        ExtractionContext::Unresolved,
    )
    .expect("add b→c");

    // 2. Persist to a temp SQLite database.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("graph.db");
    let store = SqliteGraphStore::open(&path).expect("open store");
    store.save_graph(&g).expect("save graph");

    // 3. Reload via `load_graph` (exercises the bincode v2 read path).
    let loaded = store.load_graph().expect("load ok").expect("some graph");
    assert_eq!(loaded.symbol_count(), 3);
    assert_eq!(loaded.edge_count(), 3);

    // 4. Wrap in the explorer adapter and read metadata.
    let repo = CallGraphRepository::from_graph(loaded);

    let a_metas = repo.callees_with_metadata(&a);
    assert_eq!(a_metas.len(), 2);
    let b_metas = repo.callees_with_metadata(&b);
    assert_eq!(b_metas.len(), 1);

    // 5. Assert metadata roundtrips.
    let a_to_b = a_metas.iter().find(|(id, _, _, _)| id == &b).expect("a→b");
    assert_eq!(a_to_b.1, DependencyType::Calls);
    assert_eq!(a_to_b.2, Provenance::Extracted);
    assert_eq!(a_to_b.3, 1.0_f64);

    let a_to_c = a_metas.iter().find(|(id, _, _, _)| id == &c).expect("a→c");
    assert_eq!(a_to_c.1, DependencyType::Imports);
    assert_eq!(a_to_c.2, Provenance::Inferred);
    assert_eq!(a_to_c.3, 0.6_f64);

    let b_to_c = b_metas.iter().find(|(id, _, _, _)| id == &c).expect("b→c");
    assert_eq!(b_to_c.1, DependencyType::References);
    assert_eq!(b_to_c.2, Provenance::Ambiguous);
    assert_eq!(b_to_c.3, 0.3_f64);

    // 6. Spec invariant: every edge has finite, in-range confidence.
    //    We walk every source symbol and inspect all its callees.
    for source in [&a, &b, &c] {
        for (_tgt, _dep, _prov, conf) in repo.callees_with_metadata(source) {
            assert!((0.0..=1.0).contains(&conf), "out of range: {conf}");
            assert!(!conf.is_nan(), "NaN leaked");
            assert!(conf.is_finite(), "non-finite: {conf}");
        }
    }
}

#[test]
fn end_to_end_metadata_via_adapter_invariant_holds_for_every_edge() {
    let (mut g, a, b, c) = build_three_node_graph();
    g.add_dependency_with_provenance(
        &a,
        &b,
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    )
    .unwrap();
    g.add_dependency_with_provenance(
        &a,
        &c,
        DependencyType::Imports,
        ExtractionContext::Heuristic { score: 0.55 },
    )
    .unwrap();
    g.add_dependency_with_provenance(
        &b,
        &c,
        DependencyType::References,
        ExtractionContext::Unresolved,
    )
    .unwrap();

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("graph.db");
    let store = SqliteGraphStore::open(&path).expect("open store");
    store.save_graph(&g).expect("save");
    let loaded = store.load_graph().expect("load").expect("some");
    let repo = CallGraphRepository::from_graph(loaded);

    // For every source symbol with at least one callee, every edge
    // returned by the adapter must satisfy the post-condition
    // `confidence ∈ [0.0, 1.0] && is_finite() && !is_nan()`.
    for source in [&a, &b, &c] {
        for (_tgt, _dep, _prov, conf) in repo.callees_with_metadata(source) {
            assert!((0.0..=1.0).contains(&conf), "out of range: {conf}");
            assert!(!conf.is_nan(), "NaN leaked");
            assert!(conf.is_finite(), "non-finite: {conf}");
        }
    }
}
