# Proposal: e14-narrative-runtime — Narrative Embed Resolver + Executors (Cycle 1)

## Intent
ADR-002 Phase 3 mandates `ProjectDiary` and `ExampleObject` runtime executors. Both are catalog-only (ViewKind enum, no executor/provider/shaper). `ComposedNarrative` is done. Cycle 1 delivers embed-based narrative executors with `EmbedResolver` wired from day one — fixing the prior failure where EmbedResolver was built but never wired (mem #4285: 8/16 spec scenarios unimplemented).

## Scope

### In Scope
- `EmbedResolver` — resolves `!view(kind, params)` markers in markdown → live `ViewBlock` children
- Wire `EmbedResolver` into `build_investigation_narrative` + new shapers from the start
- `ProjectDiaryExecutor` — ViewDescriptor + ViewExecutor + pure shaper (chronological investigation timeline)
- `ExampleObjectExecutor` — ViewDescriptor + ViewExecutor + pure shaper (curated code+explanation artifact)
- Register both in `REAL_EXECUTORS` map
- 4+ unit tests per executor; integration test proving embed → ViewBlock chain
- ADR-002 line 234: strike stale "Postgres" reference

### Out of Scope
- LadybugDB persistence (new `NarrativeStore` port) — deferred to Cycle 2
- Executable code snippets (Option B) — deferred to Phase 4+
- Frontend narrative block renderers — fall through to `UnknownBlockView` (matches existing `ComposedNarrative` behavior)

## Capabilities

> CONTRACT with sddk-spec. Research `openspec/specs/` before filling in.

### New Capabilities
- `narrative-embed-resolver`: `EmbedResolver` resolves `!view(kind, key=value, ...)` markers in narrative markdown blocks to live child `ViewBlock` entries. Pure (no I/O), wired into ALL narrative shapers.

### Modified Capabilities
- `contextual-views`: narrative `ViewBlock` entries SHALL support embedded view references via `EmbedResolver`, producing child blocks dynamically
- `view-registry-backend`: two new entries in `REAL_EXECUTORS` map (`project-diary`, `example-object`) — additive registry wiring, no trait/interface changes

## Approach
**Runtime model: Option A (markdown+embedded views)** from exploration. Embed resolver + pure shapers = deterministic output from pre-resolved data. No new port/store/adapter this cycle.

Pattern: follow `ComposedNarrativeExecutor`. Pure shaper → `ViewDescriptor` → `ViewExecutor` → provider → `REAL_EXECUTORS`.

**Key risk mitigation**: EmbedResolver wired into ALL narrative shapers in the same cycle — the prior e14 attempt built it but never wired it into `build_project_diary_view`. Integration test proves end-to-end.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/domain/views.rs` | Modified | Add `EmbedResolver`, `build_project_diary()`, `build_example_object()`, wire into `build_investigation_narrative`, two executor impls + providers + statics |
| `crates/cognicode-explorer/src/registry.rs` | Modified | Register `ProjectDiaryExecutor` + `ExampleObjectExecutor` in `REAL_EXECUTORS` |
| `docs/adr/ADR-002-moldable-exploration-parity-program.md` | Modified | Strike stale "Postgres" ref line 234; note deferred LadybugDB persistence |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Prior wiring failure repeat | Medium | EmbedResolver wired into ALL narrative shapers in same cycle; integration test proves end-to-end |
| Embed marker parsing fragile | Low | Grammar `!view(kind, key=value, ...)` — unit test all parse permutations |
| Frontend `UnknownBlockView` fallback | High | Acceptable — matches existing `ComposedNarrative` behavior; renderer deferred |

## Rollback Plan
Remove executor impls, providers, statics from `views.rs`. Remove `REAL_EXECUTORS` entries. Remove `EmbedResolver` calls from narrative shapers. No DB migration, no port change — pure code revert. Revert ADR-002 doc edit.

## Dependencies
- `ComposedNarrativeExecutor` (shipped) — establishes the executor pattern
- `ViewContext` with `SourceReader` (already present) — needed by `ExampleObjectExecutor`
- `ViewRegistry::REAL_EXECUTORS` (stable structure) — zero change needed to the map itself

## Success Criteria
- [ ] `EmbedResolver` resolves `!view(moldql, query=...)` → valid child `ViewBlock` entries
- [ ] `ProjectDiaryExecutor::build()` returns non-empty `ContextualView` for `SavedExploration` / `Investigation`
- [ ] `ExampleObjectExecutor::build()` returns non-empty `ContextualView` for `Symbol` / `File`
- [ ] Both executors appear in `GET /api/views` listing
- [ ] `cargo test -p cognicode-explorer` passes all new + existing tests
- [ ] ADR-002 line 234 no longer references Postgres
