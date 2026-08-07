# graph-repository-write Specification (NEW)

## Purpose

`GraphRepository` (`crates/cognicode-explorer/src/ports/graph_repository.rs`) is currently a read-only port. This change adds two write methods — `upsert_nodes` and `upsert_edges` — to fix a shared gap that blocks `docs_ingest` persistence AND blocks the new `issues_ingest` pipeline. Both methods MUST be idempotent (a re-ingest with the same id produces no duplicate rows) and gated behind `#[cfg(feature = "multimodal")]`. The PG adapter implements them with `INSERT … ON CONFLICT (id) DO UPDATE`; the in-memory test mock implements them with `HashMap::insert`. The contract is **port-first** — no method may assume a Postgres-only feature like `pg_trgm` or full-text search.

## Requirements

### Requirement: upsert_nodes on the port

`GraphRepository` MUST expose:

```rust
fn upsert_nodes(&self, nodes: Vec<GraphNode>) -> ExplorerResult<usize>;
```

The method MUST insert every node whose `(id, kind)` is not present, and update every node whose `(id, kind)` is already present (the `properties` map is replaced, `updated_at` is set to the new value, `label` and `source_path` are replaced verbatim). It MUST return the number of rows actually inserted (a row that was updated does NOT count as inserted). It MUST be transactional: either every node in the batch is upserted, or none are (PG: `BEGIN; … COMMIT;` around the whole batch).

#### Scenario: First-time insert returns row count
- GIVEN an empty `graph_nodes` table
- WHEN `upsert_nodes(vec![node_a, node_b, node_c])` is called
- THEN the result is `Ok(3)`
- AND `graph_nodes` now has 3 rows

#### Scenario: Re-ingest is idempotent
- GIVEN `graph_nodes` already contains `node_a` with the same `id`
- WHEN `upsert_nodes(vec![node_a_modified])` is called
- THEN the result is `Ok(0)` (no insert)
- AND the row's `properties`, `label`, `updated_at` reflect the new payload

#### Scenario: Batch failure is all-or-nothing
- GIVEN a batch of 100 nodes where node #47 has an invalid `kind` string
- WHEN `upsert_nodes` is called
- THEN the result is `Err(…)`
- AND the PG transaction is rolled back (no partial rows)

### Requirement: upsert_edges on the port

`GraphRepository` MUST expose:

```rust
fn upsert_edges(&self, edges: Vec<GraphEdge>) -> ExplorerResult<usize>;
```

The natural key is `(source, target, kind)` — re-ingesting the same triple updates the `confidence`, `provenance`, and `metadata` in place. Edge `id` is NOT a primary key (edges don't carry one). The method MUST validate the `GraphEdge` invariants before the batch hits the database (rejects `confidence` out of `[0.0, 1.0]`, self-loops, `NaN`).

#### Scenario: First-time edge insert
- GIVEN an empty `graph_edges` table
- WHEN `upsert_edges(vec![edge_a])` is called
- THEN the result is `Ok(1)`

#### Scenario: Same edge re-ingested updates in place
- GIVEN `graph_edges` contains `(src, dst, Cites)` with `confidence = 0.7`
- WHEN `upsert_edges(vec![(src, dst, Cites, 0.9, Extracted)])` is called
- THEN the result is `Ok(0)` (no new row)
- AND the row's `confidence` is now `0.9`

#### Scenario: Self-loop is rejected pre-DB
- GIVEN an edge with `source == target`
- WHEN `upsert_edges(vec![self_loop_edge])` is called
- THEN the result is `Err(ExplorerError::InvalidInput("self-loops are not allowed"))`
- AND the database is untouched

### Requirement: PG adapter implementation

The PG implementation MUST use `INSERT … ON CONFLICT (id) DO UPDATE SET …` (nodes) and `INSERT … ON CONFLICT (source, target, kind) DO UPDATE SET …` (edges). The PG SQL MUST be parameterised (no string interpolation of user data) and use a single transaction per batch. The migration MUST add unique constraints on `(id, kind)` for `graph_nodes` and on `(source, target, kind)` for `graph_edges` if not already present.

#### Scenario: PG ON CONFLICT fires
- GIVEN the `graph_nodes` migration includes `UNIQUE (id, kind)`
- WHEN `upsert_nodes` is called with a duplicate id
- THEN PG returns the row count for the `UPDATE` branch (0 in our model, since we count only inserts)

### Requirement: In-memory mock for tests

An in-memory `HashMap<NodeId, GraphNode>` + `HashMap<(NodeId, NodeId, EdgeKind), GraphEdge>` mock MUST be added to the test module so unit tests can exercise the write path without a Postgres dependency. The mock MUST satisfy the same trait (`GraphRepository: Send + Sync`) and live behind a `#[cfg(test)]` gate.

#### Scenario: Mock idempotency matches PG
- GIVEN the in-memory mock
- WHEN the same node is upserted twice
- THEN the second call returns `Ok(0)`

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| Empty `Vec` input | `upsert_nodes(vec![])` returns `Ok(0)`, no SQL emitted |
| Node `id` is the empty string | Reject with `ExplorerError::InvalidInput` before any DB I/O |
| Edge with `confidence = NaN` | `GraphEdge::new` already returns an error; `upsert_edges` propagates it as `ExplorerError::InvalidInput` |
| Node `kind` is `Symbol(SymbolKind::External)` (legacy 22-variant) | Persist verbatim; the `(id, kind)` key still works for legacy rows |
| Two nodes share the same `id` but different `kind` (a `Doc` and a `Symbol` both with id `auth`) | Both rows persist (the PK is `(id, kind)`, not `id` alone) |
| Concurrent upserts of the same node from two CLI invocations | Both call PG; `ON CONFLICT DO UPDATE` serialises them; final state is whichever committed last (acceptable for V1) |
| `source_path` on a node is a Windows path with `C:` | Persist verbatim (no `:` mangling) |
| `properties` map contains a non-UTF-8 string | Reject with `ExplorerError::InvalidInput` (PG `text` columns require UTF-8) |

## Out of Scope

- Bulk-load path (CSV / COPY) — V1 uses `INSERT` per row
- `DELETE` operations (no caller needs them yet)
- Soft-delete / tombstone rows
- Cross-table transactions (node + edge upserts are two separate calls; the caller sequences them)
- Optimistic concurrency tokens (the `updated_at` timestamp is the only conflict signal)

## TDD RED Gate

1. Port trait extension — 2 new methods, `Send + Sync` preserved, dyn-compat preserved
2. PG mock (in-memory) tests — 6 cases: insert, re-ingest, all-or-nothing batch, self-loop, empty vec, duplicate `(id, kind)` rejected
3. PG migration test — 1 case: `UNIQUE (id, kind)` constraint exists post-migration
4. `docs_ingest` regression — re-running `docs_ingest` after the new write path is wired reports `nodes_created == 0` for unchanged files
5. `issues_ingest` smoke — 1 case: ingesting 3 fake issues produces 3 `Issue` rows + N `Resolves` edges

## Dependencies

- `generic-graph-model` (provides `GraphNode`, `GraphEdge`, `NodeId`, `NodeKind`)
- `docs-source-adapter` (the first caller; was previously no-op after parse)
- `mcp-multimodal-tools` (existing `graph_search` reads the same tables; no schema change to the read path)
- `multimodal` Cargo feature (gates every new method)

## Risks

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Migration adds `UNIQUE` on legacy data with duplicate rows | Medium | Pre-migration check + a one-shot dedup script in the migration directory |
| `INSERT … ON CONFLICT` semantics differ between PG and SQLite | High (if SQLite path is ever enabled) | Adapter pattern keeps PG and (future) SQLite impls independent; the trait contract is the single source of truth |
| Long-running batch blocks other writers | Low | Default batch size cap (1000 rows per call); the caller splits larger payloads |
