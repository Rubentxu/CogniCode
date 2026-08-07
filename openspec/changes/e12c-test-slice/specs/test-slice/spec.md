# Spec: e12c — TestSlice ViewExecutor

## Purpose

Wire the `TestSlice` ViewExecutor so it becomes reachable from the Explorer inspector pane for `Symbol` objects. This is the third Phase 1 ViewKind realization per ADR-002.

`TestSlice` answers: "what tests exercise this symbol?" — finding test functions that call the inspected symbol. V1 uses a file-path heuristic to identify tests.

---

## ADDED Requirements

### Requirement: 1. TestSliceExecutor — Symbol

The `TestSliceExecutor` MUST be added to the `ViewRegistry` in `crates/cognicode-explorer/src/registry.rs`.

The executor MUST apply only to `InspectionTarget::Symbol`.

### Requirement: 2. build_test_slice Function

The `build_test_slice` function in `crates/cognicode-explorer/src/domain/views.rs`
MUST return a `ContextualView` containing:

1. A **Single table block** (`ViewBlock` with `id: "test_slice"`, `title: "Tests ({count})"`):
   - Table with columns: `name`, `file`, `line`, `kind`
   - One row per caller that is identified as a test
   - Rows ordered by file, then by line number

2. The `view_id` MUST be `"test-slice"`.
3. The `view_kind` MUST be `ViewKind::TestSlice`.
4. The `renderer_kind` MUST be `RendererKind::Table`.

### Requirement: 3. Test Identification Heuristic

A `RelationTarget` is classified as a TEST if its `file` satisfies any of:
- Contains `/tests/` (e.g. `src/utils/tests/helpers.rs`)
- Contains `/test/` as a path segment (e.g. `test/unit/core_test.rs`)
- File name ends with `_test.rs` or starts with `test_`
- File name ends with `.test.ts` or `.test.tsx` (JS/TS)

Non-matching callers are excluded from the result.

### Requirement: 4. Registration

The executor MUST be registered in `registry.rs` `REAL_EXECUTORS` map with key `"test-slice"`.

### Requirement: 5. Frontend View Selector

The inspector pane's view selector already shows `"test_slice"` as an available view (via ViewSpecWizard `VIEW_KIND_GROUPS.Development` and `VIEW_KIND_DEFAULT_RENDERER.test_slice`). No frontend changes required.

---

## UNCHANGED Requirements

- The `UsageExamplesExecutor` and `ApiSurfaceExecutor` continue to work as-is.
- The `ViewRegistry.known_view_kinds()` continues to list `TestSlice` in the catalog.
- All existing tests pass.

---

## Implementation Notes

- Follow the pattern of `UsageExamplesExecutor` / `build_usage_examples` exactly.
- `graph_query.callers(&symbol.id)` provides all callers.
- Apply the test-path filter after collecting callers.
- `build_test_slice(symbol, graph_query)` is the function signature.
- The `ViewBlock` body for a table should be:
  ```json
  { "columns": ["name", "file", "line", "kind"], "rows": [...] }
  ```

---

## Acceptance Criteria

- [ ] `TestSliceExecutor` registered in `REAL_EXECUTORS` map.
- [ ] `test_slice` appears in inspector view selector for symbols.
- [ ] Clicking `TestSlice` shows test callers as a table.
- [ ] Symbols with no test callers show empty table (no error).
- [ ] `cargo test -p cognicode-explorer --lib` passes.
- [ ] `npx vitest run` passes.
