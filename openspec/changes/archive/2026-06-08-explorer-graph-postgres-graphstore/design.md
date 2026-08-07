# Design: PostgreSQL Canonical Write-Path for CallGraph

## Technical Approach

Two inherent async methods on `PostgresRepository` — `save_call_graph` and `load_call_graph` — that populate and reconstruct a full `CallGraph` from the existing `symbols` + `call_edges` PG tables inside a single `sqlx` transaction. No schema changes, no trait changes, no new types. Pure additive extension following the existing `insert_edge()` pattern.

## Architecture Decisions

### Decision: Inherent methods, not trait impl

| Option | Tradeoff | Decision |
|--------|----------|----------|
| Inherent methods on `PostgresRepository` | No trait dispatch; caller needs concrete type | **Chosen** |
| Impl `GraphStore` trait | Sync trait on async pool — architecturally rejected (see proposal) | Rejected |
| New async `GraphPersistence` trait | Premature abstraction for one implementor | Rejected |

**Rationale**: One PG backend exists. A trait with a single impl is indirection without benefit. Inherent methods mirror the existing `insert_edge()` precedent. If a second async backend appears later, a trait can be extracted then.

### Decision: Delete-and-replace write strategy

| Option | Tradeoff | Decision |
|--------|----------|----------|
| `DELETE` all + `INSERT` all in one tx | Simple; full replacement guarantee | **Chosen** |
| Row-level upsert (`ON CONFLICT`) | Complex merge logic; no caller needs partial updates | Rejected |

**Rationale**: The `save_call_graph` contract is "make the DB match this graph exactly." Delete-and-replace is the simplest correct strategy. Upsert would require identity columns, conflict detection, and partial-update semantics nobody needs.

### Decision: Route load through `add_dependency_with_provenance`

| Option | Tradeoff | Decision |
|--------|----------|----------|
| Reconstruct via `add_dependency_with_provenance(ExtractionContext)` | Routes through `ConfidenceRules`; guaranteed domain-valid output | **Chosen** |
| Bypass rules, set `(Provenance, f64)` directly | Would require a new `CallGraph` method; breaks invariant | Rejected |

**Rationale**: `add_dependency_with_provenance` is the sole sanctioned path. The provenance→ExtractionContext reverse mapping is bit-exact for all values produced by `ConfidenceRules::assign`, because the stored data was itself produced by that function. See mapping table below.

### Decision: Return `Result<Option<CallGraph>, RepositoryError>` for load

| Option | Tradeoff | Decision |
|--------|----------|----------|
| `Result<Option<CallGraph>, RepositoryError>` | Idiomatic; consistent with all other `PostgresRepository` methods | **Chosen** |
| `Option<CallGraph>` (spec literal) | Cannot report DB errors | Noted deviation |

**Rationale**: SELECT can fail (connection drop, schema corruption). Every other method on `PostgresRepository` returns `Result`. The spec's bare `Option` is an abbreviation; `Result<Option<..>>` preserves the contract (None = empty tables) while allowing error propagation.

## Data Flow

### Save

```
CallGraph
  │
  ├── symbol_ids() ──→ foreach: INSERT INTO symbols (file_path,name,kind,line,column)
  │
  └── edges_with_metadata() ──→ foreach: INSERT INTO call_edges (7 cols)
                                      │
                                 All in one sqlx::Transaction:
                                 1. DELETE FROM call_edges
                                 2. DELETE FROM symbols
                                 3. INSERT symbols
                                 4. INSERT edges
                                 5. COMMIT / ROLLBACK on error
```

### Load

```
SELECT FROM symbols ORDER BY id
  │
  └── SymbolRow::into_symbol() ──→ graph.add_symbol(symbol) ──→ SymbolId map

SELECT FROM call_edges ORDER BY id
  │
  └── EdgeRow::into_edge() ──→ provenance_to_extraction_context(prov, conf)
                                    │
                                    └── graph.add_dependency_with_provenance(src, tgt, dep, ctx)
                                          │
                                          └── Returns Option<CallGraph>
                                                (None iff both tables empty)
```

### Provenance → ExtractionContext mapping (load path)

| Stored `Provenance` | Stored `confidence` | Reconstructed `ExtractionContext` | Re-assigned output |
|---------------------|--------------------:|-----------------------------------|--------------------|
| `Extracted` | `1.0` | `DirectExtraction` | `(Extracted, 1.0)` |
| `Inferred` | `[0.5..=0.9]` | `Heuristic { score }` | `(Inferred, clamp(score,0.5,0.9))` = same |
| `Ambiguous` | `0.3` | `Unresolved` | `(Ambiguous, 0.3)` |

Bit-exact for all stored values because stored confidence is always the output of `ConfidenceRules::assign`.

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/infrastructure/persistence/postgres_repository.rs` | Modify | Add `save_call_graph`, `load_call_graph`, `provenance_to_extraction_context` helper, and contract tests (~300 lines) |

## Interfaces / Contracts

### `save_call_graph`

```rust
#[cfg(feature = "postgres")]
impl PostgresRepository {
    /// Transactional write: DELETE + INSERT of all symbols and edges.
    /// Wraps the entire operation in one `sqlx` transaction.
    /// Returns `Ok(())` only after COMMIT.
    pub async fn save_call_graph(
        &self,
        graph: &CallGraph,
    ) -> Result<(), RepositoryError> { ... }
}
```

**Transaction steps**:
1. `let mut tx = self.pool.begin().await`
2. `sqlx::query("DELETE FROM call_edges").execute(&mut *tx).await`
3. `sqlx::query("DELETE FROM symbols").execute(&mut *tx).await`
4. For each `(id, symbol)` in `graph.symbol_ids()`:
   - `INSERT INTO symbols (file_path, name, kind, line, column) VALUES ($1..$5)`
   - Bind: `location.file()`, `symbol.name()`, `format!("{:?}", kind)`, `line as i32`, `column as i32`
5. For each `(src, tgt, dep, prov, conf)` in `graph.edges_with_metadata()`:
   - `INSERT INTO call_edges (caller_id, caller_name, callee_id, callee_name, dependency_type, provenance, confidence) VALUES ($1..$7)`
   - Bind: `src.as_str()`, `graph.get_symbol(&src).name()`, `tgt.as_str()`, `graph.get_symbol(&tgt).name()`, `format!("{:?}", dep)`, `prov.to_string()`, `conf`
6. `tx.commit().await`
7. On any error: `tx` drops → auto-rollback. Wrap error as `RepositoryError::Store("save_call_graph <step>: ...")`.

### `load_call_graph`

```rust
#[cfg(feature = "postgres")]
impl PostgresRepository {
    /// Read-only reconstruction of CallGraph from normalized tables.
    /// Returns `Ok(None)` iff both tables are empty.
    pub async fn load_call_graph(
        &self,
    ) -> Result<Option<CallGraph>, RepositoryError> { ... }
}
```

**Steps**:
1. `SELECT file_path, name, kind, line, column FROM symbols ORDER BY id` → `Vec<SymbolRow>`
2. If empty: `SELECT COUNT(*) FROM call_edges`. If also 0, return `Ok(None)`.
3. Build `HashMap<String, SymbolId>` mapping `fully_qualified_name → SymbolId` as symbols are added.
4. `SELECT caller_id, caller_name, callee_id, callee_name, dependency_type, provenance, confidence FROM call_edges ORDER BY id` → `Vec<EdgeRow>`
5. For each edge row: `EdgeRow::into_edge()` → extract `(caller_id, callee_id, dependency_type, provenance, confidence)` → `provenance_to_extraction_context(prov, conf)` → `graph.add_dependency_with_provenance(&src_id, &tgt_id, dep_type, ctx)`
6. Return `Ok(Some(graph))`

### `provenance_to_extraction_context` (private helper)

```rust
fn provenance_to_extraction_context(
    provenance: Provenance,
    confidence: f64,
) -> ExtractionContext {
    match provenance {
        Provenance::Extracted => ExtractionContext::DirectExtraction,
        Provenance::Inferred => ExtractionContext::Heuristic { score: confidence },
        Provenance::Ambiguous => ExtractionContext::Unresolved,
    }
}
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit (contract) | Save happy path: populates both tables | `pg_test!` with `fresh_pool()`, assert `count_symbols` + `count_edges` |
| Unit (contract) | Load empty → `Ok(None)` | Fresh DB, call `load_call_graph`, assert `None` |
| Unit (contract) | Load populated → `Ok(Some(g))` with exact metadata | Save 7 sym / 12 edge graph, load, assert counts + per-edge `(prov, conf)` |
| Unit (contract) | Round-trip `assert_eq!` | Build fixture with all 3 provenance variants + self-loop + multi-edge, save→load, `assert_eq!(original, loaded)` |
| Unit (contract) | Transaction rollback on INSERT failure | Seed a row that conflicts, save, verify counts unchanged |
| Unit (contract) | Delete-and-replace overwrites | Save graph A, save graph B, assert only B's rows |
| Unit (contract) | Idempotent re-save | Save same graph twice, assert counts equal |
| Unit (contract) | Mixed-provenance metadata preserved | 3 edges `(Extracted,1.0)`, `(Inferred,0.7)`, `(Ambiguous,0.3)` round-trip bit-exact |

**Test helper** (module-level, within `tests`):

```rust
fn build_mixed_provenance_graph() -> CallGraph {
    // 3+ symbols, edges spanning all 3 ExtractionContext variants,
    // self-loop, multi-edge between same pair with different DependencyType.
}
```

All tests use the existing `pg_test!` macro (not `#[sqlx::test]`) to avoid pulling `sqlx-sqlite`.

## Migration / Rollout

No migration required. No schema changes. Revertible with single `git revert`.

## Open Questions

None — all decisions resolved from proposal + spec + codebase analysis.
