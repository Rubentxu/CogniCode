# Explore Report — moldable-view-call-graph

**Date:** 2026-06-20
**Context quality:** C2
**Mode:** Interactive (post-grill session)
**Author:** sdd-kernel-explore

## 1. Executive Summary

The `call_graph` view bug (blank SVG, metadata renders) is **confirmed reproducible** via code inspection: `PaneInspector.tsx` (line 225-234) unconditionally renders `<Blocks>` regardless of `ViewKind`, and `ViewBlock.tsx` lacks a `case "call_graph":` branch. The ADR-040 / roadmap documents correctly diagnose the routing gap. Of the 13 grill-session decisions, **4 are already implemented** (PaneStack dedup, edge label wiring, naming in CONTEXT.md, no-backward-compat policy in ADR-040), **3 are partially in place** (routing location identified but not used, SvgGraph pan/zoom/click ready but no `onViewportChange`, `Pane` has `scrollY` but no `viewport`), and **6 are unimplemented** (GraphViewRenderer component, early-return, empty state, useMemo layout, snapshot persistence, hybrid trigger). A **critical schema gap** was discovered: ADR-040's routing check uses `display.kind` but the `ContextualView` zod schema (schemas.ts:784-800) has no `kind` field — only `view_id` and `renderer_kind`. This must be resolved before the proposal phase.

## 2. Context Quality Gate

- **Knowledge coverage:**
  - Roadmap/Backlog: `docs/roadmap/MOLDABLE-VIEW-PANE-STATE-2026.md` ✅
  - ADRs: `docs/adr/ADR-040-graph-view-renderer.md` ✅
  - UX wireframes: `docs/wireframes/MOLDABLE-VIEW-UX-WORKFLOW.md` ✅
  - Domain language (CONTEXT.md): GraphViewRenderer, Moldable Navigation, Exploration Snapshot terms added ✅
  - Work Items: NOT in M3-Sprint/ — change is open, not yet broken into tasks
- **Domain language:** Resolved (13/13 grill questions answered 2026-06-20)
- **Invariants:**
  - `Pane` is capped at `MAX_PANES = 8` (paneStack.ts:26)
  - `SELECT_OBJECT` deduplicates by `objectId` (paneStack.ts:116-124) — same object reuses existing pane
  - `ContextualView` schema has `renderer_kind` (default `"json"`) but no `view_kind` field — schema gap
  - Closing the active pane moves focus to a neighbour (paneStack.ts:71-81)

## 3. Grill Decision Verification (13/13)

| # | Decision | Codebase Reality | Status |
|---|----------|------------------|--------|
| 1 | Routing in `PaneInspector` with early-return | `PaneInspector.tsx:118` computes `display = view ?? _activeView`; line 225-234 always renders `<Blocks>`. **No early-return exists.** | ❌ |
| 2 | Location: after `display = view ?? _activeView` | Location identified at line 118, but routing does not consume it | ⚠️ Partial |
| 3 | `GraphViewRenderer` receives full `ContextualView` | `/components/GraphView/` directory exists but is **empty** (no `GraphViewRenderer.tsx`) | ❌ |
| 4 | Generic `GraphViewRenderer` (not specific) | Not created | ❌ |
| 5 | Pane Stack navigation with dedup | `paneStack.ts:116-129` `SELECT_OBJECT` finds existing pane and activates it. `e2e/pane-stack.spec.ts:232` (P3.2 variant) verifies no duplicates. | ✅ |
| 6 | Edge labels highlight-only | `SvgGraph.tsx:301` always passes `label={edge.label}`. `GraphEdge.tsx:53` renders label whenever present. Labels render on **all** edges, not just highlighted ones. | ⚠️ Partial — design intent vs implementation diverge |
| 7 | Empty state for graphs | No empty state code found anywhere | ❌ |
| 8 | `useMemo` with `[view.object_id, view.blocks]` | Design only — no implementation | ❌ |
| 9 | Pane State Snapshot (scroll + zoom + pan) | No snapshot capture logic in `SvgGraph`, no persistence in `useExplorations` | ❌ |
| 10 | `PaneSnapshot { pane_id, object_id, view_id, scroll_y, viewport }` + `ViewportState { x, y, scale }` | `Pane` type has `scrollY` (types.ts:40) but **no `viewport`**. `ViewportState` type does not exist. `dto.rs:384-394` `ExplorationSession` has only `events: Vec<ExplorationEvent>` — no `panes` field. | ❌ |
| 11 | Trigger hybrid (manual + localStorage) | Design only in ADR-040 §7 — not implemented | ❌ |
| 12 | Naming: "Exploration Snapshot" | In CONTEXT.md §Navigation, ADR-040 §6, §9 | ✅ |
| 13 | NO backward compatibility | ADR-040 §8 commits to no `#[serde(default)]` on `panes` field | ✅ |

## 4. Gap Analysis

### 4.1 Files to MODIFY

| File | Change | Effort |
|------|--------|--------|
| `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx` | Add `isGraphViewKind()` helper + early-return after line 118; import `GraphViewRenderer` | 30 min |
| `apps/explorer-ui/src/components/SvgGraph/SvgGraph.tsx` | Expose `onViewportChange` callback; thread `paneId` prop so caller can persist viewport to navigation state | 1 h |
| `apps/explorer-ui/src/state/navigation/types.ts` | Add `ViewportState` type; add optional `viewport?: ViewportState` to `Pane`; add `UPDATE_PANE_VIEWPORT` to `NavigationAction` union | 15 min |
| `apps/explorer-ui/src/state/navigation/paneStack.ts` | Handle `UPDATE_PANE_VIEWPORT` reducer case (mutate `panes[i].viewport`) | 15 min |
| `apps/explorer-ui/src/components/SvgGraph/GraphEdge.tsx` | Render label only when `highlighted=true` (Decision 6 alignment) | 15 min |
| `crates/cognicode-explorer/src/dto.rs` | Add `PaneSnapshot { pane_id, object_id, view_id, scroll_y, viewport: Option<ViewportState> }` + `ViewportState { x, y, scale }`; add `panes: Vec<PaneSnapshot>` field to `ExplorationSession` (no `#[serde(default)]`) | 1-2 h |
| `crates/cognicode-explorer/src/facades/persistence.rs` | Update `save_exploration_session` to accept and store `panes` | 1 h |
| `crates/cognicode-explorer/src/facades/mod.rs` | Update `PersistenceService` trait signature for `save_exploration_session` | 30 min |
| `crates/cognicode-explorer/src/api.rs` | Validate `panes` non-empty (or allow empty); return 422 on malformed payload | 30 min |
| `apps/explorer-ui/src/hooks/useExplorations.ts` (or equivalent) | Capture `panes` snapshot from `state.navigation.panes` on save; send to backend | 1 h |
| `apps/explorer-ui/src/api/schemas.ts` | **CRITICAL:** Add `view_kind` field to `contextualViewSchema` (decision for routing), OR derive routing from `renderer_kind === "graph"`. See §6 schema gap. | 30 min — schema decision required |

### 4.2 Files to CREATE

| File | Purpose | Effort |
|------|---------|--------|
| `apps/explorer-ui/src/components/GraphView/GraphViewRenderer.tsx` | Generic graph renderer consuming `ContextualView` | 2-3 h |
| `apps/explorer-ui/src/components/GraphView/GraphEmptyState.tsx` | "No call relationships" empty state component | 30 min |
| `apps/explorer-ui/src/components/GraphView/GraphLoadingSkeleton.tsx` | Loading skeleton for graph views | 30 min |
| `apps/explorer-ui/src/components/GraphView/index.ts` | Barrel export | 5 min |
| `apps/explorer-ui/src/components/GraphView/GraphViewRenderer.test.tsx` | Vitest unit tests (render, memoization, empty state, click navigation) | 2 h |
| `apps/explorer-ui/src/state/navigation/paneStack.test.ts` (extend) | Add `UPDATE_PANE_VIEWPORT` reducer test | 30 min |
| `apps/explorer-ui/e2e/graph-renderer.spec.ts` | Playwright: call-graph-rendered, pane-stack-multi, call-graph-empty | 2 h |
| `apps/explorer-ui/e2e/exploration-persistence.spec.ts` | Playwright: save with viewport, load restores viewport | 2 h |

### 4.3 Tests to CREATE / EXTEND

- **Vitest:**
  - `GraphViewRenderer.test.tsx` — renders SvgGraph, memoizes layout, shows empty state when `nodes.length <= 1`, dispatches `SELECT_OBJECT` on node click
  - `paneStack.test.ts` — `UPDATE_PANE_VIEWPORT` reducer
  - `viewport.test.ts` — `ViewportState` type round-trip
- **Playwright:**
  - `call-graph-ready` — switch to call_graph tab, SVG renders with ≥2 nodes, screenshot
  - `call-graph-empty` — switch to symbol with no callers/callees, empty state visible
  - `pane-stack-multi` — click node, second pane opens, no duplicates
  - `exploration-save-viewport` — zoom graph, save exploration, verify payload contains viewport

## 5. Risk Assessment

1. **Schema gap: `ContextualView.kind` does not exist.** ADR-040's routing pseudo-code uses `display.kind` but the zod schema (schemas.ts:784-800) has no such field. Options: (a) add `view_kind` to the schema, (b) derive routing from `renderer_kind === "graph"`, (c) pass ViewKind through navigation state. **Must be resolved before implementation.** Risk: silent regression if routing key is undefined → early-return never fires → bug persists.

2. **Breaking change: `ExplorationSession.panes` field has no default.** Per ADR-040 §8 + grill Decision 13, old sessions (no `panes`) will fail deserialization with 422. Users with saved explorations lose access. Risk: data loss without migration path. Mitigation: explicit user-facing warning + changelog entry.

3. **`useMemo([view.object_id, view.blocks])` may produce stale layout.** Block reference equality changes on every `useViews` SWR refetch even when content is identical (new array). If layout depends on derived data outside `object_id`/`blocks` (e.g., `relations`), it will not update. Risk: layout mismatch after data update. Mitigation: extract a `layoutKey` derived from canonicalized blocks, add Vitest for layout stability.

4. **`SvgGraph` viewport capture requires plumbing through `PaneInspector`.** SvgGraph is a generic component — adding `paneId` + `onViewportChange` props couples it to navigation state. Risk: tight coupling, testability reduced. Mitigation: keep SvgGraph prop optional with no-op default; only `GraphViewRenderer` wires it.

5. **Routing early-return placement could affect existing pane-stack tests.** Currently every pane renders `<Blocks>`; after early-return, graph-kind panes render `<GraphViewRenderer>`. Existing snapshot tests may capture different markup. Risk: visual regressions in `visual-regression.spec.ts`. Mitigation: regenerate snapshots, document intentional divergence.

6. **Bundle size: `GraphViewRenderer` pulls in cytoscape via `InteractiveGraph`.** Already lazy-loaded in `rendererRegistry.tsx:19-21`. New renderer should follow same pattern via `React.lazy()`. Risk: initial bundle unaffected; graph tab triggers dynamic import. Mitigation: lazy + Suspense boundary.

## 6. Knowledge Contract Check

| Artifact | Status | Path |
|----------|--------|------|
| CONTEXT.md (GraphViewRenderer term) | ✅ | `CONTEXT.md:143-162` |
| ADR-040 | ✅ | `docs/adr/ADR-040-graph-view-renderer.md` (192 lines) |
| Roadmap | ✅ | `docs/roadmap/MOLDABLE-VIEW-PANE-STATE-2026.md` (522 lines, 9 tasks) |
| UX wireframes | ✅ | `docs/wireframes/MOLDABLE-VIEW-UX-WORKFLOW.md` (lines 280-680 cover Call Graph flow) |
| Delta spec | ❌ | Not yet created — owned by sdd-kernel-spec |
| Task breakdown | ❌ | Not yet created — owned by sdd-kernel-tasks |

**Outstanding knowledge gap (blocking):** The `ContextualView` schema does not expose `view_kind`. ADR-040 routing logic assumes `display.kind` exists. This needs a schema decision (Rust enum + TS zod field) before proposal is finalized.

## 7. Lenses Applied

- **interface-design** — API contract for `GraphViewRenderer` props (`view: ContextualView`, `objectId: string`, `onClose?: () => void`, optional `onViewportChange?: (v: ViewportState) => void`); `PaneSnapshot` shape; `isGraphViewKind` predicate location.
- **connascence-static** — coupling between `GraphViewRenderer` ↔ `SvgGraph` (must export layout type), `GraphViewRenderer` ↔ `PaneStackNavigation` (SELECT_OBJECT dispatch), `Pane.viewport` ↔ `SvgGraph.onViewportChange`.
- **cognicode-sdd** — verified bug via code inspection; identified schema gap in routing decision; cross-referenced existing tests (`pane-stack.spec.ts` P3.2, `graph.spec.ts` Phase 11 acceptance).
- **test-pyramid** — unit (Vitest on reducer + renderer) + integration (Playwright e2e) coverage; visual regression baseline required before merge.

## 8. Affected Areas

- `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx` — routing orchestration
- `apps/explorer-ui/src/components/GraphView/*` (new dir) — renderer + empty/loading states
- `apps/explorer-ui/src/components/SvgGraph/SvgGraph.tsx` — viewport exposure
- `apps/explorer-ui/src/state/navigation/{types,paneStack}.ts` — viewport state
- `apps/explorer-ui/src/hooks/useExplorations*` — snapshot capture on save
- `apps/explorer-ui/src/api/schemas.ts` — `ContextualView` schema gap
- `crates/cognicode-explorer/src/dto.rs` — `ExplorationSession.panes`
- `crates/cognicode-explorer/src/facades/persistence.rs` — store panes
- `crates/cognicode-explorer/src/facades/mod.rs` — trait signature
- `crates/cognicode-explorer/src/api.rs` — endpoint validation
- `CONTEXT.md` — already updated; verify no further term additions needed
- `docs/adr/ADR-040` — already exists; flag schema gap as ADR addendum

## 9. Recommended Next Phase

Launch **sdd-kernel-propose** with the following inputs:

- **WHY:** Critical bug fix (blank SVG in call_graph) + unlock Moldable Development experience (pane-stack drill-down, exploration snapshot persistence). Aligns with grill session 2026-06-20 (13/13 decisions resolved, 13+1 schema decision pending).
- **WHAT:** 2 epics, 9 tasks per roadmap, ~11-14 h total.
  - **Epic 1 — GraphViewRenderer (4 tasks, 4-6 h, ALTA):** Create component → routing in PaneInspector → visual regression test → ADR-040 addendum for schema gap.
  - **Epic 2 — Exploration Snapshot (5 tasks, 7-8 h, MEDIA):** Extend `Pane` viewport → capture viewport in SvgGraph → backend `ExplorationSession.panes` → frontend snapshot capture → E2E persistence tests.
- **APPROACH:** Spike-then-spec. First resolve the `ContextualView` schema gap (proposal must choose: add `view_kind` field vs derive from `renderer_kind`). Then TDD: failing Vitest → implementation → passing → Playwright visual regression.
- **OUT OF SCOPE:** Real backend layout endpoint (mocked via `layoutFromContextualView`); other graph ViewKinds (`dependency_graph`, `seam_map`, `data_flow`, `impact_radius`) route but render with the same SvgGraph; remote renderer plugins; WorkspaceSnapshot UI beyond storage.

## 10. Ready For Proposal

**Yes** — with one blocking question:

> **Schema decision for routing:** Should `ContextualView` carry `view_kind` (semantic intent) to drive routing, OR should routing derive from `renderer_kind === "graph"` (visual strategy)? Both are valid per ADR-008 (ViewKind vs RendererKind split). The grill session did not surface this gap.

Resolution unlocks the proposal's interface-design lens output.
