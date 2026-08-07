# Proposal: e12 — ViewKind Realization (Phase 1 Wave 1)

## Change ID: `e12-viewkind-realization`

## Intent

Convert `UsageExamples` from a catalogued `ViewKind` into a real wired `ViewExecutor`,
making it reachable from the Explorer inspector pane. This is the first execution
cycle of ADR-002 Phase 1.

## Scope

### In Scope

1. `UsageExamplesExecutor` in `crates/cognicode-explorer/src/domain/views.rs`:
   - Applies to `Symbol` (like `CallGraphExecutor`)
   - Returns a `ContextualView` with `ViewBlock::Table` showing callers and callees
   - Uses `GraphQueryPort::callees()` and `callers()` (already wired)

2. Registration in `crates/cognicode-explorer/src/registry.rs` `REAL_EXECUTORS` map

3. Frontend: `UsageExamples` appears in the inspector view selector for symbols

### Out of Scope for e12

- `ApiSurface`, `OwnershipMap`, `TestSlice`, `DebugSlice`, `DocCodeAlignment` — these are Phase 1
  Wave 1 but not e12. Each gets its own cycle after e12 demonstrates the pattern.
- ` Lepiter`-equivalent runtime (Phase 3)
- Universal Spotter expansion (Phase 2)

## Approach

1. Follow the existing `CallGraphExecutor` pattern (proven, ~100 LOC)
2. `UsageExamples` shows callers AND callees as a two-section table block
3. `renderer_kind: RendererKind::Table` — no new renderer needed
4. Frontend: no new renderer — reuse existing `Blocks` component with `Table` rendering

## Open Questions

| Question | Status | Resolution |
|---|---|---|
| Does `ResolvedSymbol` have visibility field for future `ApiSurface`? | Deferred | Check in spec phase |
| Are test symbols in the graph for `TestSlice`? | Deferred | Check in spec phase |

## Success Criteria

- [ ] `UsageExamples` executor is registered and returns real data for a symbol
- [ ] Inspector pane shows `UsageExamples` as a view option for symbols
- [ ] Clicking `UsageExamples` shows callers + callees in a table
- [ ] `just explorer-test` (vitest) passes
- [ ] `cargo test -p cognicode-explorer` passes
