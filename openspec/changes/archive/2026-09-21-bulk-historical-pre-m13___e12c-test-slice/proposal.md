# Proposal: e12c — TestSlice ViewExecutor

## Change ID: `e12c-test-slice`

## Intent

Convert `TestSlice` from a catalogued `ViewKind` into a real wired `ViewExecutor`, making it reachable from the Explorer inspector pane for `Symbol` objects. This is the third execution cycle of ADR-002 Phase 1.

## Scope

### In Scope

1. `TestSliceExecutor` in `crates/cognicode-explorer/src/domain/views.rs`:
   - Applies to `Symbol` (like UsageExamplesExecutor)
   - Returns a `ContextualView` with `ViewBlock::Table` showing test functions that call the symbol
   - Uses `GraphQueryPort::callers()` filtered by test-path heuristic

2. Registration in `crates/cognicode-explorer/src/registry.rs` `REAL_EXECUTORS` map

3. Frontend: `test_slice` already in ViewSpecWizard (maps to `table` renderer)

### Out of Scope for e12c

- Precise test identification via symbol metadata (requires `test` flag on `ResolvedSymbol`)
- TestSlice for Scope (would need broader caller traversal)
- Any new renderer — `RendererKind::Table` is already wired

## Approach

1. Follow the `UsageExamplesExecutor` pattern — same ports, similar structure
2. `TestSlice` applies to `Symbol` and shows callers that are tests
3. Test heuristic: a caller is a "test" if its `file` contains `_test` or `/tests/` or `/test/`
4. Table columns: `name`, `file`, `line`, `kind`
5. Graceful degradation when `graph_query` is `None`

## Open Questions

| Question | Status | Resolution |
|---|---|---|
| How to identify test functions without a `test` flag? | Resolved | V1: heuristic on file name/path (contains `_test` or `/tests/`) |
| Does GraphQueryPort return all callers including tests? | Resolved | Yes — callers() returns all callers; filter happens in build |

## Success Criteria

- [ ] `TestSliceExecutor` registered in `REAL_EXECUTORS` map
- [ ] `test_slice` appears in inspector view selector for symbols
- [ ] Clicking `TestSlice` shows test callers as a table
- [ ] `cargo test -p cognicode-explorer --lib` passes
- [ ] `npx vitest run` passes
