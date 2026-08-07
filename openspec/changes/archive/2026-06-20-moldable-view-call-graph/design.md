# Kernel Design: moldable-view-call-graph

**Status:** Designed — pending sdd-kernel-tasks
**Date:** 2026-06-20
**Author:** sdd-kernel-design
**Context quality:** C2 | **Taxonomy:** routing-gap, schema-stamp, persistence, breaking-change

## Context Reuse Check
| Input | Status | Notes |
|-------|--------|-------|
| Knowledge coverage | present | ADR-040, roadmap MOLDABLE-VIEW-PANE-STATE-2026, UX wireframes, CONTEXT.md; delta specs both present |
| Exploration | present | explore-report.md reused (13/13 grill decisions, gap analysis, file list) |
| Proposal/spec alignment | ok | Corrected Task 0 (backend stamp) reused; one spec typo flagged below |
| Code verification | ok | dto.rs:194-213, facades/view.rs:278-298, facades/mod.rs:132/140, schemas.ts:784-799, PaneInspector.tsx:118/225-234, domain/views.rs:1227-1241, registry.rs:345, types.ts:33-76 |
| Context quality | C2 | deepen — no re-exploration |
| Problem taxonomy | present | routing-gap + schema-stamp (Epic 1), persistence + breaking-change (Epic 2) |
| Domain language | present | GraphViewRenderer, ViewKind routing, Pane Stack, Exploration Snapshot, ViewportState, PaneSnapshot — all resolved |
| Recommended effort | deepen | shaped depth: contracts + entropy + migration |

**Corrections to upstream claims (do NOT propagate):**
1. No `get_object_view` exists → stamping site is **`contextual_view`** (`facades/view.rs:278`, trait `ViewService::contextual_view` `facades/mod.rs:140`).
2. Executors implement `ViewDescriptor` at **`domain/views.rs:1227`** (`ViewExecutor: ViewDescriptor`, `view_kind()`+`renderer_kind()`); `registry.rs:130` is the *separate* `ViewDescriptorProvider` metadata trait — cite the correct one.
3. `#[derive(Default)]` on `ContextualView` (dto.rs:194) conflicts with a `view_kind: ViewKind` field (enum has no `Default`) → see Architecture Decision AD-4.
4. Spec typo: graph-view-renderer spec "missing descriptor metadata" says `view_kind SHALL default to RendererKind::Json` — that is a **type error** (`ViewKind ≠ RendererKind`). Intent = safe non-graph fallback; routing falls through to `<Blocks>` for any non-graph kind.

## Technical Approach
Two epics. **Epic 1 (bug fix):** backend stamps `view_kind`+`renderer_kind` from the executor descriptor onto the DTO after `build()`; frontend `PaneInspector` early-returns to `GraphViewRenderer` when `view_kind ∈ {call_graph, dependency_graph, data_flow, impact_radius, seam_map}`. **Epic 2 (persistence):** add `ViewportState` to `Pane`, capture pan/zoom in `SvgGraph`, extend backend `ExplorationSession` with `panes: Vec<PaneSnapshot>` (breaking, no default), hybrid localStorage + server save.

## Knowledge Impact
- Durable artifacts reused: ADR-040, roadmap, UX wireframes, CONTEXT.md Navigation/Visualization terms.
- Artifacts that may need supersession: ADR-040 addendum required for the schema-gap correction (Task 0 widened to backend+frontend). graph-view-renderer spec typo needs a one-line fix.
- Memory-only learnings consulted: None — all facts ADR-backed.

## Applied Lenses
| Lens | Delegation | Status | Why Applied | Design Impact |
|------|------------|--------|-------------|---------------|
| base-discipline | kernel | applied | Always active | Surfaced 4 upstream inaccuracies; routed to corrected sites |
| entropy-sdd | skill | deepened | New coupling in well-isolated seam | Connascence of name (GraphViewRenderer↔SvgGraph, PaneInspector↔isGraphViewKind); 1 new branch — low entropy (see Entropy Constraints) |
| cognicode-sdd | skill | verified | Taxonomy: schema-stamp | Confirmed gap is backend, not frontend; `get_executor` returns `&'static dyn ViewExecutor` ⇒ stamping via vtable is valid |
| interface-design | explore heuristic | deepened | New component contracts | Fixed GraphViewRenderer props; `ViewportState`/`PaneSnapshot` shapes; predicate location |

## Invariants And Constraints
| Invariant / Constraint | Enforcement Point | Verification |
|------------------------|-------------------|--------------|
| `MAX_PANES = 8` cap | `paneStack.ts` | spec "click on node opens new pane" regression |
| `SELECT_OBJECT` dedup by `objectId` | `paneStack.ts` | spec same scenario |
| `ExplorationSession.panes` NO `#[serde(default)]` | dto.rs | spec "legacy session deserialization fails" |
| Routing set is closed (5 kinds) | `isGraphViewKind` predicate | spec "GraphViewRenderer routes graph views" |
| Backend always stamps after `build()` | `contextual_view` | spec "get_object_view returns view_kind" |

## Architecture Decisions
| Decision | Choice | Alternatives Considered | Rationale |
|----------|--------|-------------------------|-----------|
| AD-1 routing key | `view_kind` (semantic) stamped on DTO | derive from `renderer_kind==="graph"` | ADR-008 separates ViewKind/RendererKind; semantic intent is the routing authority |
| AD-2 stamp site | mutate DTO after `executor.build()` in `contextual_view` | stamp inside each executor's `build()` | single seam; executors stay metadata-free; matches spec scenario |
| AD-3 DTO view_kind default | `#[serde(default)]` + add `Default` for `ViewKind` (neutral non-graph value) | make field required / remove `#[derive(Default)]` | preserves legacy-deser safety; non-graph default ⇒ Blocks fallback; avoids touching every `ContextualView::default()` site |
| AD-4 early-return placement | after `display` computed, before `<LoadingTier>` — **bypasses header/ViewTabs** | inline inside body panel | grill Decision 1/2 (full canvas). ⚠ see Open Question Q-1 (view-switch regression) |
| AD-5 layout memo deps | `useMemo(layoutFromContextualView, [object_id, blocks])` | deep-compare blocks | perf budget <50ms; SWR refetch risk mitigated by JSDoc + stability test |
| AD-6 breaking persistence | `panes` required, no default | serde default + migration | grill Decision 13 / ADR-040 §8 — intentional, localStorage is user mitigation |

## Data Flow
```
contextual_view(object_id, view_id)
  → executor = registry.get_executor(view_id)   // &'static dyn ViewExecutor
  → view = executor.build(&ctx).await?
  → view.view_kind = executor.view_kind()        // AD-2 stamp
  → view.renderer_kind = executor.renderer_kind()
  → JSON { ..., view_kind: "call_graph" }

PaneInspector: display = view ?? _activeView
  → isGraphViewKind(display.view_kind)? → <GraphViewRenderer view objectId onClose/>
       → useMemo(layoutFromContextualView, [object_id, blocks])
       → nodes<=1 ? <GraphEmptyState/> : <SvgGraph onSelectObject=dispatch(SELECT_OBJECT)/>
  : <Blocks view={display}/>

SvgGraph pan/zoom → onViewportChange → dispatch(UPDATE_PANE_VIEWPORT) → pane.viewport
Save → snapshot(state.panes) → POST /api/explorations/session {panes:[PaneSnapshot]}
Load → hydrate panes (last active) → SvgGraph applies saved viewport
```

## File Changes
| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/dto.rs` | modify | add `view_kind: ViewKind` (+`#[serde(default)]`, `Default` impl) to `ContextualView` (194-213); add `ViewportState{x,y,scale}` + `PaneSnapshot{pane_id,object_id,view_id,scroll_y,viewport:Option}`; add `panes: Vec<PaneSnapshot>` (NO default) to `ExplorationSession` (384); mirror `SaveExplorationSessionRequest` |
| `crates/cognicode-explorer/src/facades/view.rs` | modify | in `contextual_view` (278-298): `let mut view = executor.build(&ctx).await?; view.view_kind = executor.view_kind(); view.renderer_kind = executor.renderer_kind(); Ok(view)` |
| `crates/cognicode-explorer/src/facades/persistence.rs` + `mod.rs` | modify | `save_exploration_session` accepts/stores `panes` |
| `crates/cognicode-explorer/src/api.rs` | modify | validate `panes`; 422 on malformed/legacy payload |
| `apps/explorer-ui/src/api/schemas.ts` | modify | add `view_kind: viewKindSchema.optional()` to `contextualViewSchema` (784-799); add `paneSnapshotSchema`, `viewportStateSchema`, `panes` to session schema |
| `apps/explorer-ui/src/mocks/handlers.ts` | modify | include `view_kind` in fixtures |
| `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx` | modify | `isGraphViewKind()` + early-return after line 118, before line 123 |
| `apps/explorer-ui/src/components/GraphView/GraphViewRenderer.tsx` | create | generic renderer: memoized layout, empty state, SELECT_OBJECT on click |
| `apps/explorer-ui/src/components/GraphView/GraphEmptyState.tsx` + `index.ts` | create | empty state + barrel |
| `apps/explorer-ui/src/components/SvgGraph/SvgGraph.tsx` | modify | optional `onViewportChange?(v)` (no-op default) |
| `apps/explorer-ui/src/state/navigation/types.ts` | modify | `ViewportState`; `Pane.viewport?`; `UPDATE_PANE_VIEWPORT` action |
| `apps/explorer-ui/src/state/navigation/paneStack.ts` | modify | handle `UPDATE_PANE_VIEWPORT` |
| `apps/explorer-ui/src/hooks/useExplorations*` | modify | capture `panes` snapshot on save; hydrate on load |
| `apps/explorer-ui/src/components/SvgGraph/GraphEdge.tsx` | modify | label only when `highlighted` (Decision 6) |

## Interfaces / Contracts
```rust
// dto.rs
pub struct ContextualView {
    pub object_id: String, pub view_id: String, pub title: String,
    #[serde(default)] pub view_kind: ViewKind,        // AD-3
    #[serde(default)] pub renderer_kind: RendererKind,
    pub blocks: Vec<ViewBlock>, pub relations: Vec<TypedRelation>,
    pub evidence: Vec<EvidenceBlock>, #[serde(default)] pub findings: Vec<DesignFinding>,
}
pub struct ViewportState { pub x: f32, pub y: f32, pub scale: f32 }
pub struct PaneSnapshot { pub pane_id: String, pub object_id: String,
    pub view_id: String, pub scroll_y: f32, pub viewport: Option<ViewportState> }
pub struct ExplorationSession { /* id, workspace_id, events, navigation_mode */
    pub panes: Vec<PaneSnapshot>,  // NO #[serde(default)] (AD-6)
    pub created_at: String }
```
```typescript
// types.ts
export interface ViewportState { x: number; y: number; scale: number }
export interface Pane { /* ...existing */ scrollY: number; viewport?: ViewportState }
| { type: "UPDATE_PANE_VIEWPORT"; payload: { paneId: string; viewport: ViewportState } }
// schemas.ts: view_kind: viewKindSchema.optional()
// GraphViewRenderer.tsx
interface Props { view: ContextualView; objectId: string; onClose?: () => void }
const layout = useMemo(() => layoutFromContextualView(view), [view.object_id, view.blocks]);
```

## Entropy Constraints
| Interface/Module | Risk | Constraint |
|------------------|------|------------|
| PaneInspector↔isGraphViewKind | routing set drift | keep set in one named predicate; unit-test all 5 kinds |
| ContextualView↔view_kind | stale routing if unstamped | stamp is the single contract; integration test asserts non-empty for call_graph |
| GraphViewRenderer↔SvgGraph | layout type coupling | shared `LayoutResult` type; memo deps documented |
| ExplorationSession.panes | silent data loss | no default; explicit error "missing field: panes" |
| SvgGraph↔navigation | generic component coupled to pane state | `onViewportChange` optional, no-op default |

**Performance budgets:** layout <50ms (10 nodes); memo hit <1ms; SELECT_OBJECT <5ms; SvgGraph render <16ms; localStorage save <100ms; server save <500ms; load <1000ms; bundle <20KB gzipped (React.lazy code-split).

## Testing Strategy
| Layer | What To Test | Approach |
|-------|--------------|----------|
| Vitest | routing: Blocks for non-graph, GraphViewRenderer for each of 5 kinds; early-return before LoadingTier | `PaneInspector.test.tsx` (new) |
| Vitest | layout memo stability; empty state ≤1 node; click dispatches SELECT_OBJECT w/ viewId preserved | `GraphViewRenderer.test.tsx` (new) |
| Vitest | `UPDATE_PANE_VIEWPORT` reducer; ViewportState round-trip | extend `paneStack.test.ts` |
| Playwright | `call-graph-ready` golden (SVG non-blank); `call-graph-empty`; pane-stack-multi (no dup) | `e2e/call-graph-rendering.spec.ts` (new) |
| Playwright | save with viewport → payload contains viewport; load restores | `e2e/exploration-persistence.spec.ts` (new) |
| Rust unit | ContextualView serializes `view_kind`; ExplorationSession serializes `panes`; legacy (no panes) fails deser | extend dto.rs test module |

## Migration / Rollout
**No backward compatibility** (AD-6/Decision 13). Old `ExplorationSession` JSON invalid (422). Frontend `view_kind.optional()` keeps legacy ContextualView payloads safe (undefined ⇒ Blocks). Steps: (1) merge behind `MOLDABLE_VIEW_ENABLED` flag; (2) run existing `pane-stack.spec.ts`/`visual-regression.spec.ts` after Task 1.2, regenerate snapshots; (3) staging smoke; (4) CHANGELOG breaking-change entry + user-facing warning; (5) enable. Rollback: revert Task 0 + early-return ⇒ ContextualView without `view_kind` falls through to Blocks (pre-change behavior).

## Open Questions
- **Q-1 (seam decision):** AD-4 early-return bypasses `ViewTabs`/`SuggestionStrip` — users cannot switch views *within* a graph pane. Options: (a) accept full-canvas (grill intent), (b) render `ViewTabs` before the early-return. **Recommend (b)** to preserve view-switching; needs tasks-phase confirmation. Non-blocking — does not affect contracts.
- Q-2: confirm `ViewKind` neutral `Default` value (AD-3) — propose `VerticalSlice`; tasks phase finalizes.
