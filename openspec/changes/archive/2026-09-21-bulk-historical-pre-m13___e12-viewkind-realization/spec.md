# Spec: e12 — UsageExamples ViewExecutor

## Purpose

Wire the `UsageExamples` ViewExecutor so it becomes reachable from the Explorer
inspector pane. This is the first Phase 1 ViewKind realization per ADR-002.

`UsageExamples` answers the question: "where is this function/type used, and what
does it call?" — the natural complement to `CallGraph`, which shows the same
information as a visual graph. `UsageExamples` shows it as a navigable table.

---

## ADDED Requirements

### Requirement: 1. UsageExamplesExecutor — Scope

The `UsageExamplesExecutor` MUST be added to the `ViewRegistry` in
`crates/cognicode-explorer/src/registry.rs`.

The executor MUST apply only to `InspectionTarget::Symbol`.

### Requirement: 2. build_usage_examples Function

The `build_usage_examples` function in `crates/cognicode-explorer/src/domain/views.rs`
MUST return a `ContextualView` containing:

1. A **Callers block** (`ViewBlock` with `id: "callers"`, `title: "Called by"`):
   - Table with columns: `name`, `file`, `line`, `kind`
   - One row per caller symbol
   - Populated via `graph_query.callers(&symbol.id)`

2. A **Callees block** (`ViewBlock` with `id: "callees"`, `title: "Calls"`):
   - Table with columns: `name`, `file`, `line`, `kind`
   - One row per callee symbol
   - Populated via `graph_query.callees(&symbol.id)`

3. The `view_id` MUST be `"usage-examples"`.
4. The `view_kind` MUST be `ViewKind::UsageExamples`.
5. The `renderer_kind` MUST be `RendererKind::Table`.

#### Scenario: Symbol with callers and callees

- GIVEN `InspectionTarget::Symbol(UserService::create_user)` with 3 callers and 2 callees
- WHEN `build_usage_examples` is called
- THEN the returned `ContextualView` has 2 blocks: callers (3 rows) and callees (2 rows)
- AND both blocks use `RendererKind::Table`

#### Scenario: Symbol with no callers (export/interface)

- GIVEN `InspectionTarget::Symbol` with 0 callers and 5 callees
- WHEN `build_usage_examples` is called
- THEN the callers block has 0 rows (empty table)
- AND the callees block has 5 rows

#### Scenario: Symbol with no callees (leaf/terminal)

- GIVEN `InspectionTarget::Symbol` with 2 callers and 0 callees
- WHEN `build_usage_examples` is called
- THEN the callers block has 2 rows
- AND the callees block has 0 rows (empty table)

#### Scenario: GraphQueryPort unavailable (mock/legacy path)

- GIVEN `graph_query: None`
- WHEN `build_usage_examples` is called
- THEN both callers and callees blocks have 0 rows
- AND no error is returned (graceful degradation)

### Requirement: 3. Registration

The executor MUST be registered in `registry.rs` `REAL_EXECUTORS` map with key
`"usage-examples"`.

### Requirement: 4. Frontend View Selector

The inspector pane's view selector MUST show `"usage-examples"` as an available
view for `Symbol` objects.

---

## UNCHANGED Requirements

- The `CallGraphExecutor` continues to work as-is.
- The `ViewRegistry.known_view_kinds()` continues to list `UsageExamples` in the catalog.
- All existing tests pass.

---

## Implementation Notes

- Follow the pattern of `CallGraphExecutor` / `build_callgraph` exactly.
- `graph_query.callers(&symbol.id)` and `graph_query.callees(&symbol.id)` are
  already available via the `GraphQueryPort` trait.
- `ResolvedSymbol` fields available for the table: `id`, `name`, `file`, `line`, `kind`.
- The `ViewBlock` body for a table should be a JSON object:
  ```json
  { "columns": ["name", "file", "line", "kind"], "rows": [...] }
  ```
- The frontend `Table` renderer (`apps/explorer-ui/src/components/rendererRegistry.tsx`)
  already handles `{ columns, rows }` shape — no new renderer needed.

---

## Acceptance Criteria

- [ ] `UsageExamplesExecutor` registered in `REAL_EXECUTORS` map.
- [ ] `UsageExamples` appears in inspector view selector for symbols.
- [ ] Clicking `UsageExamples` shows callers + callees as a table.
- [ ] Empty callers/callees shows empty table (no error).
- [ ] `cargo test -p cognicode-explorer` passes.
- [ ] `just explorer-test` (vitest) passes.
