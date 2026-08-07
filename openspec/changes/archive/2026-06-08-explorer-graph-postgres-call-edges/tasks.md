# Tasks: PostgreSQL `call_edges` — Read-Path + Minimal Write Helper

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~280 |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR |
| Delivery strategy | ask-on-risk |
| Chain strategy | pending |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: pending
400-line budget risk: Low

## Phase 1: Domain Foundations (ungated, no sqlx)

- [x] 1.1 Create `edge_metadata.rs` with `EdgeMetadata` (7 fields, `Debug, Clone, PartialEq`, no `Serialize`, ungated).
- [x] 1.2 Re-export `EdgeMetadata` in `value_objects/mod.rs`.
- [x] 1.3 Add `FromStr` to `Provenance` (`"Extracted"|"Inferred"|"Ambiguous"`, `type Err = ()`) + unit tests.
- [x] 1.4 Add `FromStr` to `DependencyType` (Debug + Display forms, 8 variants) + unit tests.

## Phase 2: Repository Trait Extension (ungated)

- [x] 2.1 Add 3 async methods to `Repository`: `find_edges_by_caller`, `find_edges_by_callee`, `count_edges`. Keep `Send + Sync` + `#[async_trait]`.
- [x] 2.2 Update `EmptyRepo` + `CountingRepo` test stubs for the 3 new methods.
- [x] 2.3 Unit test: `Box<dyn Repository>` and `Arc<dyn Repository>` still compile + dispatch.

## Phase 3: Schema Extension (gated)

- [x] 3.1 Append `call_edges` DDL + 2 indexes (`caller_id`, `callee_id`) to `schema_postgres.sql`.
- [x] 3.2 `pg_test!` `schema_idempotent_and_columns_match` — assert 8 columns in declared order via `information_schema.columns`.

## Phase 4: PostgresRepository Edge Implementation (gated)

- [x] 4.1 Add private `#[derive(sqlx::FromRow)] EdgeRow` struct (7 fields), mirror `SymbolRow`.
- [x] 4.2 `EdgeRow::into_edge()` with `FromStr` parsing; fallback `Provenance::Extracted` / `DependencyType::Calls`.
- [x] 4.3 Implement `find_edges_by_caller` via `sqlx::query_as` + `WHERE caller_id = $1 ORDER BY id`.
- [x] 4.4 Implement `find_edges_by_callee` analogously with `WHERE callee_id = $1`.
- [x] 4.5 Implement `count_edges` mirroring `count_symbols`.
- [x] 4.6 Add `pub(crate) async fn insert_edge(&self, edge: &EdgeMetadata)` inherent method (NOT on trait).

## Phase 5: Contract Tests (gated, requires `TEST_DATABASE_URL`)

- [x] 5.1 `pg_test!` `edge_round_trip_insert_then_query` — insert then query returns equal struct.
- [x] 5.2 `pg_test!` `find_edges_by_caller_preserves_insertion_order` — 3 edges, ORDER BY id.
- [x] 5.3 `pg_test!` `find_edges_by_callee_returns_empty_vec_when_no_match`.
- [x] 5.4 `pg_test!` `count_edges_tracks_inserts` — 0 then 5.
- [x] 5.5 `pg_test!` `edge_query_uses_indexed_predicate` — 50 callers × 2 edges.
- [x] 5.6 `pg_test!` `edge_unparseable_provenance_falls_back_to_extracted`.
- [x] 5.7 `pg_test!` `edge_unparseable_dep_type_falls_back_to_calls`.
- [x] 5.8 `pg_test!` `remigration_preserves_existing_edges`.
- [x] 5.9 `pg_test!` `dyn_repository_edge_methods_work`.

## Phase 6: Visibility & Compilation Guards

- [x] 6.1 Compile-time check: `PostgresRepository::insert_edge` is `pub(crate)`.
- [x] 6.2 `cargo check --workspace` succeeds; `sqlx` NOT in dep graph; `EdgeMetadata` reachable.
- [x] 6.3 `cargo check --features postgres -p cognicode-core` clean, no new warnings.
- [x] 6.4 `cargo doc --no-deps -p cognicode-core` no new doc warnings.

## Phase 7: Final Verification

- [x] 7.1 `cargo test --workspace` (0 regressions) + `cargo test -p cognicode-core --features postgres -- --test-threads=1` with `TEST_DATABASE_URL` (all `pg_test!` pass).
- [x] 7.2 `cargo clippy --workspace --all-targets -- -D warnings` (and with `--features postgres`) + `cargo fmt --all -- --check` + `cargo doc --no-deps -p cognicode-core` clean.
- [x] 7.3 Column-set contract test (3.2) passes; column order matches spec.
