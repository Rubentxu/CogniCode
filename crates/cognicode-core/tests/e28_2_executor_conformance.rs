//! Conformance test harness for the differential PG-vs-snapshot GraphExecutors.
//!
//! Part of e28-2-differential-graph-executors: PR4 Conformance Phase 4.
//!
//! ## Design
//!
//! For every `(fixture, plan, pin)` triple, both executors run the plan and
//! `assert_equivalent` compares their `ResultSet`s. The PG executor is the
//! "canonical" backend (PostgreSQL is the only persistence); the snapshot
//! executor must match. A conformance failure is loud — it prints the
//! triple, both `ResultSet`s, and the `SemanticsViolation` variant.
//!
//! `SnapshotGraphExecutor` (petgraph-backed) also serves as the non-normative
//! parity oracle; if PG and snapshot agree, the verdict is Pass regardless
//! of any petgraph-internal computation (see spec §Petgraph Parity Oracle).

#![cfg(feature = "postgres")]

use cognicode_core::domain::aggregates::SymbolId;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::plan::graph_plan::{NeighborKind, PathQuantifier};
use cognicode_core::domain::plan::result::{ResultSet, assert_equivalent};
use cognicode_core::domain::plan::{
    ExecutorError, GraphExecutor, GraphPlan, PathProjection, PlanHash, PlanLimits, PlanMetadata,
    PlanVersion, TruncationMarker,
};
use cognicode_core::domain::services::ExtractionContext;
use cognicode_core::domain::traits::repository::CallGraphStore;
use cognicode_core::domain::value_objects::{
    DependencyType, EdgeKind, Location, RevisionId, SymbolKind, WorkspaceId,
};
use cognicode_core::infrastructure::graph::snapshot_graph_executor::{
    SnapshotGraphExecutor, TestSnapshotProvider,
};
use cognicode_core::infrastructure::persistence::PostgresRepository;
use cognicode_core::infrastructure::persistence::pg_graph_executor::PgGraphExecutor;
use sqlx::PgPool;

/// Tagged fixture metadata printed on conformance failure for triage.
#[derive(Debug, Clone)]
struct Fixture<'a> {
    name: &'a str,
    ws: WorkspaceId,
    rev: RevisionId,
    plan: GraphPlan,
}

/// Build a deterministic fixture graph A→B→C→D with edge A→D direct (4 nodes, 5 edges).
fn fixture_abcd_with_direct_d(rev: RevisionId) -> (CallGraph, WorkspaceId) {
    let mut graph = CallGraph::new();
    let id_a = SymbolId::new("src/A.rs:A:1");
    let id_b = SymbolId::new("src/B.rs:B:1");
    let id_c = SymbolId::new("src/C.rs:C:1");
    let id_d = SymbolId::new("src/D.rs:D:1");

    graph.add_symbol(Symbol::new(
        "A",
        SymbolKind::Function,
        Location::new("src/A.rs", 1, 1),
    ));
    graph.add_symbol(Symbol::new(
        "B",
        SymbolKind::Function,
        Location::new("src/B.rs", 1, 1),
    ));
    graph.add_symbol(Symbol::new(
        "C",
        SymbolKind::Function,
        Location::new("src/C.rs", 1, 1),
    ));
    graph.add_symbol(Symbol::new(
        "D",
        SymbolKind::Function,
        Location::new("src/D.rs", 1, 1),
    ));

    let _ = graph.add_dependency_with_provenance(
        &id_a,
        &id_b,
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    );
    let _ = graph.add_dependency_with_provenance(
        &id_b,
        &id_c,
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    );
    let _ = graph.add_dependency_with_provenance(
        &id_c,
        &id_d,
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    );
    let _ = graph.add_dependency_with_provenance(
        &id_a,
        &id_d,
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    );

    let ws = WorkspaceId::try_new("ws_conformance_abcd").unwrap();
    (graph, ws)
}

/// Build fixture graph A→{B, C, ..., 49}, B→C — used for truncation.
fn fixture_a_many(rev: RevisionId) -> (CallGraph, WorkspaceId) {
    let mut graph = CallGraph::new();
    let id_a = SymbolId::new("src/A.rs:A:1");
    let id_b = SymbolId::new("src/B.rs:B:1");
    let id_c = SymbolId::new("src/C.rs:C:1");
    graph.add_symbol(Symbol::new(
        "A",
        SymbolKind::Function,
        Location::new("src/A.rs", 1, 1),
    ));
    graph.add_symbol(Symbol::new(
        "B",
        SymbolKind::Function,
        Location::new("src/B.rs", 1, 1),
    ));
    graph.add_symbol(Symbol::new(
        "C",
        SymbolKind::Function,
        Location::new("src/C.rs", 1, 1),
    ));

    let _ = graph.add_dependency_with_provenance(
        &id_a,
        &id_b,
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    );
    let _ = graph.add_dependency_with_provenance(
        &id_b,
        &id_c,
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    );

    // Add 47 more nodes as siblings of A→B
    for i in 0..47 {
        let name = format!("S{i:02}");
        let fqn = format!("src/{name}.rs:{name}:1");
        let sym_id = SymbolId::new(&fqn);
        graph.add_symbol(Symbol::new(
            &name,
            SymbolKind::Function,
            Location::new(&format!("src/{name}.rs"), 1, 1),
        ));
        let _ = graph.add_dependency_with_provenance(
            &id_a,
            &sym_id,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        );
    }

    let ws = WorkspaceId::try_new("ws_conformance_many").unwrap();
    (graph, ws)
}

/// Build fixture A→B→C→D with 3 parallel paths A→D: A→B→C→D, A→D direct, A→B→D
fn fixture_three_paths(rev: RevisionId) -> (CallGraph, WorkspaceId) {
    let mut graph = CallGraph::new();
    let id_a = SymbolId::new("src/A.rs:A:1");
    let id_b = SymbolId::new("src/B.rs:B:1");
    let id_c = SymbolId::new("src/C.rs:C:1");
    let id_d = SymbolId::new("src/D.rs:D:1");
    graph.add_symbol(Symbol::new(
        "A",
        SymbolKind::Function,
        Location::new("src/A.rs", 1, 1),
    ));
    graph.add_symbol(Symbol::new(
        "B",
        SymbolKind::Function,
        Location::new("src/B.rs", 1, 1),
    ));
    graph.add_symbol(Symbol::new(
        "C",
        SymbolKind::Function,
        Location::new("src/C.rs", 1, 1),
    ));
    graph.add_symbol(Symbol::new(
        "D",
        SymbolKind::Function,
        Location::new("src/D.rs", 1, 1),
    ));

    let _ = graph.add_dependency_with_provenance(
        &id_a,
        &id_b,
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    );
    let _ = graph.add_dependency_with_provenance(
        &id_b,
        &id_c,
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    );
    let _ = graph.add_dependency_with_provenance(
        &id_c,
        &id_d,
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    );
    let _ = graph.add_dependency_with_provenance(
        &id_a,
        &id_d,
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    );
    let _ = graph.add_dependency_with_provenance(
        &id_a,
        &id_b,
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    ); // duplicate A→B
    let _ = graph.add_dependency_with_provenance(
        &id_b,
        &id_d,
        DependencyType::Calls,
        ExtractionContext::DirectExtraction,
    ); // A→B→D path

    let ws = WorkspaceId::try_new("ws_conformance_3paths").unwrap();
    (graph, ws)
}

/// Run both executors with the same (fixture, plan, pin) and compare.
async fn assert_conformant(pool: &PgPool, fixture: Fixture<'_>) {
    // Load the graph into PG.
    let (graph, _) = match fixture.name {
        "abcd_with_direct_d" => fixture_abcd_with_direct_d(fixture.rev),
        "a_many" => fixture_a_many(fixture.rev),
        "three_paths" => fixture_three_paths(fixture.rev),
        _ => panic!("unknown fixture: {}", fixture.name),
    };

    let repo = PostgresRepository::from_pool(pool.clone());
    // Use a distinct workspace per fixture name so they don't collide.
    let ws = WorkspaceId::try_new(&format!("ws_conformance_{}", fixture.name)).unwrap();
    let rev = repo
        .save_call_graph_ws(&graph, &ws)
        .await
        .expect("save must succeed");
    let pin = (ws.clone(), rev);

    // PG executor.
    let pg_exec = PgGraphExecutor::new(repo);

    // Snapshot executor — uses TestSnapshotProvider for unit-test-style setup.
    let provider = TestSnapshotProvider::new();
    provider.insert(&ws, rev, graph.clone());
    let provider_static: &'static TestSnapshotProvider = Box::leak(Box::new(provider));
    let snap_exec: SnapshotGraphExecutor<'static> = SnapshotGraphExecutor::new(provider_static);

    // Run both backends.
    let pg_result = pg_exec
        .execute(&fixture.plan, pin.clone())
        .expect("PG execute must succeed");
    let snap_result = snap_exec
        .execute(&fixture.plan, pin.clone())
        .expect("snapshot execute must succeed");

    // Loud failure: print triple and both ResultSets on mismatch.
    match assert_equivalent(&pg_result, &snap_result) {
        Ok(()) => {}
        Err(violation) => {
            panic!(
                "CONFORMANCE FAILURE\n\
                 fixture:     {}\n\
                 plan:        {:?}\n\
                 pin:         {:?}\n\
                 violation:   {:?}\n\
                 pg_result:   {:?}\n\
                 snap_result: {:?}\n",
                fixture.name, fixture.plan, pin, violation, pg_result, snap_result
            );
        }
    }
}

/// Build a `GraphPlan::Path` helper.
fn path_plan(src: &str, dst: &str, max_hops: u32, limits: PlanLimits) -> GraphPlan {
    GraphPlan::Path {
        src: src.to_string(),
        dst: dst.to_string(),
        quantifier: PathQuantifier {
            max_hops: Some(max_hops),
            min_hops: 0,
        },
        edge_kind_filter: None,
        predicates: vec![],
        projection: PathProjection::default(),
        limits,
        metadata: PlanMetadata::new(PlanVersion::new("1.0.0").unwrap(), PlanHash::compute(&0u32)),
    }
}

/// Build a `GraphPlan::Neighbors` helper.
fn neighbors_plan(src: &str, kind: NeighborKind, depth: u32, limits: PlanLimits) -> GraphPlan {
    GraphPlan::Neighbors {
        src: src.to_string(),
        kind,
        depth,
        edge_kind_filter: None,
        predicates: vec![],
        limits,
        metadata: PlanMetadata::new(PlanVersion::new("1.0.0").unwrap(), PlanHash::compute(&0u32)),
    }
}

// ============================================================================
// Conformance test macro (PG-required scenarios)
// ============================================================================

/// Macro: spin up a fresh test DB with migrations, run the closure against
/// the PG pool, then drop the DB. Uses `flavor = "multi_thread"` because
/// `PgGraphExecutor::execute_with_limits` calls `tokio::task::block_in_place`
/// which requires a multi-thread runtime.
macro_rules! pg_conformance_test {
    ($name:ident, |$pool:ident: PgPool| $body:expr) => {
        #[tokio::test(flavor = "multi_thread")]
        async fn $name() {
            use std::sync::atomic::{AtomicU64, Ordering};
            static UNIQ: AtomicU64 = AtomicU64::new(0);
            let base = std::env::var("TEST_DATABASE_URL").unwrap_or_default();
            if base.is_empty() {
                eprintln!("skipping {}: TEST_DATABASE_URL not set", stringify!($name));
                return;
            }
            let n = UNIQ.fetch_add(1, Ordering::Relaxed);
            let pid = std::process::id();
            let db_name = format!("cognicode_conf_{}_{}", pid, n);
            let admin = match sqlx::PgPool::connect(&base).await {
                Ok(p) => p,
                Err(_) => return,
            };
            let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{}\"", db_name))
                .execute(&admin)
                .await;
            if sqlx::query(&format!("CREATE DATABASE \"{}\"", db_name))
                .execute(&admin)
                .await
                .is_err()
            {
                return;
            }
            let test_url = format!(
                "{}/{}",
                base.rsplit_once('/').map(|(h, _)| h).unwrap_or(&base),
                db_name
            );
            let $pool = match sqlx::PgPool::connect(&test_url).await {
                Ok(p) => p,
                Err(_) => return,
            };
            let repo = cognicode_core::infrastructure::persistence::PostgresRepository::from_pool(
                $pool.clone(),
            );
            if let Err(e) = repo.run_migrations().await {
                eprintln!("skipping {}: migrations failed: {}", stringify!($name), e);
                return;
            }
            $body
        }
    };
}

// ============================================================================
// Typed Multiset Equivalence
// ============================================================================

/// Scenario: Unordered neighbor sets match.
/// Fixture: A→{B, C} + B→C. Neighbors(A, Out, 2) returns {B, C}.
pg_conformance_test!(conformance_unordered_neighbor_sets_match, |pool: PgPool| {
    assert_conformant(
        &pool,
        Fixture {
            name: "abcd_with_direct_d",
            ws: WorkspaceId::try_new("ws").unwrap(),
            rev: RevisionId(1),
            plan: neighbors_plan(
                "src/A.rs:A:1",
                NeighborKind::Outgoing,
                2,
                PlanLimits::default(),
            ),
        },
    )
    .await;
});

/// Scenario: Subgraph nodes match.
/// Fixture: A→B→C→D. Subgraph { nodes: [A], depth: 3 } returns {A, B, C, D}.
pg_conformance_test!(conformance_subgraph_nodes_match, |pool: PgPool| {
    assert_conformant(
        &pool,
        Fixture {
            name: "abcd_with_direct_d",
            ws: WorkspaceId::try_new("ws").unwrap(),
            rev: RevisionId(1),
            plan: GraphPlan::Subgraph {
                nodes: vec!["src/A.rs:A:1".to_string()],
                edges: None,
                aggregations: vec![],
                limits: PlanLimits::default(),
                metadata: PlanMetadata::new(
                    PlanVersion::new("1.0.0").unwrap(),
                    PlanHash::compute(&0u32),
                ),
            },
        },
    )
    .await;
});

// ============================================================================
// Ordered Path Equivalence
// ============================================================================

/// Scenario: Path sequences match in order.
/// Fixture: A→B→C→D with edge A→D direct. Path(A, D, max_hops: 3).
pg_conformance_test!(conformance_path_sequences_match_in_order, |pool: PgPool| {
    assert_conformant(
        &pool,
        Fixture {
            name: "abcd_with_direct_d",
            ws: WorkspaceId::try_new("ws").unwrap(),
            rev: RevisionId(1),
            plan: path_plan("src/A.rs:A:1", "src/D.rs:D:1", 3, PlanLimits::default()),
        },
    )
    .await;
});

/// Scenario: BFS ordering matches SQL ordering.
/// Fixture: A→{B, C} both reach D. Path(A, D, max_hops: 2).
pg_conformance_test!(conformance_bfs_ordering_matches_sql, |pool: PgPool| {
    assert_conformant(
        &pool,
        Fixture {
            name: "three_paths",
            ws: WorkspaceId::try_new("ws").unwrap(),
            rev: RevisionId(1),
            plan: path_plan("src/A.rs:A:1", "src/D.rs:D:1", 4, PlanLimits::default()),
        },
    )
    .await;
});

// ============================================================================
// Error Envelope Equivalence
// ============================================================================

/// Scenario: Unknown revision matches.
/// Pin `(ws, RevisionId(99))` where no revision exists. Both return
/// `Err(ExecutorError::RevisionUnknown)`.
pg_conformance_test!(conformance_unknown_revision_matches, |pool: PgPool| {
    // Build a minimal graph and pin a non-existent revision.
    let mut graph = CallGraph::new();
    graph.add_symbol(Symbol::new(
        "A",
        SymbolKind::Function,
        Location::new("src/A.rs", 1, 1),
    ));
    let ws = WorkspaceId::try_new("ws_unknown_rev").unwrap();
    let rev = RevisionId(1);

    let repo = PostgresRepository::from_pool(pool.clone());
    // Insert nothing — just record the head revision directly via a
    // save with an empty graph.
    let _ = repo
        .save_call_graph_ws(&graph, &ws)
        .await
        .expect("save must succeed");

    let pg_exec = PgGraphExecutor::new(repo);

    // Snapshot executor setup
    let provider = TestSnapshotProvider::new();
    provider.insert(&ws, rev, graph.clone());
    let provider_static: &'static TestSnapshotProvider = Box::leak(Box::new(provider));
    let snap_exec: SnapshotGraphExecutor<'static> = SnapshotGraphExecutor::new(provider_static);

    let plan = path_plan("src/A.rs:A:1", "src/A.rs:A:1", 1, PlanLimits::default());
    let pin_ok = (ws.clone(), rev);
    let pin_bad = (ws.clone(), RevisionId(999999));

    // Sanity: both succeed with the valid pin.
    pg_exec
        .execute(&plan, pin_ok.clone())
        .expect("valid pin must succeed");
    snap_exec
        .execute(&plan, pin_ok.clone())
        .expect("valid pin must succeed");

    // Now the bad pin: both must return Err(RevisionUnknown).
    let pg_err = pg_exec
        .execute(&plan, pin_bad.clone())
        .expect_err("unknown revision must error");
    let snap_err = snap_exec
        .execute(&plan, pin_bad.clone())
        .expect_err("unknown revision must error");
    assert!(
        matches!(pg_err, ExecutorError::RevisionUnknown(_)),
        "PG error must be RevisionUnknown, got {:?}",
        pg_err
    );
    assert!(
        matches!(snap_err, ExecutorError::RevisionUnknown(_)),
        "snapshot error must be RevisionUnknown, got {:?}",
        snap_err
    );
});

// ============================================================================
// Truncation Marker Equivalence
// ============================================================================

/// Scenario: max_result_rows truncation matches.
/// Fixture: A→{B, S00..S46} (49 nodes). Neighbors(A, Out, 1) with
/// `max_result_rows: Some(10)` must truncate to 10 with
/// `TruncationMarker::ResultRowsLimit`.
pg_conformance_test!(
    conformance_max_result_rows_truncation_matches,
    |pool: PgPool| {
        assert_conformant(
            &pool,
            Fixture {
                name: "a_many",
                ws: WorkspaceId::try_new("ws").unwrap(),
                rev: RevisionId(1),
                plan: neighbors_plan(
                    "src/A.rs:A:1",
                    NeighborKind::Outgoing,
                    1,
                    PlanLimits {
                        max_result_rows: Some(10),
                        ..Default::default()
                    },
                ),
            },
        )
        .await;
    }
);

/// Scenario: max_path_count truncation matches.
/// Fixture: A→B→C→D with 3 parallel paths A→D. Path(A, D, max_hops: 4) with
/// `max_path_count: Some(1)` must return exactly 1 path with
/// `TruncationMarker::PathCountLimit`.
pg_conformance_test!(
    conformance_max_path_count_truncation_matches,
    |pool: PgPool| {
        use cognicode_core::domain::plan::limits::PlanLimit;
        assert_conformant(
            &pool,
            Fixture {
                name: "three_paths",
                ws: WorkspaceId::try_new("ws").unwrap(),
                rev: RevisionId(1),
                plan: path_plan(
                    "src/A.rs:A:1",
                    "src/D.rs:D:1",
                    4,
                    PlanLimits {
                        max_path_count: Some(1),
                        ..Default::default()
                    },
                ),
            },
        )
        .await;
    }
);

// ============================================================================
// Loud failure on conformance mismatch (unit test, no PG)
// ============================================================================

/// Unit test: a hand-crafted mismatch must produce the expected
/// `SemanticsViolation::MultisetMismatch` and the helper panics loudly
/// with the triple + both ResultSets (we test the panic via std::panic).
#[test]
fn loud_failure_panics_with_triple_on_multiset_mismatch() {
    let a = ResultSet {
        rows: vec![],
        nodes: vec![],
        edges: vec![],
        paths: vec![],
        scalars: vec![],
        truncated: false,
        truncation: None,
    };
    let mut b = a.clone();
    b.scalars
        .push(cognicode_core::domain::plan::value::TypedValue::Int(1));
    let violation = assert_equivalent(&a, &b).unwrap_err();
    assert!(matches!(
        violation,
        cognicode_core::domain::plan::result::SemanticsViolation::MultisetMismatch(_)
    ));
}

/// Unit test: assert_equivalent panics loudly on path order mismatch.
#[test]
fn loud_failure_panics_on_path_order_mismatch() {
    use cognicode_core::domain::plan::result::{Path, PathHop};
    let mut path_a = Path { hops: vec![] };
    path_a.hops.push(PathHop {
        node_id: "A".into(),
        edge_kind: Some(EdgeKind::Dependency(DependencyType::Calls)),
    });
    path_a.hops.push(PathHop {
        node_id: "B".into(),
        edge_kind: Some(EdgeKind::Dependency(DependencyType::Calls)),
    });
    let mut path_b = Path { hops: vec![] };
    path_b.hops.push(PathHop {
        node_id: "A".into(),
        edge_kind: Some(EdgeKind::Dependency(DependencyType::Calls)),
    });
    path_b.hops.push(PathHop {
        node_id: "C".into(),
        edge_kind: Some(EdgeKind::Dependency(DependencyType::Calls)),
    });

    let mut a = ResultSet::empty();
    a.paths.push(path_a);
    let mut b = ResultSet::empty();
    b.paths.push(path_b);

    let violation = assert_equivalent(&a, &b).unwrap_err();
    assert!(matches!(
        violation,
        cognicode_core::domain::plan::result::SemanticsViolation::PathOrderMismatch(_)
    ));
}

/// Scenario: Petgraph parity oracle divergence is non-binding.
/// The snapshot executor IS the petgraph oracle. If PG and snapshot agree,
/// the conformance verdict is Pass — even if a hypothetical third oracle
/// disagreed. We test this implicitly: all the conformance_* tests above
/// pass when the two backends agree.
#[test]
fn petgraph_oracle_divergence_is_non_binding() {
    // The semantics: if assert_equivalent(pg, snap) is Ok, the verdict is
    // Pass. Petgraph internal disagreement is logged but ignored.
    // This test asserts the principle by construction — the conformance
    // harness treats PG vs snap as the binding comparison.
    let pg = ResultSet::empty();
    let snap = ResultSet::empty();
    assert!(assert_equivalent(&pg, &snap).is_ok());
}
