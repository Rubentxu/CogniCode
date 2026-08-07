# Archive Report: explorer-graph-postgres-graphstore

**Slice**: PostgreSQL Canonical Write-Path — `save_call_graph` + `load_call_graph` inherent methods on `PostgresRepository`
**Archived**: 2026-06-08
**Artifact Store**: hybrid (Engram + OpenSpec)
**Status**: ✅ ARCHIVED

---

## 1. What Changed

| Field | Detail |
|-------|--------|
| **File modified** | `crates/cognicode-core/src/infrastructure/persistence/postgres_repository.rs` |
| **Lines added** | +817 (vs ~310 planned; well-commented implementation) |
| **Lines deleted** | 0 |
| **Schema changes** | None |
| **Trait changes** | None |
| **New feature flag** | None (reuses `#[cfg(feature = "postgres")]`) |

### New capabilities delivered

- `PostgresRepository::save_call_graph(&self, graph: &CallGraph) -> Result<(), RepositoryError>` — transactional DELETE+INSERT into `symbols` + `call_edges`, all in one `sqlx` transaction
- `PostgresRepository::load_call_graph(&self) -> Result<Option<CallGraph>, RepositoryError>` — SELECT reconstruction via `add_dependency_with_provenance` with bit-exact provenance mapping
- `provenance_to_extraction_context` private helper (Extracted→DirectExtraction, Inferred→Heuristic{score}, Ambiguous→Unresolved)
- 8 contract `pg_test!` tests covering: happy path, empty→None, exact metadata, round-trip assert_eq!, delete-and-replace, idempotent re-save, mid-INSERT rollback, DELETE-phase rollback

### Domain archived

- **Domain**: `postgres-callgraph-persistence`
- **Spec**: `openspec/specs/postgres-callgraph-persistence/spec.md` (new — was delta only, now promoted to main spec)
- All artifacts moved to `openspec/changes/archive/2026-06-08-explorer-graph-postgres-graphstore/`

---

## 2. What Intentionally Did NOT Change

| Area | Reason |
|------|--------|
| `SqliteGraphStore` | SQLite write-path is separate; blob+tables remain SQLite's canonical pattern |
| `GraphStore` trait | Sync trait on async pool — architecturally rejected in prior slices |
| `Repository` trait | Read-path only by design; write-path is additive inherent methods |
| `cognicode-db/src/graph.rs` | Verified empty diff post-apply |
| `domain/traits/graph_store.rs` | Verified empty diff post-apply |
| `symbols` / `call_edges` schema | Tables already existed; no DDL needed |
| `postgres-call-edges` spec | Different domain (edge queries vs full graph persistence) |
| New async trait | Premature abstraction — one implementor doesn't justify a trait |
| Blob/bincode in PG | Normalized tables are canonical truth per roadmap |
| Explorer adapter / MCP envelope / petgraph projection | Unblocked by this slice but separate slices |

---

## 3. Entropy / Design Quality Summary

### Connascence pairs (all explicit, documented)

| Pair | Type | I(bits) | Severity |
|------|------|---------|----------|
| `save_call_graph` ↔ `CallGraph` | Type | 1.0 | ⚠️ Medium |
| `save_call_graph` ↔ `symbols` table | Algorithm | 1.58 | ⚠️ Medium |
| `save_call_graph` ↔ `call_edges` table | Algorithm | 1.58 | ⚠️ Medium |
| `save_call_graph` ↔ `SqliteGraphStore::populate_*` | Algorithm | 2.0 | ⚠️ Medium |
| `load_call_graph` ↔ `CallGraph` | Type | 0.5 | ✅ OK |
| `load_call_graph` ↔ tables | Name | 0.32 | ✅ OK |

**Critical pairs (I > 3.0)**: None
**Hidden connascence**: None
**SOLID violations**: None

### Design Quality Score

- **DQS**: ~0.65/1.0 (ACCEPTABLE)
- **Entropy budget**: H(Δ_existing) = 0 bits — pure additive, single file
- **OCP compliance**: ✅ — zero existing code modified
- **Architecture decisions documented**: 4 decisions (inherent vs trait, delete-and-replace vs upsert, route through `add_dependency_with_provenance`, `Result<Option<..>>` vs bare `Option`)
- **Connascence trend**: Stable — all pairs explicit and documented

### Size Budget Exception

> ⚠️ **Explicit size-budget exception**: +817 lines added vs ~310 planned. The implementation is well-commented and behaviorally correct. The 400-line soft budget was exceeded. Budget risk was rated "Low" per spec because it is single-file, additive, with no deletions and no cross-cutting concerns. This exception is documented in the verify-report and accepted.

---

## 4. Artifact Inventory (Hybrid Store)

### Engram observations

| Observation ID | Type | Artifact |
|----------------|------|----------|
| #1329 | architecture | `sdd/explorer-graph-postgres-graphstore/explore` |
| #1330 | architecture | `sdd/explorer-graph-postgres-graphstore/proposal` |
| #1331 | architecture | `sdd/explorer-graph-postgres-graphstore/spec` |
| #1332 | architecture | `sdd/explorer-graph-postgres-graphstore/design` |
| #1333 | architecture | `sdd/explorer-graph-postgres-graphstore/tasks` |
| #1334 | architecture | `sdd/explorer-graph-postgres-graphstore/apply-progress` |
| #1336 | architecture | `sdd/explorer-graph-postgres-graphstore/verify-report` |

### OpenSpec filesystem

| Path | Artifact |
|------|----------|
| `openspec/changes/archive/2026-06-08-explorer-graph-postgres-graphstore/proposal.md` | Proposal |
| `openspec/changes/archive/2026-06-08-explorer-graph-postgres-graphstore/specs/postgres-callgraph-persistence/spec.md` | Delta spec |
| `openspec/changes/archive/2026-06-08-explorer-graph-postgres-graphstore/design.md` | Design |
| `openspec/changes/archive/2026-06-08-explorer-graph-postgres-graphstore/tasks.md` | Tasks |
| `openspec/changes/archive/2026-06-08-explorer-graph-postgres-graphstore/exploration.md` | Exploration |
| `openspec/changes/archive/2026-06-08-explorer-graph-postgres-graphstore/reports/auto-grill.html` | Auto-grill report |
| `openspec/specs/postgres-callgraph-persistence/spec.md` | Main spec (promoted from delta) |

---

## 5. Follow-On Slices Now Unblocked

| Slice | What it needs | Status |
|-------|---------------|--------|
| **Explorer PostgreSQL Adapter** | Populated PostgreSQL; `SymbolRepository` can resolve from PG | ✅ UNBLOCKED |
| **MCP PostgreSQL Envelope (Phase 3)** | `save_call_graph` + `load_call_graph` for test data seeding | ✅ UNBLOCKED |
| **petgraph Projection from PostgreSQL** | Data in PG to build in-memory `petgraph::Graph` | ✅ UNBLOCKED |
| **CI/CD PostgreSQL Integration** | `cargo test --features postgres` now tests real data operations | ✅ UNBLOCKED |

### Dependency chain

```
explorer-graph-foundation          ✅ ARCHIVED
  └── explorer-graph-repository-bridge  ✅ ARCHIVED
        └── explorer-graph-postgres-repository  ✅ ARCHIVED
              └── explorer-graph-postgres-call-edges  ✅ ARCHIVED
                    └── explorer-graph-postgres-graphstore  ✅ ARCHIVED (this slice)
                          ├── Explorer PostgreSQL Adapter  🔲 unblocked
                          ├── MCP PostgreSQL Envelope  🔲 unblocked
                          └── petgraph Projection  🔲 unblocked
```

---

## 6. Cleanup / Debt Notes

| Item | Notes |
|------|-------|
| Pre-existing test failures (5) | `rustc not found ×3`, `path accessibility ×2` — environmental, unrelated to this slice |
| Pre-existing E0432 import error | `interface/mcp/handlers/mod.rs` — exists in default build, not introduced by this slice |
| Size budget | Exceeded 400-line soft budget (+817 vs ~310 planned); accepted with documented exception |
| No schema migrations needed | `run_migrations()` re-run is byte-identical on populated PG |
| Single-file revert | `git revert` of `postgres_repository.rs` is sufficient and clean |

---

## 7. Archival Verdict

| Criterion | Status |
|-----------|--------|
| All tasks complete | ✅ 17/17 tasks checked off |
| All spec requirements met | ✅ |
| All 8 contract tests pass | ✅ |
| No CRITICAL issues in verify-report | ✅ (pre-existing failures are environmental) |
| Delta spec synced to main spec | ✅ — `postgres-callgraph-persistence` domain created |
| Change folder moved to archive | ✅ — `openspec/changes/archive/2026-06-08-explorer-graph-postgres-graphstore/` |
| Archive is an audit trail | ✅ — never modified after archival |
| OpenSpec spec promoted | ✅ — `openspec/specs/postgres-callgraph-persistence/spec.md` |

### Final verdict: **ARCHIVED ✅**

PostgreSQL now has a canonical write-path. `save_call_graph` + `load_call_graph` are live on `PostgresRepository`, feature-gated behind `#[cfg(feature = "postgres")]`, transactional, and round-trip-verified. The slice is clean, OCP-compliant, single-file, and unblocks the entire downstream adapter chain.

**Remaining failures are pre-existing and environmental (rustc not found, path accessibility) — they did not block archival.**

---

## Observation IDs for Traceability

```
#1329 — sdd/explorer-graph-postgres-graphstore/explore
#1330 — sdd/explorer-graph-postgres-graphstore/proposal
#1331 — sdd/explorer-graph-postgres-graphstore/spec
#1332 — sdd/explorer-graph-postgres-graphstore/design
#1333 — sdd/explorer-graph-postgres-graphstore/tasks
#1334 — sdd/explorer-graph-postgres-graphstore/apply-progress
#1336 — sdd/explorer-graph-postgres-graphstore/verify-report
```
