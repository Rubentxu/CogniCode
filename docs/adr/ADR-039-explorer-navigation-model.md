# ADR-039: Explorer Navigation Model — Pane-Stack, Graph-First, C4 Dual Entry

**Status:** Accepted
**Date:** 2026-06-19
**Source:** grill-with-docs session on Explorer evolution, apps/explorer-ui audit

## Context

The CogniCode Explorer (`apps/explorer-ui`) has a React 19 + Vite + Cytoscape
frontend with substantial implementation: pane-stack navigation, Miller columns,
Spotter search, renderer registry, ViewSpec wizard, interactive graph, and C4
stylesheet classes. However, the product direction was not locked.

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

### 5. Cytoscape is the renderer base; WebGL evaluation stays deferred

The current Cytoscape integration is retained as the rendering base. The
existing ELK.js layout worker (`layout.worker.ts`) is integrated into the main
renderer (replacing `preset` layout). WebGL evaluation remains a **future
evolutionary path** when graph scale or interaction fluidity demands GPU
acceleration — but not before the view model is consolidated.

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

1. Consolidate view model — `rendererRegistry` becomes the real render pipeline
2. Integrate ELK layout worker into `InteractiveGraph` (replace `preset`)
3. Ship perspective toggle (`Graph ↔ C4`) on the graph canvas
4. Eliminate `column` navigation mode (hard cut)
5. Evaluate WebGL / Sigma.js when scale demands it

> **Implementation status (June 2026):** Item 2 is implemented in
> `InteractiveGraph.tsx` + `layout.worker.ts`. The remaining items stay in the
> roadmap as follow-on work.

## Consequences

- **`column` mode removal** simplifies the navigation state machine, the Shell
  layout logic, and the `NavigationAdapter` contract. Tests referencing Miller
  columns are updated or removed.
- **`rendererRegistry` must become authoritative** — all view rendering passes
  through it, not through ad-hoc component switches.
- **ELK worker integration** changes `InteractiveGraph` from static-preset to
  dynamic layout with animation and cancellation.
- **C4 backend support** is needed for the perspective toggle to show real C4
  nodes. The `cognicode-diagram` crate (planned in `docs/planes/`) provides the
  inference pipeline.
- **WebGL evaluation** is deferred but not forgotten — tracked in roadmap.

## Related ADRs

- [ADR-038](ADR-038-sandbox-hardening-and-coverage.md) — Sandbox hardening
- [ADR-034](ADR-034-mcp-production-readiness.md) — MCP production readiness
- ADR-008 (implied) — ViewSpec / RendererKind / ViewKind architecture

## References

- `apps/explorer-ui/src/state/navigation/types.ts` — NavigationAdapter contract
- `apps/explorer-ui/src/state/navigation/paneStack.ts` — Pane-stack reducer
- `apps/explorer-ui/src/components/InteractiveGraph/*` — Graph renderer + ELK worker
- `apps/explorer-ui/src/components/rendererRegistry.tsx` — Renderer registry (skeleton)
- `apps/explorer-ui/src/components/PaneStackView.tsx` — Pane-stack view
- `apps/explorer-ui/src/components/Spotter.tsx` — Spotter search palette
- `apps/explorer-ui/src/components/ObjectInspector/ViewSpecWizard.tsx` — ViewSpec authoring
- `docs/planes/cognicode-diagram/` — C4 inference architecture (planned crate)
