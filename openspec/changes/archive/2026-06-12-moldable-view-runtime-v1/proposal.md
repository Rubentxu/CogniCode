# Proposal: moldable-view-runtime-v1

## Intent

CogniCode currently has hardcoded view builders (overview, callgraph, source, quality) in `domain/views.rs`. Each new view requires Rust code + recompilation. Users cannot create custom views from the Explorer. The Moldable View Runtime transforms CogniCode into a moldable dev environment: built-in views stay type-safe, but users add runtime views as declarative JSON (ViewSpecs) that render immediately without recompilation.

## Scope

### In Scope (7 deliverables)
- ViewKind, RendererKind, HierarchyKind enums as first-class domain vocabulary
- ViewSpec DTO with JSON Schema validation
- Backend ViewRegistry trait + linkme/distributed-slice discovery for built-in views
- Runtime ViewSpec store (Postgres CRUD + in-memory cache)
- Frontend RendererRegistry skeleton (Map<string, React component>)
- Explorer authoring flow: choose ViewKind → RendererKind → data source → preview → save
- EntryPointResolver default ViewKind mapping

### Out of Scope
- Extension host, WASM plugins, embedded scripting, Module Federation
- Full 30+ ViewKind implementation (phased; v1 implements 5-8 core views)
- C4 hierarchy extraction from code
- Executable snippets in project_diary/composed_narrative

## Capabilities

### New Capabilities
- `view-spec-domain`: Core ViewKind, RendererKind, HierarchyKind enums + ViewSpec DTO + JSON Schema validation
- `view-registry-backend`: Trait-object ViewRegistry with linkme discovery for built-in views + CRUD API for runtime ViewSpecs
- `renderer-registry-frontend`: Frontend Map<renderer_kind, ReactComponent> with fallback to raw JSON
- `viewspec-authoring-flow`: Explorer-first ViewSpec creation UX (wizard + live preview + save)
- `entry-point-resolver`: Typed entry point resolution with default ViewKind per ResolvedEntryPoint kind

### Modified Capabilities
- `contextual-views`: ViewDescriptor response must include both built-in and runtime ViewSpec views; ContextualView transport unchanged
- `named-view-persistence`: NamedView evolves toward ViewSpec shape; migration path needed

## Phased Roadmap

**Phase 0 — Domain Vocabulary (1-2 sessions)**: Define enums + ViewSpec DTO in Rust + TS. No behavior change. Zero-risk.

**Phase 1 — ViewRegistry Skeleton (2-3 sessions)**: Trait + linkme registration for existing views (overview, callgraph, source, quality). Existing behavior unchanged; views now discovered via registry instead of match arms.

**Phase 2 — Runtime ViewSpec Store (2-3 sessions)**: Postgres table + CRUD API + cache. Users can persist ViewSpecs. No authoring UX yet.

**Phase 3 — RendererRegistry (2-3 sessions)**: Frontend Map<id, component>. Existing ViewBlock switch → registry lookup. Fallback to raw JSON for unknown renderers.

**Phase 4 — Authoring Flow (3-4 sessions)**: Explorer wizard: pick ViewKind → RendererKind → MoldQL data source → JSONata transform → live preview → save.

**Phase 5 — EntryPointResolver (1-2 sessions)**: Map ResolvedEntryPoint kind → default ViewKind. Auto-opens best view.

## Entropy Budget (Protocol B)

**Method**: Heuristic (CogniCode not available for this analysis)

| Metric | Estimate | Threshold | Status |
|--------|----------|-----------|--------|
| H(Δ_existing) | ~0.8 bits (match arms → registry, NamedView migration) | < 1.0 | ✅ |
| H(Δ_new) | ~3.5 bits (5 new modules, 2 new DB tables) | > 0 | ✅ |
| New connascence pairs | 4 (ViewSpec↔ViewKind, ViewSpec↔RendererKind, Registry↔views, Store↔Postgres) | < 3 | ⚠️ |
| OCP compliant? | Yes — extension via registry, no existing view code modified | yes | ✅ |

**Breaking Change Indicators**: None. All existing views continue working through registry.

**Verdict**: 🟢 GREEN — Pure extension. New coupling is well-bounded (domain vocabulary is the shared contract).

## Auto-Grill Results (Step 3.5)

**Questions generated**: 8 | **Auto-resolved**: 6 | **Escalated**: 2

| # | Question | Resolution | Confidence |
|---|----------|------------|------------|
| 1 | Is linkme/distributed-slice stable for production? | Auto-resolved: linkme is used by bevy, tracing-subscriber. Stable. | 0.95 |
| 2 | Does existing ViewBlock shape conflict with ViewSpec? | Auto-resolved: ViewBlock is transport; ViewSpec is metadata. No conflict. | 0.90 |
| 3 | Should NamedView be replaced by ViewSpec or coexist? | **ESCALATED** | — |
| 4 | Is JSONata Rust crate mature enough? | Auto-resolved: jsonata-rs exists but is less maintained. Consider serde_json + custom transform. | 0.75 |
| 5 | Where does ViewSpec store live — same DB as graph? | Auto-resolved: Yes, Postgres. Separate table. | 0.95 |
| 6 | Does MoldQL need changes to support ViewSpec data sources? | Auto-resolved: MoldQL executor already returns MoldQLResultDto. ViewSpec data_source.query maps to existing executor. | 0.90 |
| 7 | Should ViewKind be an enum or string? | **ESCALATED** | — |
| 8 | How to handle unknown RendererKinds? | Auto-resolved: ADR-008 says "degrade to raw JSON or unsupported message". | 1.0 |

### Escalated Decisions

**Q3: NamedView vs ViewSpec**: NamedView is a saved graph projection (level, lens, focus_node, max_depth). ViewSpec is richer (view_kind, data_source, transform, renderer_kind, props). Options: (A) Migrate NamedView → ViewSpec, deprecate NamedView. (B) Keep both, link via id. (C) ViewSpec wraps NamedView as a special case. **Recommendation**: Option A — cleaner long-term, migration is straightforward since NamedView usage is low.

**Q7: ViewKind as enum vs string**: Enum gives compile-time safety + exhaustive match. String gives extensibility without recompilation. Options: (A) Enum with `#[serde(other)]` fallback. (B) String with validation list. (C) Enum for built-in + string for user-defined. **Recommendation**: Option A — best of both worlds, Rust pattern.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/domain/views.rs` | Modified | Existing view builders register via ViewRegistry instead of direct calls |
| `crates/cognicode-explorer/src/dto.rs` | Modified | Add ViewKind, RendererKind, HierarchyKind, ViewSpec DTOs |
| `crates/cognicode-explorer/src/api.rs` | Modified | New endpoints: GET/POST /viewspecs, GET /objects/:id/views includes runtime |
| `crates/cognicode-explorer/src/service.rs` | Modified | Service layer integrates ViewRegistry + ViewSpec store |
| `apps/explorer-ui/src/api/schemas.ts` | Modified | Add ViewSpec, ViewKind, RendererKind zod schemas |
| `apps/explorer-ui/src/components/ObjectInspector/ViewBlock.tsx` | Modified | Switch → RendererRegistry lookup |
| `apps/explorer-ui/src/hooks/useViews.ts` | Modified | Response includes runtime ViewSpecs |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| linkme not working on target platforms | Low | Fallback to manual registration (inventory crate or lazy_static) |
| JSONata Rust crate insufficient | Medium | Use serde_json + custom lightweight transform; defer JSONata to v2 |
| NamedView migration breaks saved views | Low | Migration script; keep NamedView as deprecated alias for 1 release |
| ViewSpec schema validation too strict | Medium | Start with lenient validation, tighten iteratively |

## Rollback Plan

- **Phase 0**: No rollback needed (additive enums only).
- **Phase 1**: ViewRegistry is additive; existing match arms kept as fallback. Remove registry → back to direct calls.
- **Phase 2**: ViewSpec store is new table; drop table to rollback.
- **Phase 3**: RendererRegistry wraps existing ViewBlock switch; remove registry → back to switch.
- **Phase 4-5**: Feature-flagged behind `MOLDABLE_VIEWS` env var. Disable → back to hardcoded views.

## Dependencies

- ADR-008 (Moldable View Runtime) — approved
- ADR-007 (No WASM in Browser) — approved
- ADR-009 (Hybrid Explorer Navigation) — approved
- Postgres instance (already available via cognicode-db)
- linkme crate (or inventory as fallback)
- serde_json (already in deps)

## Success Criteria

- [ ] ViewKind/RendererKind/HierarchyKind enums compile and serialize correctly
- [ ] Existing views discoverable via ViewRegistry (no hardcoded match arms in service layer)
- [ ] User can create a ViewSpec via API and see it listed alongside built-in views
- [ ] Frontend renders known RendererKinds via registry; unknown → raw JSON fallback
- [ ] Explorer authoring wizard creates a ViewSpec that persists and renders
- [ ] EntryPointResolver maps each ResolvedEntryPoint to a default ViewKind
- [ ] All existing tests pass (zero regressions)
