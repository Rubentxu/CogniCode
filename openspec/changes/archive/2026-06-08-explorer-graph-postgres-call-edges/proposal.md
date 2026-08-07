# Proposal: PostgreSQL `call_edges` — Read-Path + Minimal Write Helper

## Intent

PostgreSQL backend persists `symbols` but has zero `call_edges`. SQLite backend has
full provenance+confidence on edges. Gap: queries like "who calls this symbol" are
impossible against PostgreSQL. This slice closes that gap with the read-path and
a minimal test-seeding write helper.

## Why Now

- `explorer-graph-postgres-repository` is **archived & verified** — pool + symbols table ready
- No other work depends on us — this slice **unblocks** PostgreSQL GraphStore, explorer bridge,
  MCP envelope (all Phase 2+)
- ~200-300 Δlines, within review budget

## Scope

### In Scope

| Item | Detail |
|------|--------|
| `call_edges` table | Column-for-column parity with SQLite `call_edges` v2: `caller_id`, `callee_id`, `caller_name`, `callee_name`, `dep_type`, `provenance`, `confidence` |
| `EdgeMetadata` value object | New struct in `cognicode-core::domain::value_objects` — 5 fields, `Debug + Clone` |
| `Repository` trait: 3 new methods | `find_edges_by_caller`, `find_edges_by_callee`, `count_edges` → `Vec<EdgeMetadata>` |
| `PostgresRepository` edge queries | `sqlx::query_as` with `FromRow` impl on `EdgeMetadata` |
| `schema_postgres.sql` extension | Add `call_edges` DDL + indexes on `caller_id`, `callee_id` |
| `insert_edge()` write helper | `pub(crate)` on `PostgresRepository` — seeds test data; NOT on `Repository` trait |
| Contract tests | Per-test isolated PG databases; edge roundtrip, metadata preservation, empty-state queries |

### Out of Scope

- Full `save_call_graph(&CallGraph)` write-path (separate slice)
- `GraphStore` impl for PostgreSQL (sync trait on async pool, rejected in explore)
- Explorer-to-Postgres adapter / MCP wiring / `petgraph` projection
- Batch/bulk insert optimization / `ltree` / `pgvector`

## Capabilities

### New Capabilities

- `postgres-call-edges`: PostgreSQL `call_edges` table + `Repository` trait extension (3 edge query methods) + `EdgeMetadata` value object + `PostgresRepository` query impl + minimal `insert_edge()` write helper

### Modified Capabilities

- `postgres-symbol-repository`: same crate, same struct, same feature gate — additive extension (table count 1→2, schema DDL grows, no existing code touched)

## Approach

**Schema**: raw SQL via `include_str!` (same pattern as `symbols`). Table named `call_edges`
for SQLite column-parity — NOT `graph_edges` (auto-grill OS=0.78 confirms). Indexes on
`caller_id` and `callee_id`; `provenance` index deferred.

**Trait extension**: 3 methods directly on `Repository` (not a sub-trait). Docstring in
`repository.rs:9` explicitly anticipates "typed query methods in a follow-up slice."
5 total methods — well under the 10-method split threshold.

**Return type**: `Vec<EdgeMetadata>`, not `Box<dyn Iterator>`. Simple, `async_trait`-compatible,
no lifetimes. Switch to `Stream` when edge counts exceed 10K per query.

**Write helper**: `PostgresRepository::insert_edge()` — `pub(crate)`, NOT on the trait.
Tests seed via `#[cfg(test)]` re-export. Not a public API commitment.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/domain/value_objects/edge_metadata.rs` | **New** | `EdgeMetadata` struct |
| `crates/cognicode-core/src/domain/traits/repository.rs` | Modified | 3 new async methods |
| `crates/cognicode-core/src/infrastructure/persistence/schema_postgres.sql` | Modified | `call_edges` DDL + indexes |
| `crates/cognicode-core/src/infrastructure/persistence/postgres_repository.rs` | Modified | Edge query impl + `insert_edge` + tests |

## Entropy Budget

| Metric | Estimate | Threshold | Status |
|--------|---------|-----------|--------|
| H(Δ_existing) — files modified | log2(3) ≈ 1.58 | < 1.0 | ⚠️ AMBER (inevitable — trait extension touches trait file) |
| H(Δ_new) — new files | log2(1) ≈ 0.0 | > 0 | ✅ |
| New connascence pairs | 4 (EdgeMetadata↔schema, EdgeMetadata↔trait, PostgresRepository↔SqliteGraphStore, schema_postgres↔schema.rs) | < 3 | ⚠️ AMBER |
| OCP compliant? | Yes — trait extended, no existing method signatures changed | yes | ✅ |
| **DQS (pre-slice)** | **0.78** | > 0.70 | ✅ EXCELLENT |

## Risks

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Schema drift between SQLite/PG `call_edges` | Medium | Column-for-column parity enforced; contract test comparing `PRAGMA table_info` vs `information_schema.columns` |
| `Repository` trait grows too large | Low | 5 methods total — far from 10-method split threshold |
| `insert_edge()` becomes de-facto public API | Low | `pub(crate)`; tests gate behind `#[cfg(test)]` |
| PostgreSQL test infra requirement | Medium | Same `TEST_DATABASE_URL` pattern as prior slice — already proven in CI |

## Rollback Plan

Revert the merge commit. `Repository` trait method additions are purely additive — no
existing callers affected. `schema_postgres.sql` DDL uses `IF NOT EXISTS` — dropping
the table is `DROP TABLE IF EXISTS call_edges`. No data migration to unwind.

## Dependencies

- `explorer-graph-postgres-repository`: **Archived ✅** (provides `PgPool`, migration pattern)
- `edge-provenance`: **Live ✅** (provides `Provenance` enum, confidence semantics)
- `repository-trait-bridge`: **Live ✅** (provides async `Repository` trait seam)
- External: PostgreSQL 14+ for tests (same as prior slice)

## Success Criteria

- [ ] `cargo check --workspace` passes (default build sqlx-free)
- [ ] `cargo check --features postgres -p cognicode-core` passes
- [ ] `schema_postgres.sql` idempotent for `call_edges` table
- [ ] `find_edges_by_caller("a.rs:foo:1")` returns seeded edges with correct `Provenance` + `confidence`
- [ ] `find_edges_by_callee` returns empty `Vec` when no edges point to symbol
- [ ] `count_edges()` returns 0 on fresh DB, N after seeding N edges
- [ ] Zero regression in 295+ existing tests
- [ ] `cargo doc --no-deps` produces no new warnings

## Open Questions

1. **`dep_type` column name**: Auto-grill exposes the question — the SQLite schema uses
   `dependency_type` (8 chars), but the exploration shorthand calls it `dep_type` (7 chars).
   Which name? **Recommendation**: `dependency_type` (parity with SQLite schema v2 column name,
   not a shortening). Will resolve in specs phase.

2. **None.** Both escalated decisions resolved:
   - Table name: `call_edges` (OS=0.78, confirmed)
   - Return type: `Vec<EdgeMetadata>` (OS=0.65, confirmed)
   - Write helper visibility: `pub(crate)` (confirmed)
