# Delta Spec: postgres-call-edges (explorer-graph-postgres-call-edges)

## Purpose

Close the SQLite↔PostgreSQL parity gap: SQLite has `call_edges` with
`(Provenance, confidence)`, PG has only `symbols`. This slice adds
the `call_edges` table, `EdgeMetadata` value object, three
`Repository` trait methods, `PostgresRepository` impl, and a
`pub(crate)` write helper. Pure extension.

## ADDED Requirements

### Requirement: Canonical `call_edges` schema

`schema_postgres.sql` MUST add `call_edges(id SERIAL PK, caller_id TEXT
NOT NULL, caller_name TEXT NOT NULL, callee_id TEXT NOT NULL, callee_name
TEXT NOT NULL, dependency_type TEXT NOT NULL, provenance TEXT NOT NULL
DEFAULT 'Extracted', confidence REAL NOT NULL DEFAULT 1.0)` with indexes
on `caller_id` and `callee_id`. All DDL uses `IF NOT EXISTS`. Column
name is `dependency_type` (NOT `dep_type`) — SQLite v2 parity.

#### Scenario: Idempotent migration + column-set parity
- GIVEN fresh PG
- WHEN `run_migrations()` runs twice
- THEN both succeed and `information_schema.columns` lists exactly
  the 8 columns in order.

### Requirement: `EdgeMetadata` value object

`EdgeMetadata` (7 fields: `caller_id, caller_name, callee_id,
callee_name, dependency_type, provenance, confidence`) in
`cognicode-core::domain::value_objects::edge_metadata`, re-exported.
Derives `Debug, Clone, PartialEq`. No `Serialize`. Ungated.

### Requirement: Three edge-query methods on `Repository`

The async `Repository` trait MUST gain
`find_edges_by_caller(&str) -> Vec<EdgeMetadata>`,
`find_edges_by_callee(&str) -> Vec<EdgeMetadata>`, and
`count_edges() -> usize` (trait grows 2→5). Stays `Send + Sync` and
`#[async_trait]`-compatible. In-test stubs MUST return `vec![]` / `0`.

#### Scenario: Caller lookup returns seeded edges in order
- GIVEN 3 edges with `caller_id="foo"`, varying callees
- WHEN `find_edges_by_caller("foo")`
- THEN result has 3 items, in insertion order, with `caller_id=="foo"`
  and seeded metadata intact.

#### Scenario: Empty result is `Ok(vec![])`, not error
- GIVEN empty `call_edges`
- WHEN `find_edges_by_callee("nothing")`
- THEN result is `Ok(Vec::new())`.

#### Scenario: Count tracks inserts
- GIVEN empty → 5 inserted
- WHEN `count_edges()` runs at each stage
- THEN it returns 0 then 5.

### Requirement: `PostgresRepository` edge implementation

`PostgresRepository` MUST implement the three methods via
`sqlx::query_as` over a private `#[derive(sqlx::FromRow)] EdgeRow`.
Unparseable `provenance` → `Provenance::Extracted`; unparseable
`dependency_type` → `DependencyType::Calls`. `#[cfg(feature =
"postgres")]`-gated.

#### Scenario: Query uses indexed predicate
- GIVEN 50 callers × 2 edges
- WHEN `find_edges_by_caller("caller_42")` runs
- THEN SQL contains `WHERE caller_id = $1` AND returns 2 edges.

### Requirement: `insert_edge()` test-seeding write helper

`PostgresRepository::insert_edge(&self, edge: &EdgeMetadata)` MUST be a
`pub(crate)` inherent method, NOT on the trait. Test seeding only. No
bulk / upsert variant.

#### Scenario: Insert round-trip + API visibility
- GIVEN empty `call_edges` and `EdgeMetadata{caller_id="a",
  callee_id="b", dep=Calls, prov=Inferred, conf=0.7}`
- WHEN `insert_edge(&edge)` then `find_edges_by_caller("a")`
- THEN returned Vec has one element equal to the inserted one.
- AND WHEN a downstream crate writes
  `use ...::PostgresRepository::insert_edge;`
- THEN the build fails (method not visible outside the crate).

### Requirement: Feature flag — non-breaking default build

This slice MUST reuse the existing `postgres` feature. NO new feature.
Gating: PG impl, write helper, DDL, tests → `#[cfg(feature =
"postgres")]`; `EdgeMetadata` → ungated; trait extension → ungated.

#### Scenario: Default build is sqlx-free + struct reachable
- GIVEN a clean workspace
- WHEN `cargo check --workspace` runs (no `--features postgres`)
- THEN `sqlx` is NOT in the dep graph AND
  `cognicode_core::domain::value_objects::EdgeMetadata` is reachable
  AND every pre-slice public item of `cognicode-core` is unchanged.

### Requirement: Testability — per-test isolated contract tests

`pg_test!` (existing) MUST be used. Coverage: caller/callee lookup,
empty result, count, insert round-trip, column-set parity.

#### Scenario: Per-test isolation holds for edge methods
- GIVEN two `pg_test!` functions, one inserting edges, one asserting
  `count_edges()==0`
- WHEN both run in the same process
- THEN the second sees `count_edges()==0`.

### Requirement: Rollout safety — additive, reversible, idempotent

`Repository` trait extension is additive. `run_migrations()` on a
populated DB MUST be a no-op preserving every row.

#### Scenario: Re-migrate preserves rows
- GIVEN 3 edges inserted
- WHEN `run_migrations()` runs again
- THEN it succeeds AND `count_edges()==3` AND every edge round-trips.

## Out of Scope (asserted)

`save_call_graph`; `GraphStore` for PG; explorer/MCP/petgraph; `ltree`,
`pgvector`, batch insert, upsert; new public re-exports of
`insert_edge`; new workspace deps; `cognicode-store-traits` removal.
`git revert` of this slice MUST restore the green workspace without
data-loss concerns.
