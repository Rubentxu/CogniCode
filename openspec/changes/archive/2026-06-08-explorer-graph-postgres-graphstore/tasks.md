# Tasks: PostgreSQL Canonical Write-Path for CallGraph

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~310 (additions only; no deletions) |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR |
| Delivery strategy | ask-on-risk |
| Chain strategy | size-exception |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: size-exception
400-line budget risk: Low

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Add `save_call_graph` + `load_call_graph` inherent methods + 8 contract tests, additive to `postgres_repository.rs` | PR 1 | Single file; 1 backend; 1 file change. Below 400-line budget. |

> Delivery strategy rationale: ~310 additive lines, no deletions, no schema/trait changes, no cross-cutting concerns, single backend, single file. The slice is well within the 400-line review budget; one PR is appropriate. `size:exception` is not required because the change IS under the budget — `Chain strategy: size-exception` is listed only as the resolved value of `ask-on-risk` for this in-budget slice. Apply may proceed without an explicit user decision.

## Phase 1: Helper & Save Method

- [x] 1.1 Add `provenance_to_extraction_context` private fn in `postgres_repository.rs` inside the `impl PostgresRepository` block: `Extracted→DirectExtraction`, `Inferred→Heuristic{score:confidence}`, `Ambiguous→Unresolved` (file: `crates/cognicode-core/src/infrastructure/persistence/postgres_repository.rs`)
- [x] 1.2 Implement `pub async fn save_call_graph(&self, graph: &CallGraph) -> Result<(), RepositoryError>` in `postgres_repository.rs`: `pool.begin()` → `DELETE FROM call_edges` → `DELETE FROM symbols` → loop `graph.symbol_ids()` binding `(file_path, name, kind, line, column)` to `INSERT INTO symbols` → loop `graph.edges_with_metadata()` binding `(caller_id, caller_name, callee_id, callee_name, dependency_type, provenance, confidence)` to `INSERT INTO call_edges` → `tx.commit()`. Wrap errors as `RepositoryError::Store("save_call_graph <step>: …")`. Auto-rollback on `tx` drop. Feature-gated with `#[cfg(feature = "postgres")]`
- [x] 1.3 Verify: `cargo check -p cognicode-core --features postgres` compiles; `cargo check --workspace` (no feature) succeeds with the method unreachable

## Phase 2: Load Method

- [x] 2.1 Implement `pub async fn load_call_graph(&self) -> Result<Option<CallGraph>, RepositoryError>` in `postgres_repository.rs`: `SELECT file_path, name, kind, line, column FROM symbols ORDER BY id` → if empty AND `SELECT COUNT(*) FROM call_edges == 0` return `Ok(None)`; else build `CallGraph`, `add_symbol` per row via `SymbolRow::into_symbol`, build `HashMap<String, SymbolId>` from `symbol.fully_qualified_name()` → `SELECT caller_id, caller_name, callee_id, callee_name, dependency_type, provenance, confidence FROM call_edges ORDER BY id` → for each row decode via `EdgeRow::into_edge`, look up `src_id`/`tgt_id` from the FQN map, call `provenance_to_extraction_context(prov, conf)` → `graph.add_dependency_with_provenance(&src_id, &tgt_id, dep_type, ctx)`. Return `Ok(Some(graph))`. Feature-gated with `#[cfg(feature = "postgres")]`
- [x] 2.2 Verify: `cargo check -p cognicode-core --features postgres` compiles; `cargo doc --no-deps -p cognicode-core --features postgres` exposes both methods

## Phase 3: Contract Tests

- [x] 3.1 Add `build_mixed_provenance_graph()` test helper at the top of the `mod tests` block in `postgres_repository.rs` (≥5 symbols, ≥3 dependency types, all 3 provenance variants with confidences `{0.0, 0.5, 1.0}`, includes one self-loop and one multi-edge pair with different `DependencyType`s). Feature-gated with `#[cfg(all(test, feature = "postgres"))]`
- [x] 3.2 `pg_test!(save_populates_both_tables, …)` — empty DB, save mixed-provenance graph, assert `count_symbols()==5+` and `count_edges()==expected`
- [x] 3.3 `pg_test!(load_empty_returns_none, …)` — fresh DB, `load_call_graph` returns `Ok(None)`, no write DML issued
- [x] 3.4 `pg_test!(load_populated_returns_some_with_exact_metadata, …)` — save graph → load → assert counts + per-edge `(provenance, confidence)` matches source bit-for-bit (f64 exact)
- [x] 3.5 `pg_test!(round_trip_assert_eq, …)` — build fixture, save→load, `assert_eq!(original, loaded)` passes (covers self-loop + multi-edge preservation)
- [x] 3.6 `pg_test!(delete_and_replace_overwrites, …)` — save graph A (3 sym) → save graph B (different 5 sym) → assert only B's rows present, no A rows remain
- [x] 3.7 `pg_test!(idempotent_re_save, …)` — save same graph twice → assert counts equal post-first-save counts; row set semantically equivalent
- [x] 3.8 `pg_test!(rollback_on_mid_insert_failure, …)` — seed a row with a unique-index conflict, call `save_call_graph` → assert `Err(RepositoryError::Store(_))` AND `count_symbols()==pre-seed` AND `count_edges()==0`
- [x] 3.9 `pg_test!(rollback_unwinds_delete_phase, …)` — save A (3 sym, 4 edges) → attempt to save B that fails AFTER both `DELETE`s (test seam: pre-seeded conflicting row) → assert A's 3 symbols + 4 edges still present

## Phase 4: Non-Breaking & Build Verification

- [x] 4.1 Run `cargo test --workspace` (no `--features postgres`) — every pre-slice test must pass; new methods must be unreachable
- [x] 4.2 Run `cargo check --workspace` (no `postgres` feature) — build succeeds; `sqlx` is absent from the dep graph
- [x] 4.3 Run `cargo test -p cognicode-core --features postgres postgres_repository::tests` — all 8 new contract tests pass + all pre-slice tests pass
- [x] 4.4 Run `git diff HEAD -- crates/cognicode-db/src/graph.rs crates/cognicode-core/src/domain/traits/graph_store.rs` — both files MUST show empty diff (SQLite + `GraphStore` untouched)
- [x] 4.5 Run `cargo doc --no-deps -p cognicode-core` (no feature) — exposed item list is byte-identical to pre-slice; `use cognicode_core::infrastructure::persistence::PostgresRepository::save_call_graph` fails to compile

## Phase 5: Rollback Safety Check

- [x] 5.1 Confirm no `ALTER TABLE`, no new tables, no new indexes in PG — `run_migrations()` re-run on populated instance is byte-identical to pre-slice
- [x] 5.2 Confirm `git revert <merge-sha>` workflow: reverts a single file (`postgres_repository.rs`); pre-slice `--features postgres` build does not regress
- [x] 5.3 Confirm `additions + deletions ≤ 400` on the final diff (target: ~310 lines, all additive)

## Out of Scope (locked from spec)

- `GraphStore` trait impl for PG
- New async `GraphPersistence` trait
- New `Repository` methods
- New tables/columns/indexes in PG
- Bincode/blob sidecar
- Explorer-PG adapter, MCP envelope, petgraph projection
- `ltree` / `pgvector`
- Removal of SQLite / `cognicode-store-traits` / `GraphStore`
- Schema migrations
