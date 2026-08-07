# Tasks — moldable-view-call-graph

**Status:** Planned — pending sdd-kernel-apply
**Date:** 2026-06-20
**Total estimated effort:** ~13-15.5h (corrected for backend schema stamp)
**Author:** sdd-kernel-tasks

## Router Context Used
- **Knowledge Coverage:** sufficient (ADR-040, roadmap MOLDABLE-VIEW-PANE-STATE-2026, UX wireframes, CONTEXT.md, delta specs both present)
- **Context Quality:** C2
- **Taxonomy:** routing-gap, schema-stamp, persistence, breaking-change
- **Invariants Driving Tasks:** `MAX_PANES=8` cap, `SELECT_OBJECT` dedup by `objectId`, `ExplorationSession.panes` has NO `#[serde(default)]`, routing set is closed (5 kinds)
- **Recommended Effort:** deepen — shaped depth: contracts + entropy + migration
- **Resolved Q-1 (seam decision):** Early-return placement = Option B (after ViewTabs, before body) — preserves view-switching within graph pane

## Corrections From Previous Phases (verified against code)
1. `get_object_view()` → actual method is `contextual_view()` in `ViewService` trait (`facades/view.rs:278`)
2. `ViewDescriptor` trait lives at `domain/views.rs:1227` (`ViewExecutor: ViewDescriptor`), NOT `registry.rs:118/130` (that is a separate `ViewDescriptorProvider` trait)
3. `ContextualView` has `#[derive(Default)]` at `dto.rs:194` — `view_kind: ViewKind` field needs `#[serde(default)]` + manual `Default` impl because enum has no `Default`
4. Spec type error fix: `view_kind` defaults to `ViewKind::Custom("unknown")` (NOT `RendererKind::Json` — different types)
5. Q-1 resolved: early-return placement = **Option B** (after ViewTabs, before body) — better UX, minimal extra work

## Review Budget Forecast
- **Estimated changed lines:** medium-high (~600-800 lines: ~150 backend Rust + ~300 frontend TSX/TS + ~150 tests + ~50 schema/types)
- **400-line budget risk:** High — backend DTO + frontend component + tests + e2e all in one PR is borderline
- **Chained PRs recommended:** Yes — split into 3 stacked PRs:
  1. **PR1 — Task 0 (schema stamp):** T1 + T4 + T5 (backend stamp + Zod schema + MSW). Foundational, no UX change.
  2. **PR2 — Epic 1 (GraphViewRenderer):** T2 + T3 + T11a (call-graph-rendering.spec.ts only). Visible bug fix.
  3. **PR3 — Epic 2 (Exploration Snapshot):** T6 + T7 + T8 + T9 + T10 + T11b (exploration-persistence.spec.ts) + T12 (docs). Persistence layer.
- **Decision needed before apply:** Yes — confirm chained-PR split or accept single-PR risk

## Knowledge Traceability
- **Work item source artifacts:**
  - Proposal: `openspec/changes/moldable-view-call-graph/proposal.md` (Capabilities: graph-view-routing, exploration-snapshot; Approach)
  - Explore: `openspec/changes/moldable-view-call-graph/explore-report.md` (13/13 grill decisions, gap analysis, file list)
  - Delta specs: `specs/graph-view-renderer/spec.md`, `specs/exploration-snapshot/spec.md`
  - Design: `openspec/changes/moldable-view-call-graph/design.md` (AD-1..AD-6, data flow, contracts)
- **Ownership source:** grill session 2026-06-20 (13/13 decisions resolved, ADR-backed); ADR-040 §8 (no backward compat)
- **Open knowledge gaps affecting execution:** None — schema gap resolved by corrected Task 0 (backend stamp + frontend schema + MSW)

## Dependency Graph

```
T1 (Zod schema) ──┬──> T2 (GraphViewRenderer) ──> T3 (routing) ──> T11a (e2e call-graph) ──┐
                  │                                                                             │
                  └──> T4 (backend stamp) ─────────────────────────────────────────────────────┤
                                                                                                │
                  └──> T5 (MSW fixtures) ───────────────────────────────────────────────────────┤
                                                                                                │
T6 (ViewportState) ──> T7 (capture viewport) ──> T8 (backend PaneSnapshot) ──> T11b (e2e persist) ──> T12 (docs)
                                                      │
                                                      └──> T9 (localStorage) ──> T10 (server save/load)
```

## Task List (ordered)

### T1 — Schema gap: add view_kind to Zod schema
**File:** `apps/explorer-ui/src/api/schemas.ts` (line 784-799)

**Change:**
```typescript
export const contextualViewSchema = z.object({
  object_id: z.string(),
  view_id: z.string(),
  title: z.string(),
  view_kind: viewKindSchema.optional(),  // NEW (was missing — ADR-040 routing key)
  renderer_kind: rendererKindSchema.default("json"),
  blocks: z.array(viewBlockAnySchema),
  relations: z.array(typedRelationSchema),
  evidence: z.array(evidenceBlockSchema),
  findings: z.array(designFindingSchema).default([]),
});
```

**Verification:**
```bash
cd apps/explorer-ui && npm run typecheck
```

**Commit:** `feat(explorer-ui): add view_kind to ContextualView Zod schema`

**Effort:** 15 min

---

### T2 — Create GraphViewRenderer component (spike + TDD)

**Files:**
- `apps/explorer-ui/src/components/GraphView/GraphViewRenderer.tsx` (NEW)
- `apps/explorer-ui/src/components/GraphView/GraphEmptyState.tsx` (NEW)
- `apps/explorer-ui/src/components/GraphView/GraphLoadingSkeleton.tsx` (NEW — optional)
- `apps/explorer-ui/src/components/GraphView/index.ts` (NEW — barrel export)
- `apps/explorer-ui/src/components/GraphView/GraphViewRenderer.test.tsx` (NEW)

**TDD order:**
1. Write failing tests in `GraphViewRenderer.test.tsx`
2. Implement minimal component to pass tests
3. Refactor

**Tests:**
- `renders empty state when nodes.length <= 1`
- `renders SvgGraph when nodes.length > 1`
- `dispatches SELECT_OBJECT with viewId on node click`
- `memoizes layout with useMemo([object_id, blocks])`
- `does not recalculate layout on re-render with same deps`

**Implementation sketch:**
```typescript
// apps/explorer-ui/src/components/GraphView/GraphViewRenderer.tsx
import { useMemo } from "react";
import { useAppDispatch } from "../../state/context";
import type { ContextualView } from "../../api/types";
import { SvgGraph } from "../SvgGraph/SvgGraph";
import { layoutFromContextualView } from "../../mocks/layoutMock";
import { GraphEmptyState } from "./GraphEmptyState";

interface Props {
  view: ContextualView;
  objectId: string;
  onClose?: () => void;
}

export function GraphViewRenderer({ view, objectId, onClose }: Props) {
  const dispatch = useAppDispatch();

  const layout = useMemo(
    () => layoutFromContextualView(view),
    [view.object_id, view.blocks]
  );

  if (layout.nodes.length <= 1) {
    return <GraphEmptyState />;
  }

  return (
    <div data-testid="graph-view-renderer" className="flex h-full flex-col">
      <header className="flex items-center justify-between px-4 py-2">
        <h2>{view.title}</h2>
        {onClose && (
          <button
            type="button"
            onClick={onClose}
            data-testid="graph-view-close"
            aria-label="Close pane"
          >
            ✕
          </button>
        )}
      </header>
      <SvgGraph
        layout={layout}
        selectedId={objectId}
        onSelectObject={(nodeId) => {
          dispatch({
            type: "SELECT_OBJECT",
            payload: { objectId: nodeId, viewId: view.view_id },
          });
        }}
      />
    </div>
  );
}
```

**Commit:** `feat(explorer-ui): add GraphViewRenderer component with TDD`

**Effort:** 2h (1h tests, 1h impl)

---

### T3 — Add routing in PaneInspector (Option B: ViewTabs before early-return)

**File:** `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx`

**Change:** Add early-return AFTER ViewTabs rendering (around line 214, before `<Blocks>` body at line 225):

```typescript
// After display computed (line 118) and ViewTabs rendered (line ~214)
// ...

// Helper function (module scope)
function isGraphViewKind(kind: string | undefined): boolean {
  return (
    kind === "call_graph" ||
    kind === "dependency_graph" ||
    kind === "data_flow" ||
    kind === "impact_radius" ||
    kind === "seam_map"
  );
}

// In render body, after ViewTabs but before <Blocks>:
if (display && isGraphViewKind(display.view_kind)) {
  return (
    <>
      {/* ViewTabs already rendered above — preserved per Option B (Q-1) */}
      <div data-testid="object-inspector-body" className="flex-1 overflow-y-auto">
        <GraphViewRenderer
          view={display}
          objectId={objectId}
          onClose={onClose}
        />
      </div>
    </>
  );
}
```

**Tests:** Update `PaneInspector.test.tsx`:
- Renders `<GraphViewRenderer>` for `call_graph` view_kind
- Renders `<GraphViewRenderer>` for each of 5 graph kinds (parameterized)
- Renders `<Blocks>` for non-graph view_kind
- ViewTabs render in BOTH cases (Option B seam — verified by Q-1)

**Commit:** `feat(explorer-ui): route graph views to GraphViewRenderer in PaneInspector`

**Effort:** 1h (30 min impl, 30 min tests)

---

### T4 — Backend: stamp view_kind and renderer_kind in ContextualView DTO

**Files:**
- `crates/cognicode-explorer/src/dto.rs` (struct changes — line 194-213)
- `crates/cognicode-explorer/src/facades/view.rs` (stamp after build — line 278)
- `crates/cognicode-explorer/src/domain/views.rs` (Default impl for ViewKind if needed — line 1227+)

**Change in `dto.rs`:**
```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextualView {
    pub object_id: String,
    pub view_id: String,
    pub title: String,
    /// Semantic intent (ADR-008 §ViewKind). Required for routing by PaneInspector.
    /// `#[serde(default)]` because ViewKind enum has no Default — see AD-3.
    /// Field is conceptually required for new payloads.
    #[serde(default = "default_view_kind")]
    pub view_kind: ViewKind,
    #[serde(default)]
    pub renderer_kind: RendererKind,
    pub blocks: Vec<ViewBlock>,
    pub relations: Vec<TypedRelation>,
    pub evidence: Vec<EvidenceBlock>,
    #[serde(default)]
    pub findings: Vec<DesignFinding>,
}

fn default_view_kind() -> ViewKind {
    ViewKind::Custom("unknown".to_string())
}
```

**Change in `facades/view.rs` (contextual_view method, line 278):**
```rust
async fn contextual_view(&self, req: ContextualViewRequest) -> ExplorerResult<ContextualView> {
    let descriptor = self.registry.get(&req.view_id).await?;
    let mut view = executor.build(&ctx).await?;
    // AD-2: stamp descriptor metadata onto DTO at single seam
    view.view_kind = executor.view_kind();
    view.renderer_kind = executor.renderer_kind();
    Ok(view)
}
```

**Tests:** Add Rust unit tests in `dto.rs` test module:
- `ContextualView` serialization includes `view_kind`
- Legacy `ContextualView` without `view_kind` deserializes with default (`Custom("unknown")`)
- `contextual_view_returns_view_kind_stamped` integration test in `facades/view.rs:703` (extend existing tests)

**Verification:**
```bash
cargo test -p cognicode-explorer
```

**Commit:** `feat(explorer-api): stamp view_kind and renderer_kind from ViewDescriptor`

**Effort:** 1.5h (1h impl, 30 min tests)

---

### T5 — Update MSW handlers to include view_kind

**File:** `apps/explorer-ui/src/mocks/handlers.ts`

**Change:** Add `view_kind` to all view fixtures returned by `/api/workspaces/{id}/objects/{id}/view/{viewId}`:

```typescript
http.get("/api/workspaces/:workspaceId/objects/:objectId/view/:viewId", ({ params }) => {
  const viewKind = viewIdToViewKind(params.viewId as string);
  return HttpResponse.json({
    object_id: params.objectId,
    view_id: params.viewId,
    title: `View ${params.viewId}`,
    view_kind: viewKind,  // NEW — unblocks frontend routing in dev
    renderer_kind: viewIdToRendererKind(params.viewId as string),
    blocks: [...],
    relations: [],
    evidence: [],
    findings: [],
  });
}),

// Helper: map known view ids to their ViewKind
function viewIdToViewKind(viewId: string): string {
  const map: Record<string, string> = {
    call_graph: "call_graph",
    dependency_graph: "dependency_graph",
    overview: "overview",
    // ... extend as ViewSpecs are added
  };
  return map[viewId] ?? "custom";
}
```

**Commit:** `feat(explorer-ui): add view_kind to MSW fixtures`

**Effort:** 30 min

---

### T6 — Extend Pane type with ViewportState

**File:** `apps/explorer-ui/src/state/navigation/types.ts` (line 33-76)

**Change:**
```typescript
export interface ViewportState {
  x: number;
  y: number;
  scale: number;
}

export interface Pane {
  // ... existing fields (line 33-39)
  scrollY: number;
  viewport?: ViewportState;  // NEW
}

// Extend NavigationAction union (line 75)
| { type: "UPDATE_PANE_VIEWPORT"; payload: { paneId: string; viewport: ViewportState } }
```

**Implementation in `paneStack.ts`:** Handle `UPDATE_PANE_VIEWPORT` reducer case:
```typescript
case "UPDATE_PANE_VIEWPORT": {
  return {
    ...state,
    panes: state.panes.map((pane) =>
      pane.paneId === action.payload.paneId
        ? { ...pane, viewport: action.payload.viewport }
        : pane
    ),
  };
}
```

**Tests:** Extend `paneStack.test.ts`:
- `UPDATE_PANE_VIEWPORT updates the matching pane's viewport`
- `UPDATE_PANE_VIEWPORT ignores unknown paneId`

**Commit:** `feat(explorer-ui): add ViewportState to Pane type and UPDATE_PANE_VIEWPORT action`

**Effort:** 30 min (15 min types + 15 min reducer + tests)

---

### T7 — Capture viewport in SvgGraph

**File:** `apps/explorer-ui/src/components/SvgGraph/SvgGraph.tsx`

**Change:** Add optional `paneId` and `onViewportChange` props; dispatch on pan/zoom:

```typescript
interface SvgGraphProps {
  layout: LayoutResult;
  selectedId?: string;
  onSelectObject?: (objectId: string) => void;
  paneId?: string;                              // NEW (optional)
  onViewportChange?: (v: ViewportState) => void; // NEW (optional, no-op default)
}

export function SvgGraph({
  layout,
  selectedId,
  onSelectObject,
  paneId,
  onViewportChange = () => {},  // no-op default keeps SvgGraph generic
}: SvgGraphProps) {
  const dispatch = useAppDispatch();

  const handleViewChange = useCallback(
    (newView: { x: number; y: number; scale: number }) => {
      const viewport = { x: newView.x, y: newView.y, scale: newView.scale };
      // Local callback (used by GraphViewRenderer)
      onViewportChange(viewport);
      // Navigation state (only when paneId provided — avoids coupling)
      if (paneId) {
        dispatch({
          type: "UPDATE_PANE_VIEWPORT",
          payload: { paneId, viewport },
        });
      }
    },
    [dispatch, paneId, onViewportChange]
  );

  // ... wire to onPointerUp and onWheel handlers
}
```

**Wire-up in `GraphViewRenderer.tsx`:** pass `paneId` prop down to `SvgGraph`.

**Commit:** `feat(explorer-ui): capture viewport state in SvgGraph on pan/zoom`

**Effort:** 1h

---

### T8 — Backend: add PaneSnapshot and ViewportState to ExplorationSession

**File:** `crates/cognicode-explorer/src/dto.rs` (line 384-394)

**Change:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewportState {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneSnapshot {
    pub pane_id: String,
    pub object_id: String,
    pub view_id: String,
    pub scroll_y: f32,
    pub viewport: Option<ViewportState>,
}

pub struct ExplorationSession {
    pub id: String,
    pub workspace_id: String,
    pub events: Vec<ExplorationEvent>,
    #[serde(default = "default_navigation_mode")]
    pub navigation_mode: String,
    pub created_at: String,
    /// NEW (REQUIRED, no serde default — Decision 13 / AD-6 / ADR-040 §8)
    pub panes: Vec<PaneSnapshot>,
}
```

**Mirror in `SaveExplorationSessionRequest`** so clients can submit panes on save.

**Tests:** Add Rust unit tests in `dto.rs`:
- `ExplorationSession` serialization includes `panes`
- Legacy `ExplorationSession` without `panes` FAILS to deserialize (verifies no `#[serde(default)]`)
- `PaneSnapshot` round-trip preserves `viewport`

**Update `facades/persistence.rs` + `facades/mod.rs`:** `save_exploration_session` accepts/stores `panes` (see explore-report §4.1).

**Update `facades/view.rs` / `api.rs`:** validate `panes`; return 422 on malformed/legacy payload.

**Verification:**
```bash
cargo test -p cognicode-explorer
```

**Commit:** `feat(explorer-api): add panes field to ExplorationSession (breaking, no default)`

**Effort:** 2h (1h DTO + 30 min persistence + 30 min api validation + 30 min tests)

---

### T9 — Frontend: capture snapshot to localStorage on navigation

**File:** `apps/explorer-ui/src/hooks/useExplorations.ts` (extend)

**Change:** Add `useSnapshotCache` hook that writes to localStorage on every pane change:

```typescript
export function useSnapshotCache(
  workspaceId: string | null,
  sessionId: string,
  panes: Pane[]
) {
  useEffect(() => {
    if (!workspaceId || panes.length === 0) return;
    const key = `cognicode.exploration.snapshot.${workspaceId}.${sessionId}`;
    try {
      localStorage.setItem(key, JSON.stringify(panes));
    } catch (e) {
      console.warn("Failed to cache snapshot to localStorage", e);
    }
  }, [workspaceId, sessionId, panes]);
}
```

**Hydration on load:** `useEffect` reads localStorage on mount, validates with `paneSnapshotSchema.parse()`, hydrates `state.navigation.panes` if present; silently ignores legacy entries.

**Commit:** `feat(explorer-ui): cache exploration snapshot to localStorage`

**Effort:** 1h

---

### T10 — Frontend: save/load snapshot to server

**File:** `apps/explorer-ui/src/hooks/useExplorations.ts` (extend)

**Change:** Update `saveExploration` to include `panes`:

```typescript
export async function saveExploration(
  request: unknown,
  panes: Pane[]
): Promise<ExplorationSession> {
  const parsed = saveExplorationSessionRequestSchema.parse(request);
  const session = await apiPost(
    "/explorations/session",
    { ...parsed, panes: panes.map(paneToSnapshot) },
    explorationSessionSchema
  );
  return session;
}

function paneToSnapshot(pane: Pane): PaneSnapshot {
  return {
    pane_id: pane.paneId,
    object_id: pane.objectId,
    view_id: pane.viewId,
    scroll_y: pane.scrollY,
    viewport: pane.viewport,
  };
}
```

**Add `paneSnapshotSchema` + `viewportStateSchema`** in `apps/explorer-ui/src/api/schemas.ts` to match Rust DTO.

**Commit:** `feat(explorer-ui): save/load exploration snapshot with panes to server`

**Effort:** 1h

---

### T11 — Playwright e2e tests + visual regression (split into T11a + T11b for chained PRs)

#### T11a — Call graph rendering e2e (Epic 1)
**File:** `apps/explorer-ui/e2e/call-graph-rendering.spec.ts` (NEW)

**Tests:**
- `call-graph-ready`: Golden image `call-graph-rendered.png` validates SVG renders (≥2 nodes, non-blank)
- `call-graph-empty`: Empty state for isolated symbols (`nodes.length <= 1`)
- `pane-stack-multi`: Click node opens new pane (Pane Stack — preserves D6 invariant)
- `pane-stack-dedup`: Selecting existing objectId activates that pane (no duplicates)

**Verification:**
```bash
cd apps/explorer-ui && npm run test:e2e:visual -- call-graph-rendering.spec.ts
```

**Commit:** `test(explorer-ui): add Playwright e2e tests for call graph rendering`

**Effort:** 2h

#### T11b — Exploration persistence e2e (Epic 2)
**File:** `apps/explorer-ui/e2e/exploration-persistence.spec.ts` (NEW)

**Tests:**
- `save-exploration-with-viewport`: Zoom graph → save → verify payload contains viewport
- `load-exploration-restores-viewport`: Save with viewport → reload → verify viewport restored
- `legacy-localstorage-ignored`: Old localStorage without panes → silently ignored, fresh empty PaneStack
- `mcp-degraded-exploration`: MCP tools can read saved exploration (degraded non-visual fidelity per ADR-040 §9)

**Verification:**
```bash
cd apps/explorer-ui && npm run test:e2e:visual -- exploration-persistence.spec.ts
```

**Commit:** `test(explorer-ui): add Playwright e2e tests for exploration persistence`

**Effort:** 2h

---

### T12 — Edge label highlight-only fix (Decision 6 alignment)

**File:** `apps/explorer-ui/src/components/SvgGraph/GraphEdge.tsx` (line 53)

**Change:** Render label only when `highlighted === true`:
```typescript
// Before (line 53): always renders label if present
// After:
{edge.label && edge.highlighted && (
  <text ...>{edge.label}</text>
)}
```

**Audit `SvgGraph.tsx:301`** to ensure `highlighted` prop is passed through to `GraphEdge` for hovered/selected edges.

**Tests:** Extend `SvgGraph.test.tsx`:
- `GraphEdge renders label only when highlighted=true`
- `GraphEdge omits label when highlighted=false`

**Commit:** `fix(explorer-ui): render graph edge labels only when highlighted (Decision 6)`

**Effort:** 30 min

---

### T13 — Documentation update (ADR-040 addendum + CHANGELOG + README)

**Files:**
- `docs/adr/ADR-040-graph-view-renderer.md` (mark as Implemented + addendum for schema gap correction)
- `CHANGELOG.md` (add breaking change entry for `ExplorationSession.panes`)
- `apps/explorer-ui/src/components/GraphView/README.md` (NEW — component usage docs)

**CHANGELOG entry:**
```
## [Unreleased]
### Breaking Changes
- `ExplorationSession.panes` is now required. Sessions saved before v0.X.0 will fail to load (422). Use localStorage export if you need to migrate. See ADR-040 §8.

### Features
- GraphViewRenderer: dedicated renderer for call_graph / dependency_graph / data_flow / impact_radius / seam_map views (ADR-040).
- Exploration Snapshot: pane stack with viewport state (pan/zoom) persists per session.
```

**Commit:** `docs(moldable-view): mark ADR-040 as implemented, add CHANGELOG entry, README for GraphViewRenderer`

**Effort:** 30 min

---

## Execution Order

**Wave 1 — Schema stamp (PR1, ~2.5h):**
1. **T1** — Schema gap (Zod view_kind)
2. **T4** — Backend stamp (ContextualView + facades/view.rs)
3. **T5** — MSW fixtures (parallel with T1/T4)

**Wave 2 — GraphViewRenderer (PR2, ~4h):**
4. **T2** — GraphViewRenderer component (TDD)
5. **T3** — PaneInspector routing (Option B seam)
6. **T11a** — Call graph e2e + visual regression
7. **T12** — Edge label highlight-only fix

**Wave 3 — Exploration Snapshot (PR3, ~7-8h):**
8. **T6** — ViewportState type + UPDATE_PANE_VIEWPORT action
9. **T7** — Capture viewport in SvgGraph
10. **T8** — Backend PaneSnapshot (breaking change)
11. **T9** — localStorage cache
12. **T10** — Server save/load
13. **T11b** — Exploration persistence e2e
14. **T13** — Documentation

## Commit Strategy

Each task = 1 atomic conventional commit.
- Branch: `feat/moldable-view-call-graph`
- PR1: Tasks T1, T4, T5 (schema stamp) — ~250 lines
- PR2: Tasks T2, T3, T11a, T12 (GraphViewRenderer) — ~400 lines
- PR3: Tasks T6, T7, T8, T9, T10, T11b, T13 (Exploration Snapshot) — ~600 lines

After all PRs merged to `main`, tag `v0.X.0` (semver minor for new feature; breaking change in `ExplorationSession.panes` warrants minor bump per semver).

## Estimated Total Effort

~13-15.5 hours (1.5 days)
- PR1: ~2.5h (T1: 15min + T4: 1.5h + T5: 30min)
- PR2: ~4h (T2: 2h + T3: 1h + T11a: 2h + T12: 30min)
- PR3: ~7h (T6: 30min + T7: 1h + T8: 2h + T9: 1h + T10: 1h + T11b: 2h + T13: 30min)

## Verification (per PR)

| PR | Command | Expected |
|----|---------|----------|
| PR1 | `cargo test -p cognicode-explorer && cd apps/explorer-ui && npm run typecheck` | All tests pass; no type errors |
| PR2 | `cd apps/explorer-ui && npm run test:e2e:visual -- call-graph-rendering.spec.ts` | Golden image matches; pane-stack tests pass |
| PR3 | `cargo test -p cognicode-explorer && cd apps/explorer-ui && npm run test:e2e:visual -- exploration-persistence.spec.ts` | All tests pass; legacy session 422; localStorage round-trip works |

## Rollback Notes

- **PR1 rollback:** Revert T1 + T4 + T5. `ContextualView` without `view_kind` falls through to `<Blocks>` (pre-change behavior). No DB impact.
- **PR2 rollback:** Revert T2 + T3 + T11a + T12. Routing no longer fires; `<Blocks>` rendering restored.
- **PR3 rollback:** Revert T6-T13. **`ExplorationSession.panes` removal requires DB column drop if migrated** — check migration status before rollback. Otherwise legacy sessions (no `panes`) become invalid (intentional per Decision 13).
