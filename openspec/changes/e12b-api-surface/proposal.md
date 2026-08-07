# Proposal: e12b — ApiSurface ViewExecutor

## Change ID: `e12b-api-surface`

## Intent

Convert `ApiSurface` from a catalogued `ViewKind` into a real wired `ViewExecutor`, making it reachable from the Explorer inspector pane. This is the second execution cycle of ADR-002 Phase 1 (ViewKind realization), following e12a-usage-examples.

## Scope

### In Scope

1. `ApiSurfaceExecutor` in `crates/cognicode-explorer/src/domain/views.rs`:
   - Applies to `Scope` (like `DependenciesExecutor` and `HotspotsExecutor`)
   - Returns a `ContextualView` with `ViewBlock::Table` showing all scope member symbols
   - Uses `InspectionTarget::Scope::symbols` (pre-populated by the inspection service)

2. Registration in `crates/cognicode-explorer/src/registry.rs` `REAL_EXECUTORS` map

3. Frontend: `ApiSurface` appears in the inspector view selector for scopes (ViewSpecWizard already maps `api_surface → table`)

### Out of Scope for e12b

- Visibility filtering (pub vs private) — `ResolvedSymbol` has no visibility field
- `ApiSurface` for individual files (would need a separate executor)
- Crate-level API surface (requires crate graph traversal)
- Any new renderer — `RendererKind::Table` is already wired in ViewSpecWizard

## Approach

1. Follow the proven `HotspotsExecutor` / `DependenciesExecutor` pattern
2. `ApiSurface` applies to `InspectionTarget::Scope` and renders all `scope.symbols` as a Table block
3. Table columns: `name`, `kind`, `file`, `line`
4. One block per scope (no subdivisions)
5. Graceful degradation when `graph_query` is `None`
6. V1 pragmatic: shows ALL symbols (pub + private) — visibility data not available in `ResolvedSymbol`

## Open Questions

| Question | Status | Resolution |
|---|---|---|
| Does `ResolvedSymbol` have a visibility field? | Resolved | No — V1 shows all symbols; visibility filter is a future improvement |
| Can we apply ApiSurface to individual files? | Deferred | File-level API surface is a separate executor if needed |
| Does the scope's symbol list include private symbols? | Resolved | Yes — all symbols in scope are listed; no visibility filter |

## Success Criteria

- [ ] `ApiSurfaceExecutor` registered in `REAL_EXECUTORS` map
- [ ] `api_surface` appears in inspector view selector for scopes
- [ ] Clicking `ApiSurface` shows scope symbols as a table
- [ ] `cargo test -p cognicode-explorer --lib` passes
- [ ] `npx vitest run` (vitest) passes
