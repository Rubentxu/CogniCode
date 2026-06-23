# ADR-039: Explorer Navigation Model — Pane-Stack, Graph-First, C4 Dual Entry

**Status:** Accepted
**Date:** 2026-06-19
**Last updated:** 2026-06-22
**Source:** grill-with-docs session on Explorer evolution, apps/explorer-ui audit

## Context

The CogniCode Explorer (`apps/explorer-ui`) has a React 19 + Vite + Cytoscape
frontend with substantial implementation: pane-stack navigation, Spotter search,
renderer registry, ViewSpec wizard, interactive graph with ELK layout, and C4
stylesheet classes. Miller columns were removed (E3 complete).

This ADR records the decisions made during a structured grill-with-docs session
that resolved the navigation model, entry points, renderer strategy, and UX
identity of the Explorer.

## Decisions

### 1. Pane-stack is the only navigation model

`pane-stack` (gtoolkit GtPager-style lateral inspection) is the **official
navigation model**. The `column` mode (Miller/Finder-style drill-down) is
**hard-cut** — it enters immediate removal, not deprecation.

Rationale: maintaining two navigation models dilutes product identity, increases
state complexity, and anchors the UX to traditional IDE patterns. If the goal is
a gtoolkit-inspired tool, the navigation must commit to that model.

### 2. Graph is the primary visual landing

The Explorer's first screen is a **graph overview**, not a file tree, not a
dashboard of cards, and not a list. The graph shows root nodes, hot paths, and
architecture structure. This delivers the visual impact of a 21st-century
exploration tool.

### 3. C4 is both entry point and lens

C4 (Context, Container, Component, Code) is a **first-class entry point** —
not a secondary view. The user can start exploration from C4 Context just as
naturally as from the call graph. Additionally, C4 acts as a **lens**: the same
graph canvas can morph between Graph perspective and C4 perspective without
navigating to a separate page.

### 4. Single canvas with perspective toggle

The landing is a **single canvas** with a lightweight perspective toggle
(`Context ↔ Graph`). There are no separate tabs, pages, or routes for different
entry modes. Spotter (Cmd+K) is an overlay on top of the canvas, not a page.

```
┌─────────────────────────────────────────────────────┐
│  CogniCode Explorer          [Context|Graph]  ⌘K    │
├──────────────────────────────────┬──────────────────┤
│                                  │                  │
│        GRAFO CENTRAL             │   PANE STACK     │
│        (toggle morphs:           │  lateral panes   │
│         C4 ↔ call graph)         │                  │
│                                  │                  │
│   click nodo → abre pane ────────┼──► inspector     │
│                                  │                  │
└──────────────────────────────────┴──────────────────┘
```

### 5. Cytoscape is the renderer base; WebGL adopted selectively

The current Cytoscape integration is retained as the rendering base. The
existing ELK.js layout worker (`layout.worker.ts`) is integrated into the main
renderer (replacing `preset` layout).

**Update (June 22, 2026 — ADR-041/ADR-042):** WebGL is now **adopted
selectively**. Evidence from real-browser benchmarks showed WebGL is ~20%
faster for graphs ≥ 500 nodes, but canvas is ~15-25% faster for small
fixtures (< 16 nodes). The implementation uses `renderer: { name: "canvas",
webgl: <bool> }` with `useWebgl = preferWebgl && nodeCount >= 500`. Sigma.js
remains behind `BENCH_ENABLE_SIGMA=1` as a future exploration path.

### 6. gtoolkit is the design inspiration rector

The Explorer follows gtoolkit's philosophy: objects are inspected through
multiple moldable views, navigation is lateral (not hierarchical), and the user
builds understanding by composing perspectives — not by navigating a file tree.

### 7. Root node display follows Graphify's visual density model

The graph landing does not dump 500 root nodes as a flat list. It uses smart
visual density: clustering, progressive disclosure, and visual hierarchy to
reveal structure without overwhelming. The full set of root nodes is available
but visually managed — the user sees the shape of the system first, then drills
into specific areas.

### 8. C4 inference uses heuristics from cognicode-core's existing features

C4 structure is inferred automatically from what the backend already parses:
- **Containers**: from `Cargo.toml`, `package.json`, `pyproject.toml` (already
  parsed by tree-sitter + config parsers)
- **Components**: from directory structure and module boundaries (already
  available via `GraphQueryPort`)
- **Code elements**: from symbols and edges (already in `CallGraph`)

No separate declarative architecture file is required. The inference pipeline
uses heuristics over existing data, not manual configuration.

### 9. Node click shows navigability preview before opening full pane

Clicking a node in the graph does **not** immediately open a full pane-stack
entry. Instead, a small navigability interface appears (mini-preview showing
the node's links, callers, callees, and available views). From that preview,
the user can choose to expand into a full pane-stack pane.

This follows gtoolkit's pattern: hover/preview first, commit to a full
inspection pane second. It prevents accidental pane proliferation and gives
the user a lightweight way to decide whether a node is worth a deeper dive.

## Evolution order

1. Consolidate view model — `rendererRegistry` becomes the real render pipeline (**E1 — incomplete**)
2. Integrate ELK layout worker into `InteractiveGraph` (replace `preset`) (**E2 — complete**)
3. Ship perspective toggle (`Graph ↔ C4`) on the graph canvas (**E5 — partial; toggle works on landing only, not in InteractiveGraph**)
4. Eliminate `column` navigation mode (hard cut) (**E3 — complete**)
5. ~~Evaluate WebGL / Sigma.js when scale demands it~~ — **E7 complete; WebGL adopted selectively** (ADR-042)

## Implementation status (June 22, 2026)

| Sprint | Status | Notes |
|--------|--------|-------|
| E1 — Consolidate View Model | ✅ Complete | E1.1✅ E1.2⚠️ E1.3✅ E1.4✅ E1.5✅ — `rendererRegistry` is now the authoritative pipeline; `PaneInspector` routes all rendering through `resolveRenderStrategy` |
| E2 — ELK layout worker | ✅ Complete | `InteractiveGraph` + `layout.worker.ts` fully integrated |
| E3 — Hard cut column nav | ✅ Complete | `column` mode removed; `MillerColumns` deleted; state is pane-stack only |
| E4 — Graph Landing Page | ✅ Complete | E4.1✅ E4.2⚠️(hook renamed) E4.3✅ E4.4✅ E4.5✅ (strip + endpoint) |
| E5 — Perspective toggle | ✅ Complete | Toggle wired into `InteractiveGraphPanel` (`Shell.tsx:45-88`); canvas morphs between graph (useSubgraph) and C4 (useArchitecture) perspectives after object selection |
| E6 — C4 Backend Inference | ❌ Not started | `cognicode-diagram` crate does not exist |
| E7 — Renderer evaluation | ✅ Complete | ADR-041/ADR-042 Accepted; WebGL adopted selectively (≥500 nodes) |

**Open architectural gaps:**
- `rendererRegistry` (E1.4/E1.5) is now resolved — `PaneInspector` routes all rendering through `resolveRenderStrategy`
- `ViewBlock.tsx` 27-case `switch` is resolved — all blocks now registered in `blockRendererRegistry`

## Consequences

- **`column` mode removal** simplifies the navigation state machine, the Shell
  layout logic, and the `NavigationAdapter` contract. Tests referencing Miller
  columns are updated or removed. Minor residue: `useRovingFocus` hook is dead
  code (no importers).
- **`rendererRegistry` is now authoritative** — all view rendering passes
  through it via `resolveRenderStrategy` and `blockRendererRegistry`. **`PaneInspector`
  no longer imports `GraphView` directly.**
- **ELK worker integration** changes `InteractiveGraph` from static-preset to
  dynamic layout with animation and cancellation. **Done (E2).**
- **C4 backend support** is needed for the perspective toggle to show real C4
  nodes. **Status: NOT YET DONE — `cognicode-diagram` crate does not exist.**
- **WebGL evaluation** is now complete (ADR-041/ADR-042). WebGL adopted
  selectively for graphs ≥ 500 nodes.

## Related ADRs

- [ADR-038](ADR-038-sandbox-hardening-and-coverage.md) — Sandbox hardening
- [ADR-034](ADR-034-mcp-production-readiness.md) — MCP production readiness
- ADR-008 (implied) — ViewSpec / RendererKind / ViewKind architecture

## References

- `apps/explorer-ui/src/state/navigation/types.ts` — Pane-stack state types
- `apps/explorer-ui/src/state/navigation/paneStack.ts` — Pane-stack reducer
- `apps/explorer-ui/src/components/InteractiveGraph/*` — Graph renderer + ELK worker
- `apps/explorer-ui/src/components/rendererRegistry.tsx` — Renderer registry (dead code in prod; bypassed by PaneInspector)
- `apps/explorer-ui/src/components/GraphView/*` — Live graph rendering path (bypasses registry)
- `apps/explorer-ui/src/components/PaneStackView.tsx` — Pane-stack view
- `apps/explorer-ui/src/components/GraphLanding/GraphLanding.tsx` — Landing page
- `apps/explorer-ui/src/components/Spotter.tsx` — Spotter search palette
- `apps/explorer-ui/src/components/ObjectInspector/ViewSpecWizard.tsx` — ViewSpec authoring
- `docs/adr/ADR-041-explorer-renderer-scale-evaluation.md` — E7 benchmark protocol
- `docs/adr/ADR-042-renderer-decision.md` — WebGL selective adoption decision
