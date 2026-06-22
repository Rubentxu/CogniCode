# ADR-043: Renderer backend reconciliation — SvgGraph vs Cytoscape

**Status:** Accepted
**Date:** June 22, 2026
**Source:** Post-implementation reconciliation after E1.4 + E1.5 landed in
v0.10.0 (merge commit `f660420`). Discovery evidence in engram
observations #2592, #2626, #2634.

## Context

After the E1.4 + E1.5 cycle (commits `9c54321`..`525512e` on branch
`sddk/E1-renderer-consolidation`, merge `f660420`, tag `v0.10.0`),
the Explorer has **two graph rendering backends** that coexist for
architecturally valid reasons:

| Backend | Surface | Strengths | Used by |
|---------|---------|-----------|---------|
| **GraphView** (custom React + `SvgGraph`) | User-driven `ViewSpec` rendering | Dispatch routing, viewport capture, `onClose`, ViewSpec-driven layout adapters (`layoutFromContextualView`, `layoutFromDependencyGraph`) | `rendererRegistry["graph"]` entry (post-E1.5), the 5 graph ViewKinds (`call_graph`, `dependency_graph`, `data_flow`, `impact_radius`, `seam_map`) |
| **InteractiveGraph** (Cytoscape.js + ELK worker) | Component-embedded subgraph renderers | Cytoscape WebGL selective (ADR-042, 5–21 % faster at ≥500 nodes), proven at scale | `RationaleView` (imports `<InteractiveGraph>` directly at `apps/explorer-ui/src/components/RationaleView/RationaleView.tsx:18,125`); `NeighborMinigraph` (instantiates `cytoscape` directly at `apps/explorer-ui/src/components/ContextualPanel/NeighborMinigraph.tsx:12,50` and reuses `InteractiveGraph/stylesheet` + `InteractiveGraph/adapter` for visual consistency); the E7 bench infrastructure under `apps/explorer-ui/src/bench/renderers/` |

Pre-E1.5 the registry's `graph` entry was a stub (`onSelectObject={() => {}}`,
`selectedId={null}`) and `PaneInspector` short-circuited to `GraphView`
directly. E1.5 rewires `rendererRegistry["graph"]` to `GraphView` and
removes the short-circuit, but it does **not** deprecate `InteractiveGraph`
because that component still ships in the production render path for
component-embedded surfaces.

The exploration report (`sddk/E1-renderer-consolidation/explore`, engram
#2626) and the prior discovery (engram #2592) both flagged this fork as
load-bearing. This ADR codifies the resulting architecture so future
contributors do not re-open the unification debate.

## Decision

Adopt a **dual-backend architecture with explicit routing rules**:

1. **Graph ViewKinds** (`call_graph`, `dependency_graph`, `data_flow`,
   `impact_radius`, `seam_map`) — canonical backend is **`GraphView`**
   (SvgGraph).

   - Routing: `PaneInspector` → `resolveRenderStrategy(view)` →
     `rendererRegistry.render("graph", view, runtimeContext)` →
     `GraphView`.
   - `runtimeContext` carries `{ view, objectId, paneId, viewId,
     dispatch, onClose, onSelectObject }` per spec E1.5
     (`REQ-E1.5-2`).
   - This codifies ADR-040 §1 ("GraphViewRenderer para ViewKinds
     estructurales") and is the path E1.5 made live.
   - `InteractiveGraph` is **not** wired into this path. Adding it
     would require porting `GraphView`'s dispatch routing, viewport
     capture, and `onClose` to `InteractiveGraph`, which the
     exploration report flagged as high-effort and low-ROI relative to
     ADR-042's existing WebGL gains.

2. **Component-embedded subgraph renderers** — canonical backend is
   **`InteractiveGraph`** (Cytoscape + ELK).

   - Used by `RationaleView` (imports `<InteractiveGraph>` directly
     and wraps it with a data-fetching layer — corroborated
     rationale subgraphs), `NeighborMinigraph` (instantiates
     `cytoscape` directly and reuses the `InteractiveGraph`
     stylesheet + adapter for visual consistency on the
     ContextualPanel), and the E7 bench harnesses in
     `apps/explorer-ui/src/bench/renderers/`.
   - These surfaces are **not** user-authored `ViewSpec` views. They
     are component-local renderers that ship with the component.
   - ADR-042 (Cytoscape WebGL selective for ≥500 nodes) governs
     performance for this surface.

3. **`rendererRegistry["graph"]` registration** — the entry resolves
   to `GraphView` (SvgGraph), not `InteractiveGraph`.

   - Pre-E1.5 the entry was a dead-code stub. E1.5 replaces it with
     a thin adapter that forwards `RuntimeContext` to `GraphView`.
   - This makes `GraphView` the canonical graph renderer for any
     `ViewSpec` whose author sets `renderer_kind: "graph"` (verified
     for 2 of 5 graph ViewKinds via `crates/cognicode-explorer/src/
     facades/view.rs:300`; the other 3 are ViewSpec-only and rely on
     the author).

### Routing summary

```
                        user input
                            │
                            ▼
                    PaneInspector
                            │
                            ▼
                resolveRenderStrategy(view)
                            │
              ┌─────────────┴─────────────┐
              │                           │
   renderer_kind === "graph"        otherwise
   OR view_kind ∈ GRAPH_KINDS            │
              │                           ▼
              ▼                   Blocks component
  rendererRegistry.render(         (per-block dispatch)
    "graph", view, ctx)                    │
              │                           ▼
              ▼                   blockRendererRegistry.get(
        GraphView (SvgGraph)                block.id)
                                       (29 entries)
```

```
   Component-embedded surfaces (RationaleView, NeighborMinigraph,
   E7 bench) — direct import path, NOT through rendererRegistry:
                            │
                            ▼
                    InteractiveGraph (Cytoscape + ELK, WebGL per ADR-042)
```

## Consequences

### Positive

- Each backend serves its natural surface: `GraphView` for
  ViewSpec-driven views, `InteractiveGraph` for component-embedded
  subgraph renderers.
- E1.5 unifies the dispatch path for user-driven views. The registry
  is no longer dead code; the short-circuit is gone.
- ADR-042 WebGL gains are preserved on the surfaces that benefit
  (component-embedded subgraphs that may scale to ≥500 nodes).
- The E7 bench harness remains the canonical tool for evaluating
  future renderer migrations. It still drives `cytoscape-canvas`,
  `cytoscape-webgl`, and `sigma-poc` adapters.
- No data-path migration; both backends coexist with their existing
  style sheets, test suites, and layout systems.

### Negative

- Two renderers means two style sheets, two test suites, two layout
  systems to maintain.
- Future migration to a single backend (e.g., unify on Cytoscape
  with `GraphView`'s dispatch routing ported over) is more work than
  if a single backend had been chosen upfront.
- Bundle size includes both cytoscape and the custom SvgGraph
  renderer. The `cytoscape-webgl` preview API also adds
  initialization overhead on small graphs.
- The `InteractiveGraph` import in `rendererRegistry.tsx` (lazy
  import for bench) is now a dead code path in production; this ADR
  does not remove it because the E7 bench still needs it.

### Neutral

- ADR-040 (GraphViewRenderer routing) governs the graph ViewKind
  path. ADR-042 (Cytoscape WebGL selective) governs the
  component-embedded path. This ADR does not change either; it
  reconciles their coexistence.
- `RationaleView` continues to import `<InteractiveGraph>` directly.
  `NeighborMinigraph` instantiates `cytoscape` directly and reuses
  `InteractiveGraph/stylesheet` + `InteractiveGraph/adapter` for
  visual consistency. Both are out of scope for any future "registry
  for everything" refactor.

## Alternatives Considered

### A) Unify on Cytoscape for all graph surfaces

**Rejected.** Would require porting `GraphView`'s dispatch routing
(`createDispatchRouting`), viewport capture, `onClose`, and
`layoutFromContextualView` into `InteractiveGraph`. The exploration
report (#2626) and the prior discovery (#2592) flagged this as
high-effort and low-ROI relative to ADR-042's WebGL gains, which
already cover the high-scale workload on the surface that benefits.

### B) Unify on SvgGraph for all graph surfaces

**Rejected.** Loses ADR-042's WebGL performance gains (5–21 % faster
at 1k+ nodes). The E7 bench results in
`apps/explorer-ui/artifacts/e7-renderer-bench/{results.json,report.md}`
are the evidence base; reverting them is not justified.

### C) Make `rendererRegistry["graph"]` abstract over backend via a `RendererBackend` enum

**Deferred.** Would require introducing a backend enum, switching
logic in the registry, and refactoring `InteractiveGraph`'s direct
import sites in `RationaleView` and `NeighborMinigraph`. The current
setup (registry entry = `GraphView`, `InteractiveGraph` via direct
import) is simpler and matches the natural surface split. Revisit
only if a third backend emerges or if the dispatch routing in
`InteractiveGraph` is needed for ViewSpec views.

### D) Deprecate `InteractiveGraph` entirely

**Rejected.** `RationaleView` imports `<InteractiveGraph>` directly.
`NeighborMinigraph` imports `cytoscape` directly and reuses
`InteractiveGraph/stylesheet` + `InteractiveGraph/adapter`. The E7
bench depends on the cytoscape adapters. Deprecation would force a
port of those surfaces to `GraphView` (SvgGraph), which has not been
evaluated at their scale. The E7 bench is the tool for that
evaluation, not a fait accompli.

## References

- `docs/adr/ADR-039-explorer-navigation-model.md` — navigation model
- `docs/adr/ADR-040-graph-view-renderer.md` — GraphViewRenderer for
  structural ViewKinds (the routing decision this ADR reconciles with)
- `docs/adr/ADR-041-explorer-renderer-scale-evaluation.md` —
  evaluation methodology
- `docs/adr/ADR-042-renderer-decision.md` — Cytoscape WebGL
  selective
- Engram #2592 — prior discovery flagging the dual-backend fork
- Engram #2626 — explore report for E1.4 + E1.5
- Engram #2629 — design for E1.4 + E1.5
- Engram #2634 — verify report for E1.4 + E1.5
- `crates/cognicode-explorer/src/facades/view.rs:300` — single seam
  where the backend stamps `renderer_kind`
- `crates/cognicode-explorer/src/domain/views.rs:1369, 1540` —
  `CallGraphExecutor`, `DependenciesExecutor` returning
  `RendererKind::Graph`
- `apps/explorer-ui/src/components/ObjectInspector/viewKind.ts` —
  `resolveRenderStrategy` and `isGraphViewKind`
- `apps/explorer-ui/src/components/rendererRegistry.tsx` —
  `rendererRegistry["graph"]` resolving to `GraphView` (post-E1.5)
- `apps/explorer-ui/src/components/ObjectInspector/blockRendererRegistry.tsx`
  — block-id registry (E1.4)
- `apps/explorer-ui/src/components/GraphView/GraphView.tsx` — SvgGraph
  surface (canonical for ViewSpec views)
- `apps/explorer-ui/src/components/InteractiveGraph/` — Cytoscape
  surface (canonical for component-embedded subgraphs)
- `apps/explorer-ui/src/bench/` — E7 bench infrastructure
- `apps/explorer-ui/artifacts/e7-renderer-bench/{results.json,report.md}`
  — bench evidence base

## Glossary terms

This ADR formalizes the following concepts already present in
`CONTEXT.md`:

- **GraphView** (post-E1.5): the canonical `Graph`-kinded
  `RendererEntry` for `ViewSpec` views. Wraps the SvgGraph renderer
  with `RuntimeContext` for dispatch routing, viewport capture, and
  `onClose`.
- **InteractiveGraph** (post-E1.5): the canonical Cytoscape-based
  renderer for component-embedded subgraph surfaces
  (`RationaleView`, `NeighborMinigraph`, E7 bench). NOT routed
  through `rendererRegistry`.
- **Dual-backend architecture**: the split where `GraphView` (custom
  React + SvgGraph) serves ViewSpec views and `InteractiveGraph`
  (Cytoscape + ELK) serves component-embedded subgraphs. This is the
  current architectural reality; future unification must be
  evaluated against the E7 bench.
