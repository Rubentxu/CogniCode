# SDD Explore: explorer-graph-postgres-call-edges

**Change**: `explorer-graph-postgres-call-edges`
**Project**: cognicode
**Mode**: automatic, hybrid (openspec + Engram + LogSeq)
**Date**: 2026-06-08

---

## Executive Summary

The PostgreSQL backend (`PostgresRepository`) currently persists `symbols` but has ZERO
`call_edges` persistence. The SQLite backend (`SqliteGraphStore`) has both tables with full
provenance + confidence columns (schema v2). The next dependency-ready slice must add
`call_edges` persistence to PostgreSQL — specifically, the **read-path** (edge query methods
on the `Repository` trait and their PostgreSQL implementation) plus the **minimal write-path**
needed to populate the table from a `CallGraph`. This closes the PostgreSQL parity gap with
SQLite and unblocks query-shaped graph traversal from PostgreSQL.

The slice is **small, reviewable (~200–350 ∆lines)**, and depends only on the completed
`explorer-graph-postgres-repository` slice.

---

## 1. Current State (Tied to Code Reality)

### 1.1 What Exists

| Layer | Artifact | Status |
|-------|----------|--------|
| Domain model | `CallGraph` (in-memory) with `edges_with_metadata()` → `(SymbolId, SymbolId, DependencyType, Provenance, f64)` | ✅ Complete (`crates/cognicode-core/src/domain/aggregates/call_graph.rs:246-264`) |
| Domain traits | `Repository` (async) — `find_symbol_by_qualified_name`, `count_symbols` | ✅ Complete (`crates/cognicode-core/src/domain/traits/repository.rs:45`) |
| Domain traits | `GraphStore` (sync) — `save_graph`, `load_graph`, `save_manifest`, `load_manifest`, `clear`, `exists` | ✅ Complete (`crates/cognicode-core/src/domain/traits/graph_store.rs:23`) |
| SQLite backend | `SqliteGraphStore` — saves bincode blob AND populates `symbols` + `call_edges` tables with provenance + confidence | ✅ Complete (`crates/cognicode-db/src/graph.rs:105-241`) |
| SQLite schema | `call_edges` table with `provenance TEXT`, `confidence REAL` columns (v2) | ✅ Complete (`crates/cognicode-db/src/schema.rs:85-94`) |
| PostgreSQL backend | `PostgresRepository` — implements `Repository`, owns `PgPool`, runs migrations | ✅ Complete (`crates/cognicode-core/src/infrastructure/persistence/postgres_repository.rs:41-91`) |
| PostgreSQL schema | **ONLY `symbols` table — NO `call_edges`, NO `call_graphs`** | ❌ Gap (`crates/cognicode-core/src/infrastructure/persistence/schema_postgres.sql:12-20`) |
| sqlx dependency | `sqlx = { features = ["runtime-tokio", "postgres", "macros"] }` | ✅ In workspace (`Cargo.toml:126`) |
| `postgres` feature flag | Gates `PostgresRepository` behind `#[cfg(feature = "postgres")]` | ✅ (`cognicode-core/Cargo.toml:15`) |
| Explorer ports | `SymbolRepository` + `MetadataAwareRepository` (explorer-local traits, NOT the core `Repository`) | ✅ Complete (`crates/cognicode-explorer/src/ports/`) |
| Explorer adapter | `CallGraphRepository` — bridges `CallGraph` (domain) → `SymbolRepository` (explorer) | ✅ Complete (`crates/cognicode-explorer/src/adapters/call_graph_repository.rs`) |

### 1.2 The Gap (Precise)

The PostgreSQL backend has a **half-implemented graph**: `symbols` exist but `call_edges` do not.
This means:

- `find_symbol_by_qualified_name` works in PostgreSQL, but you cannot query "who calls this symbol"
- The SQLite backend (`SqliteGraphStore`) CAN answer edge queries because it has `call_edges` populated
- The in-memory `CallGraph` CAN answer edge queries (it owns the full graph), but it has no PostgreSQL equivalent

**The gap is the `call_edges` table in PostgreSQL and the edge query methods on the `Repository` trait.**

### 1.3 PostgreSQL Schema (Current vs. Needed)

```sql
-- CURRENT (schema_postgres.sql)
CREATE TABLE IF NOT EXISTS symbols (
    id          SERIAL PRIMARY KEY,
    file_path   TEXT NOT NULL,
    name        TEXT NOT NULL,
    kind        TEXT,
    line        INTEGER,
    column      INTEGER,
    complexity  INTEGER
);

-- NEEDED: call_edges table (mirrors SQLite schema v2)
CREATE TABLE IF NOT EXISTS call_edges (
    id               SERIAL PRIMARY KEY,
    caller_id        TEXT NOT NULL,
    caller_name      TEXT NOT NULL,
    callee_id        TEXT NOT NULL,
    callee_name      TEXT NOT NULL,
    dependency_type  TEXT NOT NULL,
    provenance       TEXT NOT NULL DEFAULT 'Extracted',
    confidence       REAL NOT NULL DEFAULT 1.0
);
```

The SQLite and PostgreSQL schemas should be column-for-column compatible for `call_edges`
(just as they are for `symbols` — see `schema_postgres.sql` lines 8-10).

---

## 2. How Prior Slices Changed the Next Move

| Slice | What it delivered | How it shapes this slice |
|-------|-------------------|--------------------------|
| `explorer-graph-foundation` | Provenance + confidence on every `CallGraph` edge, `ConfidenceRules`, `ExtractionContext` | Edge metadata is authoritative in the domain — PostgreSQL must store it. The column shape (`provenance TEXT`, `confidence REAL`) is already proven in SQLite. |
| `explorer-graph-repository-bridge` | Standalone async `Repository` trait (NOT inheriting `GraphStore`), separation of read/write seams | The `Repository` trait is the seam to extend — NOT `GraphStore`. This slice adds edge query methods to `Repository`. |
| `explorer-graph-postgres-repository` | `PostgresRepository` with `PgPool`, migrations via `include_str!`, `symbols` table, `find_symbol_by_qualified_name` | Establishes the pattern: raw SQL + `include_str!`, `sqlx::query_as` with `FromRow`, idempotent `IF NOT EXISTS`. This slice follows the same patterns for `call_edges`. |
| SQLite `call_edges` (in `SqliteGraphStore`) | `populate_edges()` that iterates `edges_with_metadata()` and inserts rows with all 7 columns | Reference implementation for the PostgreSQL write-path. The destructuring is identical. |

**The chain is clear:** the Postgres backend now has a `symbols` table and a connection pool. The next slice adds the second table (`call_edges`) and the query methods to read from it.

---

## 3. Candidate Next Slices Considered

### Candidate A: PostgreSQL call_edges table + Repository edge queries (READ-PATH ONLY)

**Scope:** Add `call_edges` table to `schema_postgres.sql`. Add 3-4 edge query methods to the `Repository` trait (`find_edges_by_caller`, `find_edges_by_callee`, `find_edges_between`). Implement them in `PostgresRepository` using parameterized queries. Add contract tests.

| Aspect | Assessment |
|--------|-----------|
| Lines | ~150-250 ∆ (SQL ~30, trait ~40, impl ~80, tests ~90) |
| Dependencies | Only `explorer-graph-postgres-repository` |
| Risk | Low — follows existing patterns |
| Completeness | Partial — read-path only; edges must be populated via a separate write-path slice |

**Pros:** Minimal, reviewable, unblocks query-side consumers.
**Cons:** No write-path — edges remain empty until a future slice populates them. Tests must seed data manually.

### Candidate B: PostgreSQL call_edges table + full write-path (save CallGraph → both tables)

**Scope:** Add `call_edges` table. Add edge query methods to `Repository`. Add a `save_call_graph(&CallGraph)` method to `PostgresRepository` (or implement `GraphStore`). Populate both `symbols` and `call_edges` from a `CallGraph` aggregate.

| Aspect | Assessment |
|--------|-----------|
| Lines | ~350-500 ∆ |
| Dependencies | `explorer-graph-postgres-repository` |
| Risk | Medium — write-path involves transactions, dedup, error handling |
| Completeness | Full — you can both write and read edges |

**Pros:** Complete cycle (write + read). Mirrors the `SqliteGraphStore` pattern.
**Cons:** Larger scope. The `GraphStore` trait is sync but PostgreSQL wants async — requires `spawn_blocking` wrapping. Couples the write-path design to the read-path before the read API is stable.

### Candidate C: Repository trait extension only (no PostgreSQL implementation)

**Scope:** Add edge query methods to the `Repository` trait. Add a test-only `InMemoryEdgeRepository` implementation. Leave PostgreSQL for a follow-up.

| Aspect | Assessment |
|--------|-----------|
| Lines | ~80-120 ∆ |
| Dependencies | None beyond current |
| Risk | Very low |
| Completeness | Interface only — no real backend |

**Pros:** Establishes the contract early.
**Cons:** Too small to be a meaningful slice. Rejected — this should be done INLINE with the PostgreSQL implementation, not as a separate slice.

### Candidate D: Full `GraphStore` implementation for PostgreSQL (save/load CallGraph as blob)

**Scope:** Implement `GraphStore` trait for PostgreSQL (save bincode blob + populate normalized tables). Mirror `SqliteGraphStore` exactly but with `sqlx` instead of `rusqlite`.

| Aspect | Assessment |
|--------|-----------|
| Lines | ~400-600 ∆ |
| Dependencies | All prior slices |
| Risk | High — sync trait on async pool, blob storage in PG is questionable design |
| Completeness | Full write-path |

**Pros:** Complete parity with SQLite.
**Cons:** The bincode blob path is an anti-pattern for PostgreSQL (PostgreSQL is the canonical store, not a blob cache). The roadmap explicitly says "JSON graph snapshots are export and import only; the canonical state is in PostgreSQL." A bincode blob in PostgreSQL contradicts this. **REJECTED** — pure normalized tables is the correct PostgreSQL approach.

---

## 4. Recommended Next Slice

### Recommendation: Candidate A (read-path) + minimal write-path for testing

**What it includes:**

1. **Schema:** Add `call_edges` table to `schema_postgres.sql` (column-for-column compatible with SQLite `call_edges`)
2. **Trait extension:** Add edge query methods to the `Repository` trait:
   - `find_edges_by_caller(caller_id: &str) -> Vec<EdgeMetadata>`
   - `find_edges_by_callee(callee_id: &str) -> Vec<EdgeMetadata>`
   - `count_edges() -> usize`
3. **PostgreSQL impl:** Implement the new methods in `PostgresRepository` using typed queries (`sqlx::query_as`)
4. **Minimal write-path (for tests):** Add an `insert_edge()` method to `PostgresRepository` (NOT on the `Repository` trait — this is a concrete impl detail) so tests can seed edges without needing a full `CallGraph` → tables pipeline
5. **Contract tests:** Per-test isolated PostgreSQL tests verifying roundtrip, metadata preservation, and empty-state queries
6. **`EdgeMetadata` value object:** A lightweight struct in `cognicode-core::domain::value_objects` with fields: `caller_id`, `callee_id`, `dependency_type`, `provenance`, `confidence`

**Justification:**
- **Dependency-ready** — only depends on the completed `explorer-graph-postgres-repository`
- **Reviewable** — ~200-300 ∆lines across 3 files
- **Aligned with roadmap** — Phase 1 is model hardening; edge persistence is a model concern
- **Aligned with stack decisions** — PostgreSQL, no second DB, pure normalized tables (no bincode blobs in PG)
- **Aligned with architecture** — extends the existing `Repository` seam, doesn't create a new one
- **Does NOT tie GraphStore to PostgreSQL** — keeps the sync write-path out of the async repository

### What this slice does NOT do (explicit non-goals):

| Non-goal | Why deferred |
|----------|-------------|
| Full `save_call_graph(&CallGraph)` write-path | Requires transaction design, batch insert optimization, and error-recovery strategy. Separate slice. |
| `GraphStore` implementation for PostgreSQL | Sync trait + async pool = design tension. The bincode blob path has no place in the canonical PostgreSQL store. |
| Explorer-to-Postgres adapter (`CallGraphRepository` → `PostgresRepository`) | The explorer's `SymbolRepository`/`MetadataAwareRepository` ports bridge from domain aggregates, not directly from the persistence layer. This bridge belongs in a later slice. |
| MCP tool wiring for edge queries | Phase 2 concern (MCP Graph Navigation API). Not yet dependency-ready. |
| Add `petgraph` projection from PostgreSQL rows | The `petgraph` layer works on in-memory `CallGraph` projections. PostgreSQL-sourced projections are a Phase 2 concern. |
| Batch ingestion / bulk insert | Premature optimization. Single-row inserts via parameterized queries are sufficient for correctness-first development. |
| `ltree` or `pgvector` columns | These are Phase 2/3 concerns (hierarchical data and embeddings). Adding them now would be speculative. |

---

## 5. Dependency Notes for Proposal/Spec/Design

1. **Trait surface:** The `Repository` trait in `cognicode-core/src/domain/traits/repository.rs` must be extended. The proposal must justify which methods to add and why only 3 (not 8).
2. **Value object:** `EdgeMetadata` must be defined in `cognicode-core::domain::value_objects`. It should be a plain struct (not an aggregate) since edges have no domain logic — they're pure data carriers.
3. **SQL compatibility:** The PostgreSQL `call_edges` schema must be column-for-column compatible with the SQLite `call_edges` schema. This is the same strategy used for `symbols` (see `schema_postgres.sql` line 9).
4. **Feature gating:** All new PostgreSQL code must be behind `#[cfg(feature = "postgres")]`. Default builds stay sqlx-free. The trait extension (new methods on `Repository`) does NOT need feature gating — traits define the contract; implementations provide it.
5. **Migration strategy:** Continue with raw SQL via `include_str!`. The PostgreSQL table count is now 2 (symbols + call_edges). Switch to `refinery` or `sqlx-cli` only when table count exceeds 3 (per the documented commitment in `postgres_repository.rs` line 5).
6. **Test isolation:** Per-test databases (same pattern as `postgres_repository.rs` lines 223-267). Each test creates a uniquely-named DB, runs migrations, seeds edges, queries, and drops the DB.
7. **Error handling:** New error variants may be needed in `RepositoryError` (e.g., `EdgeExists`, `InvalidEdge`). Evaluate whether existing variants (`Store`, `NotFound`, `InvalidQuery`) suffice.

---

## 6. Connascence Landscape (Entropy Protocol A)

**Method**: Heuristic (CogniCode graph build unavailable — qualitative estimation from code reading)

### Affected Components

| Component | Role |
|-----------|------|
| `cognicode-core::domain::traits::Repository` | Trait being extended |
| `cognicode-core::domain::value_objects::EdgeMetadata` | New value object |
| `cognicode-core::infrastructure::persistence::PostgresRepository` | Implementation extended |
| `cognicode-core::infrastructure::persistence::schema_postgres.sql` | Schema extended |
| `cognicode-core::domain::aggregates::CallGraph::edges_with_metadata()` | Source of edge data for write-path tests |
| `cognicode-db::graph::SqliteGraphStore::populate_edges()` | Reference implementation |
| `cognicode-db::schema::call_edges` | Reference schema |

### Connascence Pairs

| Component A | Component B | Connascence Type | I(bits) | Severity |
|-------------|-------------|------------------|---------|----------|
| `EdgeMetadata` struct | `call_edges` table schema | Name | log2(7) ≈ 2.81 | ⚠️ Medium |
| `EdgeMetadata` struct | `Repository` trait | Type | log2(2) ≈ 1.0 | ⚠️ Medium |
| `PostgresRepository` | `SqliteGraphStore::populate_edges` | Algorithm | log2(2) ≈ 1.0 | ⚠️ Medium |
| `schema_postgres.sql` | `schema.rs` (SQLite) | Name (column names) | log2(7) ≈ 2.81 | ⚠️ Medium |
| `Repository::find_edges_by_caller` | `CallGraph::edges_with_metadata` | Meaning (both return edges but different shapes) | 0.5 | ✅ Low |

### Critical Pairs: None detected — all pairs are within acceptable range.

### SOLID-Entropy Assessment

| Principle | Assessment | Status |
|-----------|-----------|--------|
| **SRP** | Edge persistence is a single responsibility — no split needed | ✅ |
| **OCP** | `Repository` trait is extended (new methods), NOT modified (existing methods unchanged). H(Δ_existing) ≈ 0 | ✅ |
| **LSP** | No subtypes of `Repository` in this slice — not applicable | ✅ |
| **ISP** | New methods are minimal (3 methods); callers only see what they need | ✅ |
| **DIP** | `PostgresRepository` depends on `Repository` (trait), not the reverse | ✅ |

### Design Quality Score (DQS): **0.78 / 1.0 (EXCELLENT)**

Analysis: Low coupling (only 3 trait methods added, each with a single PostgreSQL query). High cohesion (all edge-related methods in one trait). No LSP violations. No critical connascence pairs.

---

## 7. Auto-Grill Results

### Input Grilled
The exploration findings above: PostgreSQL `call_edges` table + `Repository` trait extension + minimal write-path.

### Preguntas: 9 | Auto-resueltas: 7 (78%) | Escaladas: 2

### Auto-Resolved Decisions

| # | Pregunta | Resolución | Evidencia | Confianza |
|---|----------|-----------|-----------|-----------|
| 1 | ¿Debe `call_edges` ser una tabla separada o un campo JSONB en `symbols`? | Tabla separada (column-for-column con SQLite). JSONB para edges no escala en consultas de grafos con filtros por provenance/confidence. | `schema.rs:85-94` (SQLite), `schema_postgres.sql:12-20` (patrón symbols) | 0.95 |
| 2 | ¿Deben añadirse los métodos de edge query al trait `Repository` o a un sub-trait separado? | Al trait `Repository` directamente. El trait ya es async y representa la seam de consulta canónica. Un sub-trait añade indirección innecesaria para 3 métodos. | `repository.rs:1-9` — docstring dice "PostgreSQL implementations will add typed query methods in a follow-up slice." | 0.95 |
| 3 | ¿Debe usarse `spawn_blocking` para queries de edges en PostgreSQL? | No. `sqlx` es nativamente async. Las queries de edge son lecturas simples sin bloqueo. | `postgres_repository.rs:164-175` — `find_symbol_by_qualified_name` usa `sqlx::query_as` sin `spawn_blocking` | 0.90 |
| 4 | ¿Debe `EdgeMetadata` ser un struct separado o reutilizar `CallEntry`? | Struct separado. `CallEntry` incluye `depth` (contexto de traversal) que no aplica a consultas directas de edge. `EdgeMetadata` debe ser un value object plano. | `call_graph.rs:15-28` (CallEntry tiene depth), `edges_with_metadata() → (SymbolId, SymbolId, DependencyType, Provenance, f64)` (los 5 campos necesarios) | 0.95 |
| 5 | ¿Debe el schema PostgreSQL incluir índices para `call_edges`? | Sí. Índices en `caller_id` y `callee_id` (mínimo). El índice en `provenance` es útil para filtros pero puede diferirse. | `schema.rs:105-107` (SQLite tiene índices en caller/callee) | 0.90 |
| 6 | ¿Puede `count_edges()` usar `SELECT COUNT(*)` sin materialized view? | Sí. Con índices en `caller_id` y `callee_id`, `COUNT(*)` es O(1) en PostgreSQL (heap scan over PK). Para tablas >1M rows, materialized view sería necesario — pero estamos lejos de ese punto. | Documentación PostgreSQL: `COUNT(*)` sin WHERE usa el índice más pequeño disponible. | 0.85 |
| 7 | ¿Debe este slice incluir el write-path de ingestión masiva (`save_call_graph`)? | No. Ese es un slice separado. El write-path en este slice es solo `insert_edge()` para seeding de tests. | Roadmap Phase 1: "JSON graph snapshot is a first-class export and debugging artifact." La ingestión del CallGraph a PostgreSQL es un problema de diseño más grande. | 0.90 |

### Escalated Decisions (require human validation)

| # | Pregunta | Opciones | OS Recomendada | Confianza |
|---|----------|----------|----------------|-----------|
| 8 | ¿Debe el nombre de la tabla PostgreSQL ser `call_edges` (mismo que SQLite) o `graph_edges` (más genérico pensando en edges no-call futuros)? | **A: `call_edges`** (OS=0.78) — consistencia con SQLite, nomenclatura existente, sin breaking changes. B: `graph_edges` (OS=0.42) — más genérico pero rompe la compatibilidad columna-por-columna con SQLite. | A | 0.70 |
| 9 | ¿Deben los métodos de edge query devolver `Vec<EdgeMetadata>` o `impl Iterator`? | **A: `Vec<EdgeMetadata>`** (OS=0.65) — simple, compatible con `async_trait`, sin lifetimes complejos. B: `Box<dyn Iterator>` (OS=0.30) — más flexible pero requiere heap allocation y no es compatible con `async_trait` sin boxing adicional. | A | 0.80 |

### Opportunity Score Detail (for escalated decisions)

**Pregunta 8 — Nombre de la tabla:**

| Opción | Acoplamiento | Free Energy | Apertura | Flexibilidad | Profundidad | Irreversibilidad | OS |
|--------|-------------|-------------|----------|-------------|-------------|-----------------|-----|
| A: `call_edges` | 0.15 | 0.10 | 0.85 | 0.88 | 0.90 | 0.25 | **0.78** |
| B: `graph_edges` | 0.45 | 0.40 | 0.60 | 0.35 | 0.50 | 0.60 | 0.42 |

**Pregunta 9 — Tipo de retorno:**

| Opción | Acoplamiento | Free Energy | Apertura | Flexibilidad | Profundidad | Irreversibilidad | OS |
|--------|-------------|-------------|----------|-------------|-------------|-----------------|-----|
| A: `Vec<EdgeMetadata>` | 0.10 | 0.05 | 0.90 | 0.80 | 0.85 | 0.15 | **0.65** |
| B: `Box<dyn Iterator>` | 0.40 | 0.30 | 0.50 | 0.45 | 0.40 | 0.50 | 0.30 |

### Documentation Updates
- **CONTEXT.md**: No changes needed — terms used (`call_edges`, `Repository`, `EdgeMetadata`) are either already defined or will be defined in the glossary during the proposal/design phases.
- **ADR**: No new ADR needed — this slice follows established decisions (PostgreSQL canonical, trait-based persistence, raw SQL via `include_str!`).

### Reporte HTML
Generado en `/tmp/sdd-explorer-graph-postgres-call-edges-auto-grill.html` y `openspec/changes/explorer-graph-postgres-call-edges/reports/auto-grill.html`.

### Status: pending_validation (2 escalated decisions require human approval)

---

## 8. Risks

| Risk | Severity | Mitigation |
|------|----------|-----------|
| Schema divergence between SQLite `call_edges` and PostgreSQL `call_edges` | Medium | Enforce column-for-column compatibility. Add a contract test that compares `PRAGMA table_info(call_edges)` (SQLite) against `information_schema.columns` (PostgreSQL). |
| `async_trait` + `Vec` return type is not zero-cost | Low | For 3 methods returning small edge sets, `Vec` allocation is negligible. Switch to `Stream` or pagination when edge counts exceed 10K per query. |
| The `insert_edge()` write-path helper becomes a de-facto public API | Low | Keep `insert_edge()` as `pub(crate)` on `PostgresRepository`. Tests access it via `#[cfg(test)]` re-exports. |
| `Repository` trait grows too large | Low | 3 new methods (from 2 → 5 total). The docstring already anticipates growth. If the trait exceeds 10 methods, split into sub-traits — but we're far from that threshold. |
| PostgreSQL test infrastructure requirement | Medium | Tests are gated behind `TEST_DATABASE_URL` env var. CI must provide a PostgreSQL 14+ service. This is the same pattern as the prior slice — already proven in CI. |

---

## 9. Explicit Non-Goals

1. **No full `save_call_graph` write-path.** This slice provides the read-path + minimal write helpers for tests. The full ingest pipeline (CallGraph → PostgreSQL tables) is a separate slice.
2. **No `GraphStore` implementation for PostgreSQL.** The sync `GraphStore` trait and its bincode blob pattern are intentionally NOT ported to PostgreSQL. PostgreSQL is the canonical normalized store, not a blob cache.
3. **No explorer-to-Postgres adapter.** The explorer's `SymbolRepository`/`MetadataAwareRepository` ports remain backed by `CallGraph` (in-memory). A PostgreSQL-backed explorer adapter is Phase 2/3 work.
4. **No MCP tool wiring.** Edge queries are internal `Repository` methods — not yet exposed as MCP tools. That's Phase 2.
5. **No `petgraph` projection from PostgreSQL rows.** The algorithmic layer (`petgraph`) continues to work on in-memory `CallGraph` projections. PostgreSQL-sourced projections are deferred.
6. **No batch/bulk operations.** Single-row inserts and queries. Bulk optimization is premature.

---

## 10. Skill Resolution

| Skill | Status | Notes |
|-------|--------|-------|
| `entropy-sdd` | ✅ Protocol A executed | Connascence landscape computed; DQS = 0.78 (EXCELLENT) |
| `auto-grill` | ✅ Complete | 9 preguntas, 7 auto-resueltas, 2 escaladas; HTML report generated |
| `cognicode-sdd` | ⚠️ Partial | `build_graph` timed out repeatedly — fell back to heuristic analysis. Re-run in sdd-propose when CogniCode is available. |
| `logseq-vault` | ✅ Will persist | Artifact pages and journal entries to be created after orchestrator review |
