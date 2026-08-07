# Design: Moldable View Runtime v1

## Technical Approach

Additive domain vocabulary + registry skeleton. Phase 0 introduces `ViewKind`, `RendererKind`, `HierarchyKind`, and `ViewSpec` DTOs with zero behavior change. Phase 1 wraps existing view builders in a `ViewRegistry` trait-object pattern (mirroring `LensRegistry`) so views are discovered, not hardcoded. Existing `ContextualView`, `ViewBlock`, and `ViewDescriptor` transport types are untouched.

## Architecture Decisions

| Decision | Option | Tradeoff | Choice |
|----------|--------|----------|--------|
| ViewKind type | Enum with `#[serde(other)]` | Compile-time safety + forward compat | ✅ Chosen |
| | String with validation | More flexible but loses exhaustiveness | Rejected |
| Registry pattern | Trait-object `HashMap` (like `LensRegistry`) | OCP-compliant, no recompilation for new views | ✅ Chosen |
| | linkme/distributed-slice | Better for static discovery; added dependency | Deferred to v1.1 |
| NamedView migration | Migrate to ViewSpec, deprecate NamedView | One-time migration script | ✅ Chosen |
| | Keep both | Duplicates persistence logic | Rejected |
| Frontend registry | Map<string, Component> with fallback | Simple, degrades gracefully | ✅ Chosen |
| | Replace ViewBlock switch | Too risky for v1 | Rejected |

## Data Flow

```
User selects view
    ↓
ExplorerService::available_views → ViewRegistry::applicable_to
    ↓
User clicks view
    ↓
ExplorerService::contextual_view → ViewRegistry::build → ContextualView
    ↓
Frontend: RendererRegistry (for ViewSpec) or ViewBlock switch (built-in)
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/dto.rs` | Modify | Add `ViewKind`, `RendererKind`, `HierarchyKind`, `ViewSpec` DTOs |
| `crates/cognicode-explorer/src/domain/view_registry.rs` | Create | `ViewBuilder` trait, `ViewContext`, `ViewRegistry` |
| `crates/cognicode-explorer/src/domain/mod.rs` | Modify | Re-export `view_registry` |
| `crates/cognicode-explorer/src/service.rs` | Modify | Wire `ViewRegistry`, delegate `available_views` and `contextual_view` |
| `crates/cognicode-explorer/src/domain/views.rs` | Modify | Wrap existing builders in `ViewBuilder` impls |
| `apps/explorer-ui/src/api/schemas.ts` | Modify | Add Zod schemas for new DTOs |
| `apps/explorer-ui/src/components/rendererRegistry.ts` | Create | Frontend `Map<RendererKind, React.FC>` with fallback |
| `apps/explorer-ui/src/components/ObjectInspector/ViewBlock.tsx` | Modify | Keep switch; add registry path for ViewSpec blocks |

## Interfaces / Contracts

```rust
// Rust: ViewBuilder trait
pub trait ViewBuilder: Send + Sync {
    fn id(&self) -> &str;
    fn descriptor(&self) -> ViewDescriptor;
    fn applies_to(&self, object_type: &InspectableObjectType) -> bool;
    fn build(&self, object_id: &str, ctx: &ViewContext) -> ExplorerResult<ContextualView>;
}

// Rust: ViewSpec DTO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewSpec {
    pub id: String,
    pub title: String,
    pub applies_to: InspectableObjectType,
    pub view_kind: ViewKind,
    pub data_source: ViewSpecDataSource,
    pub transform: Option<ViewSpecTransform>,
    pub renderer_kind: RendererKind,
    pub props: Option<serde_json::Value>,
}
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `ViewKind`, `RendererKind` serde roundtrip | `#[test]` in `dto.rs` |
| Unit | `ViewRegistry` register/get/applicable_to | `#[test]` in `view_registry.rs` |
| Unit | `ViewBuilder` wrappers for existing views | `#[test]` in `views.rs` |
| Integration | `available_views` includes registry views | Service tests in `service.rs` |
| E2E | Frontend schema validation | `schemas.test.ts` |

## Migration / Rollout

- NamedView: add `#[deprecated]` alias; migration script converts `level/lens/focus_node` to `ViewSpec` with `view_kind` mapping.
- Rollout: Feature-flag `MOLDABLE_VIEWS` env var for Phases 3-5; Phases 0-1 are always safe.

## Open Questions

- [ ] How to map all 30+ `ViewKind` variants to existing view builders in v1?
- [ ] Should `ViewContext` include `graph_repo` for multimodal views?
- [ ] Exact JSONata fallback strategy if Rust crate is insufficient?

## Entropy Analysis

**Method**: Heuristic

**Design Quality Score**: 0.47/1.0 (🟡 ACCEPTABLE)

| Interface | I(X;T) Leakage | I(T;Y) Coverage | Bottleneck Quality | SOLID Check |
|-----------|---------------|-----------------|-------------------|-------------|
| `ViewBuilder` | LOW | HIGH | ✅ Optimal | SRP ✅ DIP ✅ |
| `ViewContext` | MED | HIGH | ⚠️ Review | SRP ✅ DIP ✅ |
| `ViewRegistry` | LOW | HIGH | ✅ Optimal | SRP ✅ DIP ✅ |
| `ViewSpec` | LOW | HIGH | ✅ Optimal | SRP ✅ DIP ✅ |
| `ViewKind` | LOW | HIGH | ✅ Optimal | SRP ✅ DIP ✅ |

**Connascence pairs**: 5 total, all < 1.0 bits. No critical pairs.

**OCP**: H(Δ_existing) ≈ 0.8 bits. ✅ Compliant.

## Auto-Grill Results

**Input**: Design approach for moldable-view-runtime-v1
**Preguntas**: 8 | **Auto-resueltas**: 8 (100%) | **Escaladas**: 0
**Reporte**: `openspec/changes/moldable-view-runtime-v1/reports/auto-grill.html`
**Reporte temporal**: `/tmp/sdd-moldable-view-runtime-v1-auto-grill.html`

### Auto-Resolved Decisions
- ViewBuilder wraps existing functions (no replacement)
- ViewRegistry pattern is valid (mirrors LensRegistry)
- NamedView migration is safe (low usage)
- Frontend RendererRegistry reuses existing renderers

### Escalated Decisions
None.

### Documentation Updates
- CONTEXT.md: No changes needed
- docs/adr/: No new ADRs needed

### Status
all_resolved
