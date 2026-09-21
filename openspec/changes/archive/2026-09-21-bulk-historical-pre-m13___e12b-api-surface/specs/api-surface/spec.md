# Spec: e12b — ApiSurface ViewExecutor

## Purpose

Wire the `ApiSurface` ViewExecutor so it becomes reachable from the Explorer inspector pane for `Scope` objects. This is the second Phase 1 ViewKind realization per ADR-002.

`ApiSurface` answers: "what is the public API of this module/scope?" — showing all symbols defined within a scope as a navigable table. V1 shows all symbols (pub and private); visibility filtering is deferred.

---

## ADDED Requirements

### Requirement: 1. ApiSurfaceExecutor — Scope

The `ApiSurfaceExecutor` MUST be added to the `ViewRegistry` in `crates/cognicode-explorer/src/registry.rs`.

The executor MUST apply only to `InspectionTarget::Scope`.

### Requirement: 2. build_api_surface Function

The `build_api_surface` function in `crates/cognicode-explorer/src/domain/views.rs`
MUST return a `ContextualView` containing:

1. A **Single table block** (`ViewBlock` with `id: "api_surface"`, `title: "{scope_path}"`):
   - Table with columns: `name`, `kind`, `file`, `line`
   - One row per symbol in `scope.symbols`
   - Rows ordered by symbol name (lexicographic)

2. The `view_id` MUST be `"api-surface"`.
3. The `view_kind` MUST be `ViewKind::ApiSurface`.
4. The `renderer_kind` MUST be `RendererKind::Table`.

#### Scenario: Scope with symbols

- GIVEN `InspectionTarget::Scope { path: "src/lib.rs", symbols: [fn_a, fn_b, type_c] }` with 3 symbols
- WHEN `build_api_surface` is called
- THEN the returned `ContextualView` has 1 block with 3 rows
- AND block title is `"src/lib.rs"` (scope path)
- AND columns are `["name", "kind", "file", "line"]`

#### Scenario: Scope with no symbols (empty module)

- GIVEN `InspectionTarget::Scope { path: "src/empty.rs", symbols: [] }` with 0 symbols
- WHEN `build_api_surface` is called
- THEN the block has 0 rows (empty table)
- AND no error is returned (graceful degradation)

#### Scenario: Scope inspection where graph_query is unavailable

- GIVEN `graph_query: None`
- WHEN `build_api_surface` is called
- THEN the block renders all symbols from `InspectionTarget::Scope::symbols`
- AND no error is returned (symbols come from Scope, not GraphQueryPort)

### Requirement: 3. Registration

The executor MUST be registered in `registry.rs` `REAL_EXECUTORS` map with key `"api-surface"`.

### Requirement: 4. Frontend View Selector

The inspector pane's view selector already shows `"api_surface"` as an available view for scopes (via ViewSpecWizard `VIEW_KIND_GROUPS.Development` and `VIEW_KIND_DEFAULT_RENDERER.api_surface`). No frontend changes required.

---

## UNCHANGED Requirements

- The `DependenciesExecutor` and `HotspotsExecutor` continue to work as-is.
- The `ViewRegistry.known_view_kinds()` continues to list `ApiSurface` in the catalog.
- All existing tests pass.

---

## Implementation Notes

- Follow the pattern of `HotspotsExecutor` / `build_scope_hotspots` exactly.
- `InspectionTarget::Scope::symbols` is provided by the inspection service — no new port needed.
- `InspectionTarget::Scope::path` provides the scope path for the block title.
- The `ViewBlock` body for a table should be:
  ```json
  { "columns": ["name", "kind", "file", "line"], "rows": [...] }
  ```
- The frontend `Table` renderer already handles `{ columns, rows }` shape — no new renderer needed.
- No `graph_query` access needed — symbols come directly from the scope target.

---

## Acceptance Criteria

- [ ] `ApiSurfaceExecutor` registered in `REAL_EXECUTORS` map.
- [ ] `api_surface` appears in inspector view selector for scopes.
- [ ] Clicking `ApiSurface` shows scope symbols as a table.
- [ ] Empty scope shows empty table (no error).
- [ ] `cargo test -p cognicode-explorer --lib` passes.
- [ ] `npx vitest run` passes.
