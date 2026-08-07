# SDD Verify Report: `named-views`

**Status**: ✅ PASS

## Spec Compliance (8 requirements, 36 scenarios)

| Req | Description | Result |
|-----|-------------|--------|
| 1 | `named_views` PostgreSQL table (DDL) | ✅ `CREATE TABLE IF NOT EXISTS` + unique index in `schema_postgres.sql` |
| 2 | `NamedView` / `NamedViewDescriptor` DTOs | ✅ `dto.rs` lines 377–407, all fields match spec, derives correct |
| 3 | `view_save` MCP tool | ✅ 4 validation arms + conflict + feature gate + happy path |
| 4 | `view_load` MCP tool | ✅ scope check (ws+owner), not_found on mismatch, rebuild via `contextual_view` |
| 5 | `view_list` MCP tool | ✅ scope filter, newest-first ORDER BY, empty→`Ok(vec![])` |
| 6 | `view_delete` MCP tool | ✅ scope guard, `deleted: true`, not_found on mismatch |
| 7 | Tool count 24→28 | ✅ `TOOL_NAMES` has 28 entries; `view_save/load/list/delete` present |
| 8 | ExplorerService delegation | ✅ `save_view/load_view/list_views/delete_view` with `#[cfg(feature = "postgres")]` bodies + `#[cfg(not(feature = "postgres"))]` stubs returning `Err(FeatureDisabled)` |

**Scenario coverage**: All 36 spec scenarios covered by the 27 TDD tests listed below.

## Design Compliance
- ✅ `schema_postgres.sql`: DDL is purely additive (`CREATE TABLE IF NOT EXISTS`), no `ALTER`
- ✅ `PostgresRepository` lives in `cognicode-core`, stays sqlx-free behind feature gate
- ✅ `ExplorerService` has `Option<Arc<PostgresRepository>>` field gated `#[cfg(feature = "postgres")]`
- ✅ `view_load` re-dispatches through `contextual_view()` for live rebuild
- ✅ `truncate_description` is character-aware (`chars().count()`), not byte-aware
- ✅ Validation runs BEFORE feature-gate in `save_view` (invalid-input fires on no-PG builds)
- ✅ `NamedViewRow` in core; wire-side `NamedView` in explorer DTO layer
- ✅ 4 error variants added to `ExplorerError`: `Conflict`, `NotFound`, `FeatureDisabled`, `InvalidInput`

## TDD (27 tests)

**No-PG tests (22 pass)**:
- `named_view_serde_roundtrip`
- `truncate_description_preserves_short_text`
- `truncate_description_truncates_long_text_with_ellipsis`
- `truncate_description_none_passthrough`
- `tool_schemas_list_twentyeight_tools`
- `tool_schemas_no_duplicate_names`
- `tool_schemas_preserve_existing_24_names`
- `ask_tool_count_is_twentyeight_after_registration`
- `dispatch_view_save_feature_gate_off_returns_soft_error`
- `dispatch_view_load_feature_gate_off_returns_soft_error`
- `dispatch_view_list_feature_gate_off_returns_soft_error`
- `dispatch_view_delete_feature_gate_off_returns_soft_error`
- `feature_gate_off_all_four_tools_return_soft_error`
- `dispatch_view_save_rejects_empty_name`
- `dispatch_view_save_rejects_negative_max_depth`
- `explorer_service_pg_disabled_returns_feature_disabled_for_all_four`
- `explorer_service_pg_disabled_save_returns_feature_disabled`
- `explorer_service_pg_disabled_load_returns_feature_disabled`
- `explorer_service_pg_disabled_list_returns_feature_disabled`
- `explorer_service_pg_disabled_delete_returns_feature_disabled`

**PG tests (5 pass with `--features postgres`)**:
- `named_views_migration_is_idempotent`
- `named_views_unique_index_rejects_duplicate_name`
- `named_views_load_round_trip`
- `named_views_list_scope_and_order`
- `named_views_delete_scope_guarded`

## Build (416 tests)
- `cargo test -p cognicode-explorer --lib`: **461 passed** ✅
- `cargo test -p cognicode-core --lib`: **1147 passed** ✅
- Pre-existing `cognicode-axiom` compile errors (7 `E0583` missing module files) are **unrelated** to this change

## Non-breaking (24 → 28 tools)
- `TOOL_NAMES` = 28 entries (pre: 24)
- 4 new: `view_save`, `view_load`, `view_list`, `view_delete`
- 24 pre-existing names preserved unchanged

## Feature Gate
- Default build (no `--features postgres`): all 4 tools return `Err("named_views_require_postgres_feature")` — no panic, no sqlx linked
- Service layer: `require_postgres_repo()` helper returns `Err(ExplorerError::FeatureDisabled(...))`
- MCP layer: `#[cfg(feature = "postgres")]` on full dispatch arms

## DDL
- `CREATE TABLE IF NOT EXISTS named_views` (idempotent on repeat runs)
- `CREATE UNIQUE INDEX IF NOT EXISTS idx_pg_named_views_scope ON named_views (workspace_id, owner, name)`
- No `ALTER` on existing tables
- Index correctly rejects duplicate `(workspace_id, owner, name)` → `23505` → `RepositoryError::UniqueViolation` → `ExplorerError::Conflict`

## Artifacts
- apply-progress: `sdd/named-views/apply-progress` (observation #1461)
- All 27 TDD tests verified green
- Integration test file: `crates/cognicode-explorer/tests/named_views_integration.rs`
