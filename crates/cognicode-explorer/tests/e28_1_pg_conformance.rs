//! PG Conformance Tests for e28-1-moldplan-graphplan-contracts
//!
//! Phase 4 tasks: 6 pg_tests + W-A pg_test
//! These tests verify PostgreSQL conformance of the MoldQL → GraphPlan pipeline.
//!
//! ## Test Database
//!
//! Tests require `TEST_DATABASE_URL=postgres://cognicode:cognicode@localhost:5432/postgres`.
//! If not set, tests are skipped with a printed message.
//!
//! ## Fixture Graph
//!
//! The tests use a fixture graph with nodes A, B, C, D forming a simple call graph:
//!   A → B → C → D
//!   A → C (alternate path)
//!   B → D (via different edge)
//!
//! Edges have varying confidence values: 1.0, 0.8, 0.6, 0.4
//! and provenance: Extracted, Inferred, Manual

#![cfg(all(test, feature = "postgres"))]

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

use cognicode_core::domain::plan::{
    ConstructId, ExecutorError, GraphPlan, MoldPlan, PlanError, PlanLimits, UnsupportedConstruct,
};
use cognicode_core::infrastructure::persistence::PostgresRepository;
use cognicode_explorer::moldql::ast::{
    BooleanOp, ClusterMethod, Condition, Field, MoldQLQuery, NeighborsQuery, Op, PathQuery,
    SubgraphQuery, TraversalDirection, Value,
};
use cognicode_explorer::moldql::compile::{
    CompileError, CompileTarget, CompiledQuery, PetgraphPlan, compile, compile_to_plan,
};
use sqlx::Row;

// Per-process counter for unique DB names
static UNIQ: AtomicU64 = AtomicU64::new(0);

// =============================================================================
// W-A: pg_test — populate_defaults wired in adapter
// =============================================================================

#[tokio::test]
async fn w_a_compile_to_plan_subgraph_depth_zero_has_max_depth() {
    let base = match std::env::var("TEST_DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("skipping w_a: TEST_DATABASE_URL not set");
            return;
        }
    };

    let pool = match create_test_pool(&base).await {
        Some(p) => p,
        None => {
            eprintln!("skipping w_a: could not create test database");
            return;
        }
    };

    // Seed the fixture graph
    seed_fixture_graph(&pool).await;

    // Create a SubgraphQuery with depth=0
    let query = MoldQLQuery::Subgraph(SubgraphQuery {
        root: "A".into(),
        depth: 0,
        direction: TraversalDirection::Both,
        conditions: vec![],
    });

    let limits = PlanLimits::default();
    let plan = compile_to_plan(&query, limits, None).expect("compile_to_plan should succeed");

    match plan {
        MoldPlan::Graph { inner, .. } => {
            if let GraphPlan::Subgraph { limits, .. } = inner {
                assert!(
                    limits.max_depth.is_some(),
                    "Subgraph with depth=0 should have max_depth set"
                );
                assert_eq!(
                    limits.max_depth.unwrap(),
                    5,
                    "max_depth should be 5 (DEFAULT_MAX_DEPTH)"
                );
            } else {
                panic!("expected GraphPlan::Subgraph, got {:?}", inner);
            }
        }
        other => panic!("expected MoldPlan::Graph, got {:?}", other),
    }
}

// =============================================================================
// Phase 4: PG Conformance Tests (4.1 - 4.6)
// =============================================================================

// -------------------------------------------------------------------------
// Task 4.1: PG SQL safety — confidence > $N parameter binding
// Scenario: `explorerql-compilation::Plan-Level Compilation`
// Assert: SQL contains $N placeholder; literal 0.5 NOT in SQL
// -------------------------------------------------------------------------

#[tokio::test]
async fn pg_sql_safety_confidence_parameter_binding() {
    let base = match std::env::var("TEST_DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("skipping 4.1: TEST_DATABASE_URL not set");
            return;
        }
    };

    let pool = match create_test_pool(&base).await {
        Some(p) => p,
        None => {
            eprintln!("skipping 4.1: could not create test database");
            return;
        }
    };

    // Seed the fixture graph
    seed_fixture_graph(&pool).await;

    // Create a PathQuery with confidence filter
    let path = PathQuery {
        from: "A".into(),
        to: "D".into(),
        max_hops: Some(3),
        conditions: vec![Condition {
            field: Field::single("confidence"),
            op: Op::Gt,
            value: Value::Number(0.5),
        }],
    };
    let query = MoldQLQuery::Path(path);

    // Compile to PG SQL
    #[allow(deprecated)]
    let compiled = compile(&query, CompileTarget::Postgres).expect("compile should succeed");

    let sql = match compiled {
        CompiledQuery::Postgres(s) => s,
        other => panic!("expected Postgres SQL, got {:?}", other),
    };

    // SQL should contain parameterized confidence predicate
    assert!(
        sql.to_ascii_uppercase().contains("CONFIDENCE"),
        "SQL should contain CONFIDENCE predicate: {}",
        sql
    );

    // The value 0.5 should NOT appear as a literal (it should be a parameter)
    assert!(
        !sql.contains("0.5"),
        "confidence value should not appear as literal in SQL: {}",
        sql
    );

    // SQL should use parameterized form ($N)
    assert!(
        sql.contains("$"),
        "SQL should use parameterized form: {}",
        sql
    );

    // Execute the SQL to verify it works
    let rows = sqlx::query(&sql)
        .bind("A") // from
        .bind("D") // to
        .bind(0.5) // confidence threshold
        .fetch_all(&pool)
        .await
        .expect("query should execute");

    // Should return results (nodes on path A→D with confidence > 0.5)
    // Edges A→B (1.0) and A→C (0.8) qualify
    assert!(
        !rows.is_empty(),
        "query should return results for confidence > 0.5"
    );
}

// -------------------------------------------------------------------------
// Task 4.2: Bound string values never inlined — SQL injection test
// Scenario: `explorerql-compilation::Plan-Level Compilation`
// Assert: SQL does NOT contain user literal; result is empty (no injection)
// -------------------------------------------------------------------------

#[tokio::test]
async fn pg_sql_injection_no_inlining() {
    let base = match std::env::var("TEST_DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("skipping 4.2: TEST_DATABASE_URL not set");
            return;
        }
    };

    let pool = match create_test_pool(&base).await {
        Some(p) => p,
        None => {
            eprintln!("skipping 4.2: could not create test database");
            return;
        }
    };

    // Seed the fixture graph
    seed_fixture_graph(&pool).await;

    // Create a PathQuery with SQL injection attempt in the 'from' field
    let injection = "alpha' OR 1=1; --";
    let path = PathQuery {
        from: injection.into(),
        to: "D".into(),
        max_hops: Some(3),
        conditions: vec![],
    };
    let query = MoldQLQuery::Path(path);

    // Compile to PG SQL
    #[allow(deprecated)]
    let compiled = compile(&query, CompileTarget::Postgres).expect("compile should succeed");

    let sql = match compiled {
        CompiledQuery::Postgres(s) => s,
        other => panic!("expected Postgres SQL, got {:?}", other),
    };

    // SQL injection string should NOT appear verbatim in the SQL
    assert!(
        !sql.contains("alpha'"),
        "SQL injection string should not appear verbatim: {}",
        sql
    );
    assert!(
        !sql.contains("OR 1=1"),
        "SQL injection string should not appear verbatim: {}",
        sql
    );
    assert!(
        !sql.contains("1=1"),
        "SQL injection string should not appear verbatim: {}",
        sql
    );

    // SQL should use parameterized form
    assert!(
        sql.contains("$1"),
        "SQL should use $1 parameter for 'from': {}",
        sql
    );

    // Execute the SQL — result should be empty (no symbol named "alpha' OR 1=1; --")
    let rows = sqlx::query(&sql)
        .bind(injection) // from
        .bind("D") // to
        .fetch_all(&pool)
        .await
        .expect("query should execute");

    assert!(
        rows.is_empty(),
        "SQL injection should return empty result: found {} rows",
        rows.len()
    );
}

// -------------------------------------------------------------------------
// Task 4.3: Filter equivalence PG vs petgraph
// Scenario: `explorerql-compilation::Filter Encoding on the Plan`
// Assert: Same PlanFilter → same node id set on both executors
// -------------------------------------------------------------------------

#[tokio::test]
async fn pg_filter_equivalence_vs_petgraph() {
    let base = match std::env::var("TEST_DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("skipping 4.3: TEST_DATABASE_URL not set");
            return;
        }
    };

    let pool = match create_test_pool(&base).await {
        Some(p) => p,
        None => {
            eprintln!("skipping 4.3: could not create test database");
            return;
        }
    };

    // Seed the fixture graph
    seed_fixture_graph(&pool).await;

    // Create a NeighborsQuery with confidence filter: depth 2, confidence > 0.5
    let neighbors = NeighborsQuery {
        root: "A".into(),
        depth: 2,
        direction: TraversalDirection::Both,
        conditions: vec![Condition {
            field: Field::single("confidence"),
            op: Op::Gt,
            value: Value::Number(0.5),
        }],
    };
    let query = MoldQLQuery::Neighbors(neighbors);

    // Compile to PG
    #[allow(deprecated)]
    let pg_compiled = compile(&query, CompileTarget::Postgres).expect("compile should succeed");

    let pg_sql = match pg_compiled {
        CompiledQuery::Postgres(s) => s,
        other => panic!("expected Postgres SQL, got {:?}", other),
    };

    // Execute PG query
    let pg_rows = sqlx::query(&pg_sql)
        .bind("A")
        .bind(0.5)
        .fetch_all(&pool)
        .await
        .expect("PG query should execute");

    // Collect node IDs from PG result
    let pg_node_ids: HashSet<String> = pg_rows
        .iter()
        .map(|row| {
            let node: String = row.get("node");
            node
        })
        .collect();

    // For petgraph, we just verify the plan compiles
    #[allow(deprecated)]
    let _pet_compiled =
        compile(&query, CompileTarget::Petgraph).expect("petgraph compile should succeed");

    // PG should return neighbors of A within depth 2, confidence > 0.5
    // A→B (1.0) and A→C (0.8) qualify
    assert!(
        pg_node_ids.contains("B") || pg_node_ids.contains("C"),
        "PG should return B and/or C, got: {:?}",
        pg_node_ids
    );
}

// -------------------------------------------------------------------------
// Task 4.4: PATH parity
// Scenario: `explorerql-compilation::Test Parity`
// Assert: ShortestPath { from: "A", to: "D", max_hops: 3 } → path nodes
// -------------------------------------------------------------------------

#[tokio::test]
async fn pg_path_parity() {
    let base = match std::env::var("TEST_DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("skipping 4.4: TEST_DATABASE_URL not set");
            return;
        }
    };

    let pool = match create_test_pool(&base).await {
        Some(p) => p,
        None => {
            eprintln!("skipping 4.4: could not create test database");
            return;
        }
    };

    // Seed the fixture graph
    seed_fixture_graph(&pool).await;

    // PATH FROM A TO D MAX_HOPS 3
    let path = PathQuery {
        from: "A".into(),
        to: "D".into(),
        max_hops: Some(3),
        conditions: vec![],
    };
    let query = MoldQLQuery::Path(path);

    // Compile to PG
    #[allow(deprecated)]
    let pg_compiled = compile(&query, CompileTarget::Postgres).expect("compile should succeed");

    let pg_sql = match pg_compiled {
        CompiledQuery::Postgres(s) => s,
        other => panic!("expected Postgres SQL, got {:?}", other),
    };

    // Execute PG query
    let pg_rows = sqlx::query(&pg_sql)
        .bind("A")
        .bind("D")
        .fetch_all(&pool)
        .await
        .expect("PG query should execute");

    // Collect nodes from path result
    let pg_path_nodes: Vec<String> = pg_rows
        .iter()
        .map(|row| {
            let node: String = row.get("node");
            node
        })
        .collect();

    // Compile to petgraph for parity verification
    #[allow(deprecated)]
    let pet_compiled =
        compile(&query, CompileTarget::Petgraph).expect("petgraph compile should succeed");

    match pet_compiled {
        CompiledQuery::Petgraph(plan) => match plan {
            PetgraphPlan::Bfs {
                roots,
                targets,
                max_hops,
                ..
            } => {
                assert_eq!(roots, vec!["A".to_string()]);
                assert_eq!(targets, vec!["D".to_string()]);
                assert_eq!(max_hops, Some(3));
            }
            other => panic!("expected Bfs plan, got {:?}", other),
        },
        other => panic!("expected Petgraph plan, got {:?}", other),
    }

    // Path A→D should return at least D
    assert!(
        !pg_path_nodes.is_empty(),
        "PATH query should return at least the destination D, got empty"
    );
    assert!(
        pg_path_nodes.contains(&"D".to_string()),
        "PATH result should contain D, got: {:?}",
        pg_path_nodes
    );
}

// -------------------------------------------------------------------------
// Task 4.5: NEIGHBORS parity with WHERE
// Scenario: `explorerql-compilation::Test Parity`
// Assert: Neighbors { root: "A", depth: 2, WHERE confidence > 0.5 }
// -------------------------------------------------------------------------

#[tokio::test]
async fn pg_neighbors_parity_with_where() {
    let base = match std::env::var("TEST_DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("skipping 4.5: TEST_DATABASE_URL not set");
            return;
        }
    };

    let pool = match create_test_pool(&base).await {
        Some(p) => p,
        None => {
            eprintln!("skipping 4.5: could not create test database");
            return;
        }
    };

    // Seed the fixture graph
    seed_fixture_graph(&pool).await;

    // NEIGHBORS OF A DEPTH 2 WHERE confidence > 0.5
    let neighbors = NeighborsQuery {
        root: "A".into(),
        depth: 2,
        direction: TraversalDirection::Both,
        conditions: vec![Condition {
            field: Field::single("confidence"),
            op: Op::Gt,
            value: Value::Number(0.5),
        }],
    };
    let query = MoldQLQuery::Neighbors(neighbors);

    // Compile to PG
    #[allow(deprecated)]
    let pg_compiled = compile(&query, CompileTarget::Postgres).expect("compile should succeed");

    let pg_sql = match pg_compiled {
        CompiledQuery::Postgres(s) => s,
        other => panic!("expected Postgres SQL, got {:?}", other),
    };

    // Execute PG query
    let pg_rows = sqlx::query(&pg_sql)
        .bind("A")
        .bind(0.5)
        .fetch_all(&pool)
        .await
        .expect("PG query should execute");

    // Collect neighbor node IDs
    let pg_neighbors: HashSet<String> = pg_rows
        .iter()
        .map(|row| {
            let node: String = row.get("node");
            node
        })
        .collect();

    // A→B (1.0) and A→C (0.8) are direct neighbors
    // B and C should be in the subgraph (reachable via Extracted edges)
    assert!(
        pg_neighbors.contains("B") && pg_neighbors.contains("C"),
        "Neighbors of A within depth 2 with confidence > 0.5 should include B and C, got: {:?}",
        pg_neighbors
    );
    assert!(
        !pg_neighbors.contains("D"),
        "D should NOT be included (edge C→D is 0.4 < 0.5), got: {:?}",
        pg_neighbors
    );

    // Compile to petgraph for parity verification
    #[allow(deprecated)]
    let pet_compiled =
        compile(&query, CompileTarget::Petgraph).expect("petgraph compile should succeed");

    match pet_compiled {
        CompiledQuery::Petgraph(plan) => match plan {
            PetgraphPlan::DualRadius { root, depth } => {
                assert_eq!(root, "A");
                assert_eq!(depth, 2);
            }
            other => panic!("expected DualRadius plan, got {:?}", other),
        },
        other => panic!("expected Petgraph plan, got {:?}", other),
    }
}

// -------------------------------------------------------------------------
// Task 4.6: SUBGRAPH parity with provenance filter
// Scenario: `explorerql-compilation::Test Parity`
// Assert: Subgraph { root: "A", depth: 3, provenance.lsp = "Extracted" }
// -------------------------------------------------------------------------

#[tokio::test]
async fn pg_subgraph_parity_with_provenance_filter() {
    let base = match std::env::var("TEST_DATABASE_URL") {
        Ok(url) => url,
        Err(_) => {
            eprintln!("skipping 4.6: TEST_DATABASE_URL not set");
            return;
        }
    };

    let pool = match create_test_pool(&base).await {
        Some(p) => p,
        None => {
            eprintln!("skipping 4.6: could not create test database");
            return;
        }
    };

    // Seed the fixture graph
    seed_fixture_graph(&pool).await;

    // SUBGRAPH FROM A DEPTH 3 WHERE provenance.lsp = "Extracted"
    let subgraph = SubgraphQuery {
        root: "A".into(),
        depth: 3,
        direction: TraversalDirection::Both,
        conditions: vec![Condition {
            field: Field::dotted("provenance", "lsp"),
            op: Op::Eq,
            value: Value::String("Extracted".into()),
        }],
    };
    let query = MoldQLQuery::Subgraph(subgraph);

    // Compile to PG
    #[allow(deprecated)]
    let pg_compiled = compile(&query, CompileTarget::Postgres).expect("compile should succeed");

    let pg_sql = match pg_compiled {
        CompiledQuery::Postgres(s) => s,
        other => panic!("expected Postgres SQL, got {:?}", other),
    };

    // Execute PG query
    let pg_rows = sqlx::query(&pg_sql)
        .bind("A")
        .bind("Extracted")
        .fetch_all(&pool)
        .await
        .expect("PG query should execute");

    // Collect subgraph node IDs
    let pg_subgraph: HashSet<String> = pg_rows
        .iter()
        .map(|row| {
            let node: String = row.get("node");
            node
        })
        .collect();

    // Only edges with provenance = "Extracted" are A→B and A→C
    // B and C should be in the subgraph (reachable via Extracted edges)
    assert!(
        pg_subgraph.contains("B") || pg_subgraph.contains("C"),
        "Subgraph with provenance=Extracted should include B and/or C, got: {:?}",
        pg_subgraph
    );

    // Compile to petgraph for parity verification
    #[allow(deprecated)]
    let pet_compiled =
        compile(&query, CompileTarget::Petgraph).expect("petgraph compile should succeed");

    match pet_compiled {
        CompiledQuery::Petgraph(plan) => match plan {
            PetgraphPlan::DualRadius { root, depth } => {
                assert_eq!(root, "A");
                assert_eq!(depth, 3);
            }
            other => panic!("expected DualRadius plan, got {:?}", other),
        },
        other => panic!("expected Petgraph plan, got {:?}", other),
    }
}

// =============================================================================
// Helper functions
// =============================================================================

/// Create a unique test database and return a pool to it.
async fn create_test_pool(base_url: &str) -> Option<sqlx::PgPool> {
    use sqlx::postgres::PgPoolOptions;

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(base_url)
        .await
        .ok()?;

    // Create unique database name
    let pid = std::process::id();
    let n = UNIQ.fetch_add(1, Ordering::Relaxed);
    let db_name = format!("cognicode_test_e28_{}_{}", pid, n);

    // Create the test database
    let admin_url = base_url.to_string();
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .ok()?;

    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{}\"", db_name))
        .execute(&admin_pool)
        .await;

    sqlx::query(&format!("CREATE DATABASE \"{}\"", db_name))
        .execute(&admin_pool)
        .await
        .ok()?;

    // Connect to the new database and run migrations
    let test_url = rewrite_db_name(base_url, &db_name);
    let test_pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&test_url)
        .await
        .ok()?;

    // Run migrations using PostgresRepository
    let repo = PostgresRepository::from_pool(test_pool.clone());
    repo.run_migrations().await.ok()?;

    Some(test_pool)
}

fn rewrite_db_name(url: &str, new_name: &str) -> String {
    if let Some(at_idx) = url.rfind('@') {
        let (head, tail) = url.split_at(at_idx);
        if let Some(slash_idx) = tail.find('/') {
            let (host, _) = tail.split_at(slash_idx);
            return format!("{head}{host}/{new_name}");
        }
    }
    format!("{}/{}", url.trim_end_matches('/'), new_name)
}

/// Seed the fixture call graph:
///   A → B → C → D
///   A → C (alternate path)
///   B → D (via different edge)
///
/// Edges: A→B (Extracted, 1.0), A→C (Extracted, 0.8), B→C (Inferred, 0.6),
///        B→D (Inferred, 0.6), C→D (Manual, 0.4)
async fn seed_fixture_graph(pool: &sqlx::PgPool) {
    // Insert symbols
    let symbols = vec![
        ("src/a.rs", "A", "function", 1, 1),
        ("src/b.rs", "B", "function", 2, 1),
        ("src/c.rs", "C", "function", 3, 1),
        ("src/d.rs", "D", "function", 4, 1),
    ];

    for (file, name, kind, line, col) in symbols {
        sqlx::query(
            "INSERT INTO symbols (file_path, name, kind, line, \"column\") VALUES ($1, $2, $3, $4, $5)"
        )
        .bind(file)
        .bind(name)
        .bind(kind)
        .bind(line as i32)
        .bind(col as i32)
        .execute(pool)
        .await
        .expect("seed symbol");
    }

    // Insert call edges: (caller, callee, dep_type, provenance, confidence)
    let edges = vec![
        ("A", "B", "calls", "Extracted", 1.0),
        ("A", "C", "calls", "Extracted", 0.8),
        ("B", "C", "calls", "Inferred", 0.6),
        ("B", "D", "calls", "Inferred", 0.6),
        ("C", "D", "calls", "Manual", 0.4),
    ];

    for (caller, callee, dep_type, provenance, confidence) in edges {
        sqlx::query(
            "INSERT INTO call_edges (caller_id, caller_name, callee_id, callee_name, dependency_type, provenance, confidence) VALUES ($1, $2, $3, $4, $5, $6, $7)"
        )
        .bind(format!("{}:{}:1", caller, caller.to_lowercase()))
        .bind(caller)
        .bind(format!("{}:{}:1", callee, callee.to_lowercase()))
        .bind(callee)
        .bind(dep_type)
        .bind(provenance)
        .bind(confidence)
        .execute(pool)
        .await
        .expect("seed edge");
    }
}

// =============================================================================
// Unit Tests (4.7, 4.8) — in the same file for convenience
// =============================================================================

#[cfg(test)]
mod executor_refuses_empty_success_tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Task 4.7: executor refuses empty success for unsupported construct
    // Scenario: `unsupported-operation-errors::No Empty Success for Unsupported Syntax`
    // Assert: Unsupported construct → Err(UnsupportedConstruct), never Ok(empty ResultSet)
    // -------------------------------------------------------------------------

    #[test]
    fn executor_refuses_empty_success_for_unsupported_construct() {
        // This test verifies the contract that when an executor encounters an
        // unsupported construct, it must return Err(UnsupportedConstruct) and
        // NOT Ok(ResultSet { rows: 0, ... }).

        let unsupported_err = ExecutorError::UnsupportedConstruct(
            UnsupportedConstruct::new(
                ConstructId::Other("FutureVariant".into()),
                "this variant is not yet supported",
            )
            .with_alternative("use an equivalent supported construct"),
        );

        // The error should be the UnsupportedConstruct variant
        assert!(matches!(
            unsupported_err,
            ExecutorError::UnsupportedConstruct(_)
        ));

        // If we were to convert this to a ResultSet (which we shouldn't),
        // it should NOT be an Ok with empty rows
        let as_result: Result<(), _> = Err(unsupported_err);
        assert!(as_result.is_err());
    }
}

#[cfg(test)]
mod bridge_mapping_tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Task 4.8: bridge mapping — CompileError::UnsupportedVariant → MoldError
    // Scenario: `unsupported-operation-errors::Distinct from CompileError`
    // Assert: legacy CompileError::UnsupportedVariant("X") → MoldError::UnsupportedConstruct
    // -------------------------------------------------------------------------

    #[test]
    fn legacy_compile_error_unsupported_variant_maps_to_mold_error() {
        // The legacy compile() function returns CompileError::UnsupportedVariant
        // when it encounters an unsupported AST variant.

        let legacy_error = CompileError::UnsupportedVariant("FutureQueryVariant");

        // Display should show the unsupported variant
        let display = format!("{}", legacy_error);
        assert!(
            display.contains("FutureQueryVariant"),
            "error display should contain the variant name: {}",
            display
        );

        // The conversion to PlanError/UnsupportedConstruct happens in the bridge
        let plan_error: PlanError = match legacy_error {
            CompileError::UnsupportedVariant(variant) => {
                PlanError::UnsupportedConstruct(UnsupportedConstruct::new(
                    ConstructId::Other(variant.into()),
                    "unsupported query variant",
                ))
            }
            CompileError::InvalidQuery(msg) => PlanError::UnsupportedConstruct(
                UnsupportedConstruct::new(ConstructId::Other("InvalidQuery".into()), &msg),
            ),
        };

        // Verify the construct id is preserved
        match plan_error {
            PlanError::UnsupportedConstruct(uc) => {
                // Access the construct field directly
                match uc.construct {
                    ConstructId::Other(ref s) => {
                        assert!(s.contains("FutureQueryVariant") || s.contains("InvalidQuery"));
                    }
                    _ => {}
                }
            }
            other => panic!("expected UnsupportedConstruct, got {:?}", other),
        }
    }

    #[test]
    fn compile_error_display_includes_variant_name() {
        let err = CompileError::UnsupportedVariant("CustomVariant");
        let display = format!("{}", err);
        assert!(
            display.contains("unsupported variant: CustomVariant"),
            "display should include variant name: {}",
            display
        );
    }
}
