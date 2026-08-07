# Spec: postgres-callgraph-persistence

> Companion to engram `sdd/explorer-graph-postgres-graphstore/spec` and LogSeq `Spec: explorer-graph-postgres-graphstore`.

## Purpose

Closes PG's write-path gap. PG has a full read-path (5 `Repository` methods over `symbols` + `call_edges`) but no canonical write-path. This domain adds async inherent methods `save_call_graph(&CallGraph)` and `load_call_graph() -> Option<CallGraph>` on `PostgresRepository`, atomically populating `symbols` + `call_edges` in one `sqlx` transaction. Pure additive; SQLite / `GraphStore` / `Repository` untouched.

## Requirements

### Requirement: `save_call_graph` inherent write method

`PostgresRepository::save_call_graph(&self, graph: &CallGraph) -> Result<(), RepositoryError>` MUST be `pub async`, `#[cfg(feature = "postgres")]`-gated. Body runs in one `pool.begin()`: (1) `DELETE FROM call_edges` + `DELETE FROM symbols`, (2) `INSERT` every `graph.symbol_ids()` row binding `(file_path, name, kind.to_string(), line, column, complexity)`, (3) `INSERT` every `graph.edges_with_metadata()` row binding `(caller_id, caller_name, callee_id, callee_name, dependency_type.to_string(), provenance.to_string(), confidence)`, (4) `COMMIT` on success / `ROLLBACK` on any error. Returns `Ok(())` only after `COMMIT`; errors wrapped as `RepositoryError::Store("save_call_graph <step>: …")`.

#### Scenario: Happy path populates normalized tables

- GIVEN empty `symbols` + `call_edges` AND a `CallGraph` with 7 symbols, 12 edges spanning all 3 `Provenance` variants
- WHEN `save_call_graph(&g)` awaits
- THEN result is `Ok(())` AND `count_symbols()==7` AND `count_edges()==12` AND `find_edges_by_caller` per caller id returns the exact source set

#### Scenario: Delete-and-replace overwrites prior data

- GIVEN `CallGraph_A` (3 sym) already persisted
- WHEN `save_call_graph(&graph_b)` runs with 5 different symbols
- THEN `count_symbols()==5` AND `count_edges()==edge_count(B)` AND no row from A remains

#### Scenario: Idempotent re-save

- GIVEN a `CallGraph` saved once
- WHEN `save_call_graph(&same_graph)` runs again
- THEN counts equal post-first-save counts AND the row set is semantically equivalent (surr. `SERIAL` ids regenerate; semantic equality holds)

### Requirement: Transactional atomicity on partial failure

If any `INSERT` step fails, the wrapping transaction MUST be rolled back before returning `Err`. No row from the input `CallGraph` MAY remain partially persisted; prior rows are restored.

#### Scenario: Mid-INSERT failure leaves empty tables

- GIVEN empty tables AND a `CallGraph` containing one symbol that collides with a pre-seeded unique-index row
- WHEN `save_call_graph` awaits
- THEN result is `Err(RepositoryError::Store(_))` AND `count_symbols()==pre-seed count` AND `count_edges()==0`

#### Scenario: Rollback unwinds the DELETE phase

- GIVEN `CallGraph_A` (3 sym, 4 edges) already persisted
- WHEN `save_call_graph(&graph_b)` fails AFTER both `DELETE`s (test seam: rejected `kind`)
- THEN A's 3 symbols and 4 edges are still present post-failure

### Requirement: `load_call_graph` inherent read method

`PostgresRepository::load_call_graph(&self) -> Option<CallGraph>` MUST be `pub async`, `#[cfg(feature = "postgres")]`-gated, read-only. Returns `None` iff both tables are empty. Otherwise: `SELECT … FROM symbols ORDER BY id` → `add_symbol` per row via existing `SymbolRow::into_symbol`; then `SELECT … FROM call_edges ORDER BY id` → decode via existing `EdgeRow::into_edge`, then `add_dependency_with_provenance` with `ExtractionContext` mapped from `Provenance`: `Extracted→DirectExtraction`, `Inferred→Heuristic(confidence)`, `Ambiguous→Unresolved`.

#### Scenario: Empty DB returns None

- GIVEN empty tables
- WHEN `load_call_graph()` awaits
- THEN result is `None` AND no write DML is issued

#### Scenario: Populated DB returns Some with exact metadata

- GIVEN a 7 sym / 12 edge mixed-provenance `CallGraph` saved
- WHEN `load_call_graph()` awaits
- THEN result is `Some(g2)` AND `g2.symbol_count()==7` AND `g2.edge_count()==12` AND every edge's `(provenance, confidence)` matches source bit-for-bit (f64 exact) AND symbol FQNs match

#### Scenario: Mixed-provenance round trip preserves metadata

- GIVEN three edges `(Extracted,1.0)`, `(Inferred,0.7)`, `(Ambiguous,0.3)`
- WHEN round-tripped
- THEN `loaded.edges_with_metadata()` yields the same three pairs (order unspecified)

### Requirement: Semantic equivalence with in-memory `CallGraph`

Round trip `save_call_graph(G) → load_call_graph()` MUST produce `G'` that is `PartialEq`-equal to `G` (symbols, edges, per-edge metadata). Surr. `SERIAL` ids MUST NOT participate in equality.

#### Scenario: `assert_eq!` of source and round-tripped graph

- GIVEN a fixture with ≥5 symbols, ≥3 dep types, all 3 provenance variants, confidences `{0.0, 0.5, 1.0}`
- WHEN round-tripped
- THEN `assert_eq!(g, loaded)` passes AND counts match

#### Scenario: Self-loop and multi-edge same-pair are preserved

- GIVEN a self-loop (caller==callee) AND a multi-edge between the same pair with different `DependencyType`s
- WHEN round-tripped
- THEN both edges are present in `loaded` AND each carries the source `(provenance, confidence)`

### Requirement: Non-breaking behavior vs SQLite `GraphStore`

MUST NOT modify `SqliteGraphStore`, `GraphStore`, the sync write-path, or any pre-slice public API of `cognicode-core` (other than additive inherent methods). With `postgres` disabled, new methods MUST NOT be reachable.

#### Scenario: Default build still passes the pre-slice suite

- GIVEN pre-slice public API
- WHEN `cargo test --workspace` runs WITHOUT `--features postgres`
- THEN every pre-slice test passes AND `cargo doc --no-deps -p cognicode-core` exposes the same items AND `use …::PostgresRepository::save_call_graph;` fails to compile

#### Scenario: `SqliteGraphStore` and `GraphStore` untouched

- GIVEN pre-slice revisions of `cognicode-db/src/graph.rs` and `domain::traits::graph_store.rs`
- WHEN this slice lands
- THEN `git diff HEAD --` for both files is empty AND the sync write-path functions identically

### Requirement: Reused `postgres` feature flag (no new flag)

Reuses the existing `postgres` feature from prior slices. All new code (methods + tests) MUST be `#[cfg(feature = "postgres")]`-gated. No new feature flag is introduced.

#### Scenario: Default build stays sqlx-free

- GIVEN clean workspace
- WHEN `cargo check --workspace` runs WITHOUT `--features postgres`
- THEN build succeeds AND `sqlx` is absent from the dep graph AND new methods are unreachable

#### Scenario: Feature-enabled build exposes new methods

- GIVEN `--features postgres`
- WHEN `cargo check -p cognicode-core --features postgres --no-default-features` runs
- THEN both new methods are reachable AND the gated test module compiles

### Requirement: Testability — per-test isolation, contract coverage

Tests in `postgres_repository.rs` under `#[cfg(all(test, feature = "postgres"))]` MUST use `#[sqlx::test]` for per-test isolated DBs. Suite MUST cover: save happy path, load empty→`None`, load populated→`Some` with exact metadata, round-trip `assert_eq!`, mid-INSERT rollback, delete-and-replace, idempotent re-save, success contract.

#### Scenario: Per-test isolation

- GIVEN two parallel `#[sqlx::test]` functions with disjoint `CallGraph`s
- WHEN they run in parallel
- THEN each observes an isolated DB AND rows of one are not visible to the other AND both assertion sets pass

#### Scenario: No ignored contract tests

- GIVEN the save/load contract test module
- WHEN `cargo test -p cognicode-core --features postgres -- postgres_repository::tests::save_load` runs
- THEN every listed scenario has ≥1 passing test AND no test is `#[ignore]` without documented rationale

### Requirement: Rollback and rollout safety

Revertible with one `git revert`. No trait change, no schema change, no other file modified. Re-deploying MUST NOT alter schema or rows until `save_call_graph` is invoked.

#### Scenario: `git revert` restores pre-slice build

- GIVEN slice merged
- WHEN a single `git revert <merge-sha>` runs
- THEN `cargo check --workspace` (no `postgres` feature) succeeds AND the `postgres`-feature build does not regress vs pre-slice

#### Scenario: No schema drift in PG

- GIVEN a PG instance with pre-slice `symbols` + `call_edges` populated
- WHEN `run_migrations()` re-runs after redeploy
- THEN schema is byte-identical (no `ALTER TABLE`, no new tables/indexes) AND existing rows preserved

#### Scenario: PR size budget

- GIVEN the planned changes (1 file, additive inherent methods + contract tests)
- WHEN the diff is computed
- THEN `additions + deletions` ≤ 400 AND `400-line-budget-risk` is Low

## Status

Draft. Awaiting `sdd-design`.

## Coverage

Happy paths: covered. Edge cases: covered (empty, self-loop, multi-edge, f64 boundaries). Error states: covered (mid-INSERT rollback, DELETE-phase rollback, success contract). Non-breaking: covered.

## Out of Scope (locked)

`GraphStore` impl for PG; new async `GraphPersistence` trait; new `Repository` methods; new tables/columns/indexes in PG; bincode/blob sidecar; explorer-PG adapter; MCP envelope; petgraph projection; `ltree`/`pgvector`; `Component`/`Container`/`System` kinds; removal of SQLite / `cognicode-store-traits` / `GraphStore`.
