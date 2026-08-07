# SDD Explore: explorer-graph-postgres-graphstore

## Executive Summary

PostgreSQL has a complete read-path (5 `Repository` trait methods over `symbols` + `call_edges`), but zero canonical write-path. SQLite has full write-path via `SqliteGraphStore::save_graph()`. The gap: no way to populate PostgreSQL with a `CallGraph`. This slice adds **async inherent methods** `save_call_graph(&CallGraph)` and `load_call_graph() -> Option<CallGraph>` on `PostgresRepository`, populating `symbols` and `call_edges` atomically. PostgreSQL becomes the canonical store. ~300 lines, OCP-compliant pure extension, unblocks explorer/MCP/petgraph slices.

**Status**: success

---

## 1. Current State Summary (Tied to Code Reality)

### What Prior Slices Delivered

| Slice | Key Artifacts | Status |
|-------|--------------|--------|
| `explorer-graph-foundation` | `Provenance` enum, `ConfidenceRules`, metadata on `CallGraph` edges, SQLite schema v2 | ARCHIVED ✅ |
| `explorer-graph-repository-bridge` | Standalone async `Repository` trait (2→5 methods), `MetadataAwareRepository` sub-trait, `cognicode-store-traits` deprecation | ARCHIVED ✅ |
| `explorer-graph-postgres-repository` | `PostgresRepository` struct, `PgPool`, `schema_postgres.sql` with `symbols` table, `sqlx` dep + `postgres` feature flag, `Repository` trait impl (2 methods) | ARCHIVED ✅ |
| `explorer-graph-postgres-call-edges` | `EdgeMetadata` value object, `call_edges` table in PG, `Repository` trait extension (+3 methods: `find_edges_by_caller`, `find_edges_by_callee`, `count_edges`), `insert_edge()` test helper, `FromStr` on `Provenance`/`DependencyType` | ARCHIVED ✅ |

### Code Reality — What Exists NOW

**`PostgresRepository`** (`crates/cognicode-core/src/infrastructure/persistence/postgres_repository.rs`, 836 lines):
- `PgPool` connection pool (max 8 connections)
- `run_migrations()` — idempotent, runs `schema_postgres.sql` via `include_str!`
- `Repository` trait impl: `find_symbol_by_qualified_name`, `count_symbols`, `find_edges_by_caller`, `find_edges_by_callee`, `count_edges`
- `insert_edge()` — `pub(crate)` inherent method for test seeding ONLY
- 21 contract tests (12 symbol + 9 edge) using per-test isolated databases
- Feature-gated: `#[cfg(feature = "postgres")]`

**`schema_postgres.sql`** (49 lines):
- `symbols` table: `id`, `file_path`, `name`, `kind`, `line`, `column`, `complexity` + 2 indexes
- `call_edges` table: `id`, `caller_id`, `caller_name`, `callee_id`, `callee_name`, `dependency_type`, `provenance`, `confidence` + 2 indexes

**`Repository` trait** (`crates/cognicode-core/src/domain/traits/repository.rs`, 210 lines):
- 5 async methods, `Send + Sync`, `#[async_trait]`, dyn-compatible
- Docstring explicitly says: "write-path (synchronous save/load of bincode blobs) and the read-path (async, query-shaped) remain independent seams"

**`GraphStore` trait** (`crates/cognicode-core/src/domain/traits/graph_store.rs`, 190 lines):
- **Synchronous** trait: `save_graph`, `load_graph`, `save_manifest`, `load_manifest`, `clear`, `exists`
- `SqliteGraphStore` implements it (in `cognicode-db`), PostgreSQL does NOT

**`SqliteGraphStore`** (`crates/cognicode-db/src/graph.rs`, 556 lines):
- `save_graph()`: serializes `CallGraph` as bincode blob → `call_graphs` table, THEN populates `symbols` and `call_edges` tables
- `populate_symbols()`: clears + inserts all symbols from `CallGraph`
- `populate_edges()`: clears + inserts all edges from `CallGraph`
- `load_graph()`: reads blob from `call_graphs` table, decodes via `VersionedBlob`

**Explorer bridge** (`crates/cognicode-explorer/src/adapters/call_graph_repository.rs`, 591 lines):
- `CallGraphRepository` wraps `Arc<CallGraph>` in-memory
- Implements `SymbolRepository` (sync, 9 methods) + `MetadataAwareRepository` (3 metadata methods)
- No PostgreSQL adapter exists — explorer is SQLite/in-memory only

### Critical Gap

```
PostgreSQL READ path:   ✅ symbols + call_edges (5 Repository methods)
PostgreSQL WRITE path:  ❌ NONE — only insert_edge() for test seeding

SQLite READ path:       ✅ via SymbolRepository + GraphStore::load_graph
SQLite WRITE path:      ✅ via SqliteGraphStore::save_graph (blob + tables)
```

**Without a PostgreSQL write-path:**
- Every PostgreSQL instance starts empty — no way to seed it with a real graph
- `save_call_graph(&CallGraph)` doesn't exist in the codebase for PostgreSQL
- Explorer can't switch to PostgreSQL (data never arrives)
- MCP envelope can't query real graph data from PostgreSQL
- petgraph projections can't be built from PostgreSQL (no data to project)

---

## 2. How Prior Slices Changed the Next Move

The `explorer-graph-postgres-call-edges` archive report explicitly lists the unblocked follow-on slices. Key architectural invariants:

1. **`GraphStore` is sync** (`rusqlite`). PostgreSQL is async (`sqlx`). The archive explicitly rejects "GraphStore impl for PostgreSQL." This slice must NOT implement `GraphStore`.

2. **`Repository` is read-only by design**. Adding write methods would mix concerns.

3. **The seam is additive**. This slice adds inherent methods — no trait changes, no existing code modified. Pattern mirrors `insert_edge()`.

4. **Column-for-column parity** — `symbols` and `call_edges` schemas are already compatible between PostgreSQL and SQLite.

5. **`CallGraph` is the canonical aggregate** — `SqliteGraphStore::populate_symbols`/`populate_edges` demonstrate the exact population pattern.

---

## 3. Candidate Next Slices Considered

### Slice A: PostgreSQL Canonical Write-Path — Inherent Methods (RECOMMENDED ✅)

- **Scope**: `save_call_graph(&CallGraph)` + `load_call_graph()` inherent methods on `PostgresRepository`. Transactional DELETE+INSERT. No new traits/tables.
- **Effort**: ~280 lines
- **OS**: 0.832 (EXCELENTE)
- **Pros**: Narrow, reviewable, OCP-compliant, unblocks everything

### Slice B: Async `GraphPersistence` Trait

- **OS**: 0.798 (EXCELENTE)
- **Verdict**: REJECTED — premature abstraction for one implementor

### Slice C: Explorer PostgreSQL Adapter

- **OS**: 0.535 (BUENO)
- **Verdict**: REJECTED — blocked on this slice (needs data first), sync/async mismatch

### Slice D: PostgreSQL `GraphStore` Impl (Sync on Async)

- **OS**: 0.280 (REGULAR)
- **Verdict**: HARD REJECTED — design-unsound, rejected in previous explore phases

---

## 4. Recommended Next Slice: Slice A

PostgreSQL Canonical Write-Path via inherent methods. Establishes PostgreSQL as canonical store, unblocks all downstream work, OCP-compliant pure extension.

### Scope

| # | Task | Risk |
|---|------|------|
| 1 | `save_call_graph(&CallGraph)` — transactional DELETE+INSERT into `symbols` + `call_edges` | Medium |
| 2 | `load_call_graph()` — SELECT + reconstruct `CallGraph` with provenance/confidence | Medium |
| 3 | Contract tests: round-trip, atomicity, idempotency, mixed provenance | Medium |
| 4 | Feature-gate behind `#[cfg(feature = "postgres")]` | Low |

**Total**: ~280 lines, 1 file modified.

---

## 5. Explicit Non-Goals

- ❌ `GraphStore` trait implementation
- ❌ New async trait (`GraphPersistence`)
- ❌ New `Repository` trait methods
- ❌ New PostgreSQL tables or DDL
- ❌ Blob/bincode storage in PostgreSQL
- ❌ Explorer PostgreSQL adapter
- ❌ MCP envelope wiring
- ❌ `ltree`/`pgvector` extensions
- ❌ `Component`/`Container`/`System` node kinds

---

## 6. Dependency Notes

**Inbound**: `explorer-graph-foundation` ✅, `explorer-graph-repository-bridge` ✅, `explorer-graph-postgres-repository` ✅, `explorer-graph-postgres-call-edges` ✅

**Outbound (unblocks)**:
- Explorer PostgreSQL Adapter
- MCP PostgreSQL Envelope (Phase 3)
- petgraph Projection from PostgreSQL
- CI/CD PostgreSQL Integration

---

## 7. Auto-Grill Results

**Preguntas**: 9 | **Auto-resueltas**: 7 (78%) | **Escaladas**: 2

### Auto-Resolved (7)
1. PostgreSQL es la tienda canónica ✅
2. No implementar `GraphStore` sync sobre async ❌
3. No crear trait async (inherente primero) ❌
4. No bridge explorer→PostgreSQL en este slice ❌
5. Sin nuevas tablas PostgreSQL ❌
6. Escritura transaccional ✅
7. Este slice desbloquea explorer-adapter, MCP-envelope, petgraph ✅

### Escalated (2)
- **E1**: Métodos inherentes (OS=0.832) vs trait async (OS=0.798) → **Recomendado: inherentes**
- **E2**: Solo tablas normalizadas (OS=0.820) vs blob+tablas (OS=0.650) → **Recomendado: solo normalizadas**

**Reporte**: `/tmp/sdd-explorer-graph-postgres-graphstore-auto-grill.html`  
**Status**: pending_validation

---

## 8. Entropy Analysis

**Method**: Heuristic (±1 bit confidence)

### Connascence Landscape

| Component A | Component B | Type | I(bits) | Severity |
|---|---|---|---|---|
| `save_call_graph` (new) | `CallGraph` (existing) | Type | 1.0 | ⚠️ Medium |
| `save_call_graph` (new) | `symbols` table (existing) | Algorithm | 1.58 | ⚠️ Medium |
| `save_call_graph` (new) | `call_edges` table (existing) | Algorithm | 1.58 | ⚠️ Medium |
| `save_call_graph` (new) | `SqliteGraphStore::populate_*` (existing) | Algorithm | 2.0 | ⚠️ Medium |
| `load_call_graph` (new) | `CallGraph` (existing) | Type | 0.5 | ✅ OK |
| `load_call_graph` (new) | `symbols` + `call_edges` (existing) | Name | 0.32 | ✅ OK |

**Critical pairs**: None. **Hidden connascence**: None. **SOLID violations**: None.

### Entropy Budget

| Metric | Estimate | Threshold | Status |
|--------|----------|-----------|--------|
| H(Δ_existing) | 0 bits (1 file, pure additive) | < 1.0 | ✅ |
| OCP compliant? | Yes | yes | ✅ |

**DQS**: ~0.65/1.0 (ACCEPTABLE — algorithm connascence with SQLite population pattern is expected and documented).

---

## Summary

| Field | Value |
|-------|-------|
| **Status** | success |
| **Next Recommended** | `sdd-propose` |
| **Risks** | Performance at scale (acceptable for MVP), row-level idempotency (simple DELETE+INSERT) |
| **Skill Resolution** | paths-injected — 4 skills |
