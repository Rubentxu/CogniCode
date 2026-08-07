# Proposal: PostgreSQL Canonical Write-Path for CallGraph

**Slice**: Write-path only — `save_call_graph` + `load_call_graph` inherent methods.
**Parent change**: `explorer-graph-postgres-graphstore`

## Intent

PostgreSQL has a complete read-path (5 `Repository` trait methods over `symbols` + `call_edges`) but zero write-path. SQLite owns the canonical data via `SqliteGraphStore::save_graph`. Without a PG write-path: every PG instance starts empty, explorer/MCP/petgraph slices are blocked, and SQLite remains the de-facto canonical store — contradicting the roadmap's explicit PostgreSQL-as-truth design.

## Scope

### In Scope
- `pub async fn save_call_graph(&self, graph: &CallGraph)` — transactional DELETE+INSERT into `symbols` and `call_edges`
- `pub async fn load_call_graph(&self) -> Option<CallGraph>` — SELECT reconstruction from normalized tables
- Contract tests: round-trip, empty-graph, transaction atomicity, mixed-provenance edges
- Feature-gated behind `#[cfg(feature = "postgres")]`

### Out of Scope
- `GraphStore` trait impl — sync trait on async pool, architecturally rejected
- New async trait (`GraphPersistence`) — premature abstraction for one implementor
- Blob/bincode sidecar in PG — normalized tables are the canonical truth
- `Repository` trait changes — read-path remains read-only
- Explorer adapter, MCP envelope, petgraph projection — unblocked but separate slices

## Capabilities

### New Capabilities
- `postgres-callgraph-persistence`: async save/load of full `CallGraph` to/from PostgreSQL normalized tables with transactional atomicity and semantic round-trip fidelity

### Modified Capabilities
- None — pure additive extension; zero existing code modified

## Approach

**Strategy**: Inherent methods on `PostgresRepository` (mirrors existing `insert_edge()` pattern).

**`save_call_graph`**: DELETE FROM `call_edges` + DELETE FROM `symbols`, then INSERT all symbols via `symbol_ids()` + all edges via `edges_with_metadata()` — wrapped in a single `sqlx` transaction.

**`load_call_graph`**: SELECT * FROM `symbols` + SELECT * FROM `call_edges`, reconstruct `CallGraph` via `add_symbol` + `add_dependency_with_provenance`. Semantically equivalent to bincode round-trip, verified by test.

**Design stance**: inherent methods > trait (no second backend yet). Normalized tables > blob (queryability, schema parity with SQLite). Delete-and-replace > upsert (simpler, correct, no row-level merge complexity).

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/infrastructure/persistence/postgres_repository.rs` | Modified | Add `save_call_graph` + `load_call_graph` + tests (~280 lines) |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `load_call_graph` perf at scale (>10k symbols) | Low | SELECT-based reconstruction is fast for MVP; materialized view if bottleneck |
| Transaction partial failure | Low | `sqlx` BEGIN/COMMIT ensures atomicity |
| Semantic drift from SQLite populate logic | Low | Column-for-column parity; round-trip test verifies equivalence |

## Rollback Plan

Revert `postgres_repository.rs` to prior revision. Methods are additive — no trait changes, no schema changes, no other files touched. `git revert` is sufficient and clean.

## Dependencies

- **Inbound**: `explorer-graph-postgres-call-edges` (ARCHIVED) — `call_edges` table + `EdgeMetadata` + `insert_edge()` pattern
- **Outbound unblocks**: explorer PG adapter, MCP envelope, petgraph projection

## Success Criteria

- [ ] `save_call_graph` → `load_call_graph` round-trip preserves symbol count, edge count, and all 7 edge columns
- [ ] Transaction rolls back fully on partial INSERT failure
- [ ] `cargo test --features postgres` passes all contract tests
- [ ] Zero existing code modified (OCP compliance)
