# Tasks: Named Views

Decision needed before apply: No
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: High
Estimated changed lines: ~500 across 8 files. 3 PRs stacked to main: PR 1 (no sqlx, Phase 1) → PR 2 (PG, Phase 2, rebase on PR 1) → PR 3 (Phase 3, rebase on PR 2).

## Phase 1: Foundation (PR 1)

- [ ] 1.1 Append `named_views` DDL + unique index to `crates/cognicode-core/src/infrastructure/persistence/schema_postgres.sql`
- [ ] 1.2 RED: `named_views_migration_is_idempotent` in postgres_repository tests (DDL twice → 1 table, 1 index)
- [ ] 1.3 Add `Conflict`, `NotFound`, `FeatureDisabled`, `InvalidInput` variants to `ExplorerError` (`crates/cognicode-explorer/src/error.rs`)
- [ ] 1.4 Add `NamedView`, `NamedViewDescriptor` to `crates/cognicode-explorer/src/dto.rs` (serde + Debug/Clone/PartialEq/Eq)
- [ ] 1.5 RED: `named_view_serde_roundtrip` in `dto.rs` (to_string+from_str equality)
- [ ] 1.6 GREEN: `cargo build -p cognicode-explorer` compiles + 1.2/1.5 PASS

## Phase 2: Repository (PR 2)

- [ ] 2.1 Add `save_named_view(&NamedView)` to `PostgresRepository`; map PG `23505` → `RepositoryError::UniqueViolation`
- [ ] 2.2 Add `load_named_view(id, ws, owner) -> Option<NamedView>` (scope in WHERE)
- [ ] 2.3 Add `list_named_views(ws, owner) -> Vec<NamedView>` ordered by `created_at DESC`
- [ ] 2.4 Add `delete_named_view(id, ws, owner) -> bool` (true iff row existed)
- [ ] 2.5 RED: `named_views_unique_index_rejects_duplicate_name` in postgres_repository tests
- [ ] 2.6 Extend `open_graph_from_postgres` in `postgres_bridge.rs` to also return `Arc<PostgresRepository>`
- [ ] 2.7 Validate: 2.5 PASS + `cargo build -p cognicode-explorer --features postgres`

## Phase 3: Service + MCP (PR 3)

- [ ] 3.1 Add `postgres_repo: Option<Arc<PostgresRepository>>` field to `ExplorerService` (`#[cfg(feature="postgres")]`)
- [ ] 3.2 Add `save_view`, `load_view`, `list_views`, `delete_view` (off → `Err(FeatureDisabled)`; load re-invokes `contextual_view`)
- [ ] 3.3 Add `truncate_description(s, max) -> String` helper (≤max + `…` only when truncated)
- [ ] 3.4 RED: `explorer_service_pg_disabled_returns_feature_disabled` in `service.rs` tests
- [ ] 3.5 Add `TOOL_VIEW_SAVE/LOAD/LIST/DELETE` constants in `mcp.rs`
- [ ] 3.6 Add `ViewSaveArgs/LoadArgs/ListArgs/DeleteArgs` (`serde(Deserialize)`)
- [ ] 3.7 Add 4 dispatch arms; map errors → `invalid_input`/`named_view_already_exists`/`not_found`/`named_views_require_postgres_feature`
- [ ] 3.8 Add 4 `Tool` schema entries; extend `TOOL_NAMES` 24→28
- [ ] 3.9 Rename `tool_schemas_list_twentyfour_tools` → `tool_schemas_list_twentyeight_tools`; assert 28 (lines ~1789, ~2983, ~3783)
- [ ] 3.10 RED: `tool_schemas_no_duplicate_names`, `tool_schemas_preserve_existing_24_names`
- [ ] 3.11 Create `crates/cognicode-explorer/tests/named_views_integration.rs` (reusing `fresh_test_url()` from `pg_bridge_contract.rs`)
- [ ] 3.12 RED save: happy path, duplicate→conflict, empty name, negative `max_depth`
- [ ] 3.13 RED load: rebuild equals direct `build_*`, unknown id, workspace mismatch
- [ ] 3.14 RED list: scope filter, empty scope→`Ok(vec![])`, newest-first order
- [ ] 3.15 RED delete: removes row, scope mismatch leaves row
- [ ] 3.16 RED gate-off: `feature_gate_off_all_four_tools_return_soft_error` (default features)
- [ ] 3.17 GREEN: all RED tests in 3.4, 3.10–3.16 pass
- [ ] 3.18 Add "Named View" entry to `docs/explorer-graph/glossary.md`
- [ ] 3.19 Validate: 3.17 PASS + `cargo test --workspace`

## Dependencies

Phase 1 → 2 → 3 strict. 1.1 blocks 2.x. 2.1–2.4 block 3.1–3.4. 3.1–3.4 block 3.5–3.10. 3.5–3.10 block 3.11–3.16.

## Validation

No-PG: `cargo test -p cognicode-explorer --lib named_view_serde_roundtrip tool_schemas_list_twentyeight_tools tool_schemas_no_duplicate_names tool_schemas_preserve_existing_24_names explorer_service_pg_disabled_returns_feature_disabled feature_gate_off_all_four_tools_return_soft_error`. PG (`TEST_DATABASE_URL`): `cargo test -p cognicode-core named_views_migration_is_idempotent named_views_unique_index_rejects_duplicate_name` + `cargo test -p cognicode-explorer --test named_views_integration --features postgres` + `cargo test --workspace`.
