# Spec: Graph Renderer Canonicalization (E1.5)

## Purpose

Resolve the renderer-backend fork in the frontend by making
`GraphView` the canonical implementation of the `RendererKind: "graph"`
entry in `rendererRegistry`. Today the registry's `graph` entry points
at a degraded stub (`GraphRenderer` in
`apps/explorer-ui/src/components/rendererRegistry.tsx:227-273` — wraps
`InteractiveGraph` with `onSelectObject={() => {}}` and `selectedId={null}`),
while production routing goes through
`apps/explorer-ui/src/components/GraphView/GraphView.tsx` (SvgGraph
backend) via the `isGraphViewKind()` short-circuit in
`apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx:238`.

This spec:

1. Extends `RendererEntry.render`'s signature with an optional
   `RuntimeContext` so the `graph` entry can forward dispatch routing,
   close handling, and viewport capture to `GraphView`.
2. Replaces the `graph` entry's component with `GraphView`.
3. Removes the `isGraphViewKind()` short-circuit from `PaneInspector`,
   routing **all** view kinds through the unified `Blocks` /
   `GraphView` path.
4. Retains `isGraphViewKind()` as a documented zero-cost helper for
   external callers and future tests, but stops using it in the render
   path.

This is **delta** against
`openspec/specs/renderer-registry-frontend/spec.md` (skeleton) and
`openspec/specs/visualization-stack/specs/interactive-graph/spec.md`
(InteractiveGraph behavior).

## Domain

`graph-renderer-canonicalization` — modifies the existing
`renderer-registry-frontend` capability and the existing
`PaneInspector` render dispatch. The split between
`openspec/specs/interactive-graph/spec.md` (Cytoscape backend) and
`apps/explorer-ui/src/components/GraphView/GraphView.tsx` (SvgGraph
backend) is documented in ADR-040 and ADR-042 but **not** reconciled
by any existing spec. This spec does **not** reconcile the two
backends — that is ADR-043 (post-implementation per the proposal
default).

**Phase**: E1 (Renderer Consolidation, sprint E1.5).

---

## Router Context Used

- **Knowledge Coverage**: sufficient (registry, PaneInspector,
  GraphView, GraphViewRenderer, LayoutAdapter, RoutingAdapter all
  verified in source).
- **Context Quality**: C2 — direct file reads + test reads.
- **Taxonomy**: renderer-backend fork (SvgGraph vs Cytoscape) +
  registry signature evolution.
- **Domain Language**: resolved terms from `CONTEXT.md` (RendererKind,
  RendererRegistry, RuntimeContext, ViewKind, graph-shaped ViewKinds).
- **Invariants**: H5 (`SELECT_OBJECT` preserves `viewId`); viewport
  capture; `onClose` propagation.
- **Recommended Effort**: deepen (already done in proposal + decision
  points resolved in AUTO mode).

---

## ADDED Requirements

### Requirement: REQ-E1.5-1 — `graph` entry resolves to `GraphView`

The `RendererKind: "graph"` entry registered in
`apps/explorer-ui/src/components/rendererRegistry.tsx` lines 120–123
MUST be replaced. The new entry's `Component` MUST be `GraphView`
from `apps/explorer-ui/src/components/GraphView/GraphView.tsx`,
**not** the existing `GraphRenderer` stub at lines 227–273 of
`rendererRegistry.tsx`.

The previous `GraphRenderer` function (lines 227–273) and the
`InteractiveGraph` lazy import (line 19) MUST be removed — both are
dead code after E1.5 (production never reached them; the short-circuit
in `PaneInspector.tsx:238` bypassed the registry).

```ts
// rendererRegistry.tsx (sketch — replaces lines 120-123)
this.register("graph", {
  label: "Graph",
  render: (body, extra) => {
    const ctx = (extra ?? {}) as Partial<RuntimeContext>;
    return (
      <GraphView
        view={ctx.view ?? (body as ContextualView)}
        objectId={ctx.objectId ?? ""}
        paneId={ctx.paneId}
        onClose={ctx.onClose}
      />
    );
  },
});
```

#### Scenario: `rendererRegistry.get("graph")` returns the GraphView entry

- **GIVEN** the registry is module-loaded
- **WHEN** `rendererRegistry.get("graph")` is called
- **THEN** the entry's `label` is `"Graph"` (unchanged)
- **AND** invoking `entry.render({...})` returns a React node whose
  rendered tree contains `data-testid="graph-view-renderer"` (per
  `apps/explorer-ui/src/components/GraphView/GraphView.tsx:67`)
- **AND** the entry's render function does NOT instantiate
  `InteractiveGraph` (verified by removing the lazy import and running
  the bundle — cytoscape must not appear in the chunk graph for non-graph
  routes)

#### Scenario: Existing registry tests stay green

- **GIVEN** `apps/explorer-ui/src/components/rendererRegistry.test.tsx`
  asserts at lines 32–55 that every built-in renderer id has an entry
  and `entry.render({})` does not throw
- **WHEN** run after E1.5
- **THEN** the `"graph"` entry exists and `entry.render({nodes: [], edges: []})`
  does not throw (line 137)
- **AND** the assertion at lines 43–47 ("is registered with a label")
  passes
- **AND** `registry.entries().length >= 8` still holds (line 114)

---

### Requirement: REQ-E1.5-2 — `render()` signature gains optional `RuntimeContext`

`RendererEntry.render`'s signature in
`apps/explorer-ui/src/components/rendererRegistry.tsx` lines 38–47 MUST
change from:

```ts
render: (body: unknown, extra?: Record<string, unknown>) => ReactNode;
```

to:

```ts
render: (body: unknown, extra?: RuntimeContext) => ReactNode;
```

where `RuntimeContext` is a new exported interface:

```ts
export interface RuntimeContext {
  /** Optional ContextualView — used by `graph` entry to bypass
   *  ViewBlock dispatch when called from ViewSpec-driven paths. */
  view?: ContextualView;
  /** Object being inspected — passed to GraphView as `objectId`. */
  objectId?: string;
  /** Pane id for viewport snapshot dispatch. Defaults to
   *  `state.navigation.activePaneId` inside GraphView. */
  paneId?: string;
  /** Active view id — preserved by `SELECT_OBJECT` (H5 invariant). */
  viewId?: string | null;
  /** App dispatch — forwarded to `useAppDispatch` in GraphView.
   *  Optional so renderers that don't need it (json, code, …) can ignore it. */
  dispatch?: React.Dispatch<Action>;
  /** Close callback — surfaced in GraphView's close button (line 73-81). */
  onClose?: () => void;
  /** Object picker — forwarded to interactive components. */
  onSelectObject?: (objectId: string) => void;
}
```

The change MUST be **backward-compatible additive** — existing callers
that pass `undefined` (or no second argument) continue to work. The
JSON, table, tree, code, markdown, vega_lite, and composite entries
MUST NOT need to be modified: their `render` functions ignore `extra`
and continue to work.

`Action` is the existing discriminated union from
`apps/explorer-ui/src/state/context.ts`.

#### Scenario: render() called without extra returns the same React node as before

- **GIVEN** `rendererRegistry.get("json")`
- **WHEN** `entry.render({ foo: "bar" })` is called (no `extra` argument)
- **THEN** the result is identical to the pre-E1.5 result for
  `entry.render({ foo: "bar" })`
- **AND** no TypeScript compilation error occurs (the existing call
  sites at `rendererRegistry.tsx:96, 110, 461` remain valid)

#### Scenario: render() called with RuntimeContext gets the typed shape

- **GIVEN** a caller imports `RuntimeContext` from
  `apps/explorer-ui/src/components/rendererRegistry`
- **AND** constructs `extra: RuntimeContext = { objectId: "sym-1",
  paneId: "pane-1", onClose: () => {} }`
- **WHEN** `rendererRegistry.get("graph").render({ nodes: [...], edges: [...] }, extra)` is called
- **THEN** the GraphView entry forwards `objectId`, `paneId`, `onClose`
  to `<GraphView view={…} objectId={extra.objectId} paneId={extra.paneId}
  onClose={extra.onClose} />`
- **AND** the rendered tree contains `data-testid="graph-view-renderer"`

#### Scenario: TypeScript compatibility — unknown extra keys are tolerated

- **GIVEN** an existing test or external caller passes an untyped
  `Record<string, unknown>` as the second argument (legacy shape)
- **WHEN** `entry.render(body, { unknownKey: 42 } as Record<string, unknown>)` is called
- **THEN** TypeScript's structural typing accepts the call (the
  `RuntimeContext` interface has all-optional fields, so any object
  is assignable up to missing required fields — none are required)
- **AND** the entry's `render` function may treat the extra as
  `Partial<RuntimeContext>` internally

---

### Requirement: REQ-E1.5-3 — PaneInspector removes the `isGraphViewKind()` short-circuit

`apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx`
lines 237–256 currently branch:

```tsx
{display ? (
  isGraphViewKind(display.view_kind) ? (
    <GraphView view={display} objectId={objectId} onClose={onClose} />
  ) : (
    <Blocks view={display} onSelectObject={…} />
  )
) : (
  <p>No view loaded.</p>
)}
```

The `isGraphViewKind` branch MUST be removed. After E1.5, both paths
go through the `Blocks` component, which routes to `GraphView` for
graph-shaped `view_kind` values via the registry entry registered in
REQ-E1.5-1.

Concretely, the body of `PaneInspector.tsx:237-261` collapses to:

```tsx
{display ? (
  <Blocks
    view={display}
    onSelectObject={(objId) =>
      dispatch({
        type: "SELECT_OBJECT",
        // H5: preserve current viewId for drill-down consistency
        // (graph and lists now navigate to the same viewKind).
        payload: { objectId: objId, viewId: viewId ?? display.view_id },
      })
    }
  />
) : (
  <p className="text-sm" style={{ color: "var(--color-text-muted)" }}>
    No view loaded.
  </p>
)}
```

The `<GraphView />` import at `PaneInspector.tsx:21` is removed (no
longer needed).

#### Scenario: Graph-shaped view_kind renders via unified path

- **GIVEN** `display.view_kind === "call_graph"` (one of the 5
  graph-shaped kinds)
- **AND** `display` has `view_id: "call-graph"`, `blocks: [...]`,
  `renderer_kind: "graph"`
- **WHEN** `PaneInspector` renders with this `display`
- **THEN** the rendered DOM contains `data-testid="graph-view-renderer"`
  (same observable outcome as before E1.5)
- **AND** `data-testid="object-inspector-body"` does NOT contain a
  child with `data-testid="view-blocks"` for graph-shaped views (the
  graph path does not render the block list)
- **AND** the existing `GraphViewRenderer.test.tsx` tests at lines 84-150
  continue to pass with **zero test edits**

#### Scenario: Non-graph view_kind still renders blocks

- **GIVEN** `display.view_kind === "vertical_slice"` (NOT a graph shape)
- **WHEN** `PaneInspector` renders
- **THEN** the rendered DOM contains `data-testid="view-blocks"`
  (from `ViewBlock.tsx:230`)
- **AND** per-block testids (`view-block-identity`, etc.) resolve

#### Scenario: Code reading: no `isGraphViewKind` reference in render path

- **GIVEN** the rendered source of `PaneInspector.tsx` after E1.5
- **WHEN** a regex `grep -n "isGraphViewKind" PaneInspector.tsx` runs
- **THEN** zero matches occur inside the render path (lines 135–278)
- **AND** the function `isGraphViewKind` may still be exported from the
  module for documentation purposes (per REQ-E1.5-6) but is not
  invoked

---

### Requirement: REQ-E1.5-4 — `SELECT_OBJECT` preserves `viewId: viewId ?? display.view_id` (H5 invariant)

The `SELECT_OBJECT` dispatch inside `PaneInspector`'s `onSelectObject`
callback (currently `PaneInspector.tsx:247-254`) MUST continue to use:

```ts
payload: { objectId: objId, viewId: viewId ?? display.view_id }
```

After E1.5 the dispatch happens through `Blocks` → registry path (for
graph views via the `graph` entry's `GraphView` → `routing.onSelectObject`
flow), but the **observable** behavior — that `viewId` in the dispatched
`SELECT_OBJECT` payload is either the current `viewId` or the display's
`view_id` — MUST be preserved.

> **Verification note**: The `viewId ?? display.view_id` line lives
> *inside* `PaneInspector.tsx:252` (the non-graph branch). After
> E1.5, the dispatch site moves into `Blocks`'s `onSelectObject`
> callback (still constructed in `PaneInspector` and passed down).
> The fallback expression MUST be preserved verbatim at the new site.

#### Scenario: Selecting an object in a graph view preserves the viewId

- **GIVEN** a graph view is rendered with `viewId: "call-graph"`
- **AND** `display.view_id: "call-graph"`
- **WHEN** the user clicks a graph node and `routing.onSelectObject("callee")`
  fires
- **THEN** `SELECT_OBJECT` is dispatched with payload
  `{ objectId: "callee", viewId: "call-graph" }` (the `?? display.view_id`
  branch resolves because `viewId` is non-null)
- **AND** the dispatched `viewId` matches the current `viewId` (H5
  invariant: drill-down keeps the viewKind)

#### Scenario: Selecting an object when viewId is null

- **GIVEN** `viewId` is `null` and `display.view_id: "overview"`
- **WHEN** the user clicks a block (e.g., a caller row)
- **THEN** `SELECT_OBJECT` is dispatched with payload
  `{ objectId: "sym-1", viewId: "overview" }` (the `?? display.view_id`
  branch resolves to `display.view_id`)

---

### Requirement: REQ-E1.5-5 — Viewport capture and `onClose` propagation

`GraphView`'s two non-rendering behaviors MUST continue to work after
the registry swap:

1. **Viewport capture** — `routing.onViewportChange` (from
   `apps/explorer-ui/src/components/GraphView/routing.ts`) is wired to
   `RenderSvgGraph` at `GraphView.tsx:87`. Viewport snapshots dispatch
   to the app state.
2. **Close button** — when `onClose` is provided, `GraphView.tsx:73-81`
   renders a `<button data-testid="graph-view-close">`. Clicking it
   invokes `onClose`.

Both behaviors are observable through `GraphViewRenderer.test.tsx`
tests at lines 135–150. After E1.5 these tests MUST continue to pass
**without modification** — they exercise the public `GraphView`
component (re-exported as `GraphViewRenderer` from
`GraphView.tsx:94`).

#### Scenario: onClose is wired through registry path

- **GIVEN** `PaneInspector` passes `onClose={onClose}` to the inner
  component (this happens whether directly or through the registry)
- **WHEN** a graph view is rendered and the user clicks
  `data-testid="graph-view-close"`
- **THEN** `onClose` is invoked exactly once (asserted at
  `GraphViewRenderer.test.tsx:148-149`)
- **AND** the resulting dispatch is `CLOSE_PANE` with the active
  pane's id (handler lives in `PaneInspector.tsx`'s consumer)

#### Scenario: Viewport capture dispatches through routing adapter

- **GIVEN** `GraphView` is rendered with `paneId: "pane-1"` (or
  defaults to `state.navigation.activePaneId`)
- **WHEN** `RenderSvgGraph` reports a viewport change (via
  `onViewportChange`)
- **THEN** `routing.onViewportChange` dispatches
  `SET_PANE_VIEWPORT` (or equivalent) with the current viewport bounds
- **AND** the dispatch reaches `state.navigation` (assertion is via the
  pre-existing `GraphViewRenderer.test.tsx` tests at lines 84-127)

#### Scenario: graph-shaped blocks without layout nodes show empty state

- **GIVEN** the layout adapter produces 0 or 1 nodes (e.g., a graph view
  with empty `callees` block)
- **WHEN** the registry-routed `GraphView` renders
- **THEN** `data-testid="graph-empty-state"` is in the DOM (per
  `GraphView.tsx:62` and the empty-state test at
  `GraphViewRenderer.test.tsx:101, 114`)
- **AND** `data-testid="graph-view-renderer"` is NOT in the DOM

---

### Requirement: REQ-E1.5-6 — `isGraphViewKind()` retained as zero-cost helper

The `isGraphViewKind()` function (currently `PaneInspector.tsx:24-32`)
MUST be retained in source — either in `PaneInspector.tsx` or moved
to a shared utility module — but **not used in the render path** after
E1.5.

Rationale (from proposal defaults):
- The function is a tiny string-equality check, zero-cost at runtime.
- It documents which `ViewKind` values are "graph-shaped" — useful for
  future tests, lint rules, and the E7 WebGL migration.
- Removing it would lose domain-language fidelity for downstream
  readers.

The function MUST continue to recognize all 5 graph-shaped ViewKinds:
`call_graph`, `dependency_graph`, `data_flow`, `impact_radius`,
`seam_map`.

#### Scenario: `isGraphViewKind` returns true for the 5 graph-shaped kinds

- **GIVEN** the function is imported from its retained location
- **WHEN** called with each of `"call_graph"`, `"dependency_graph"`,
  `"data_flow"`, `"impact_radius"`, `"seam_map"`
- **THEN** it returns `true`
- **AND** called with any other `ViewKind` (e.g., `"vertical_slice"`,
  `"source_view"`, `"c4_context"`), it returns `false`

#### Scenario: `isGraphViewKind` is unused in the render path

- **GIVEN** `PaneInspector.tsx` after E1.5
- **WHEN** the source is searched for `isGraphViewKind`
- **THEN** the function is **defined** but **not invoked** anywhere in
  the render path (lines 135–278)
- **AND** the function is **exported** (so test files, future
  refactors, and the E7 migration can consume it)

---

## Invariants Covered

| Invariant | Source | Scenario |
|-----------|--------|----------|
| `data-testid="graph-view-renderer"` present when layout has >1 node | `GraphView.tsx:67`; asserted in `GraphViewRenderer.test.tsx:88, 124` | REQ-E1.5-1, REQ-E1.5-3, REQ-E1.5-5 |
| `data-testid="graph-empty-state"` when layout has ≤1 node | `GraphView.tsx:62`; asserted in `GraphViewRenderer.test.tsx:101, 114` | REQ-E1.5-5 |
| `data-testid="graph-view-close"` + onClose fires | `GraphView.tsx:73-81`; asserted in `GraphViewRenderer.test.tsx:140, 148-149` | REQ-E1.5-5 |
| `SELECT_OBJECT` preserves `viewId ?? display.view_id` (H5) | `PaneInspector.tsx:252`; cross-ref to `PaneStackView.test.tsx` | REQ-E1.5-4 |
| Registry has all 8 built-in renderers, `entries().length >= 8` | `rendererRegistry.tsx:117-188`; asserted in `rendererRegistry.test.tsx:114-118` | REQ-E1.5-1 |
| Renderers that ignore `extra` (json, table, tree, code, markdown, vega_lite, composite) are unchanged | `rendererRegistry.tsx:127-187` | REQ-E1.5-2 |

---

## Affected Files

| File | LOC | Change |
|------|-----|--------|
| `apps/explorer-ui/src/components/rendererRegistry.tsx` | 473 | Lines 19 (lazy import of InteractiveGraph) and 227–273 (GraphRenderer stub) removed; lines 120–123 rewritten to register `GraphView`; `RuntimeContext` interface added near line 38; `RendererEntry.render` signature extended (backward-compatible) |
| `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx` | 279 | Line 21 (GraphView import) removed; lines 24–32 (`isGraphViewKind`) extracted to a separate exported helper OR retained in the file but unused in the render path; lines 237–256 collapsed to a single `<Blocks>` call |
| `apps/explorer-ui/src/components/GraphView/GraphView.tsx` | 94 | No source change. The component is now the registry's `graph` entry. |
| `apps/explorer-ui/src/components/GraphView/routing.ts` | unchanged | No change — the dispatch routing adapter continues to be used. |
| `apps/explorer-ui/src/components/GraphView/layout.ts` | unchanged | No change — the layout adapter continues to be used. |
| `apps/explorer-ui/src/components/rendererRegistry.test.tsx` | 160 | Add 1 test: `entry.render({...})` for `"graph"` returns a node containing `data-testid="graph-view-renderer"` when given a runtime context (no other edits) |

---

## Out of Scope

- Reconciling SvgGraph vs Cytoscape backends at the ADR level — that
  is ADR-043, scheduled post-implementation per the proposal default.
- Upgrading `InteractiveGraph` to feature parity with `GraphView`
  (Option B in proposal — explicitly rejected).
- Removing `InteractiveGraph` from the codebase (the
  `apps/explorer-ui/src/components/InteractiveGraph` module is **not**
  in scope for deletion — it is part of the Cytoscape backend that ADR-043
  will reconcile).
- The block-id registry refactor (E1.4) — separate spec.
- Vega-Lite wiring (Phase 4).
- Promoting ADR-008 from PROPOSED to ACCEPTED (governance).

---

## Open Questions

1. **`isGraphViewKind` location** — proposal says retain it. Two
   concrete options: (a) keep in `PaneInspector.tsx` but unused; (b)
   extract to `apps/explorer-ui/src/components/ObjectInspector/viewKind.ts`
   for cleaner namespace. The latter is preferred for testability
   and for downstream consumers (the E7 WebGL migration will need to
   check graph-shaped kinds). Recommend (b).
2. **Bundle implications** — removing the `lazy(() => import("./InteractiveGraph"))`
   at `rendererRegistry.tsx:19` removes cytoscape from the registry
   path. If any other code path imports `InteractiveGraph` (e.g.,
   legacy routes, ViewSpec testing), they continue to work because the
   module file itself is not deleted. But cytoscape **may** become
   dead-code in the production bundle — verifiable by a build check
   (no Cytoscape chunks). Out of scope to fix here; flagged for ADR-043.

---

## Acceptance Criteria (Given/When/Then)

These are the executable checks that gate E1.5 acceptance:

1. **Given** `rendererRegistry.get("graph")` after E1.5,
   **When** `entry.render({nodes: [], edges: []}, { objectId: "sym-1", paneId: "pane-1", onClose: () => {} })` is called
   in a test harness with `AppContext.Provider`,
   **Then** the rendered DOM contains `data-testid="graph-view-renderer"`.
2. **Given** `PaneInspector.tsx` source after E1.5,
   **When** `grep -n "isGraphViewKind" PaneInspector.tsx` runs,
   **Then** zero matches occur **inside the render function body**
   (the definition may exist if retained).
3. **Given** a graph-shaped `display.view_kind` (`call_graph` etc.),
   **When** `PaneInspector` renders,
   **Then** the existing 7 `GraphViewRenderer.test.tsx` tests pass
   unchanged.
4. **Given** the existing `rendererRegistry.test.tsx` (160 LOC),
   **When** run after E1.5,
   **Then** all assertions pass — specifically
   `rendererRegistry.entries().length >= 8`,
   `"graph"` is registered, and
   `rendererRegistry.get("graph").render({nodes: [], edges: []})` does
   not throw.
5. **Given** the `RendererEntry.render` signature changed from
   `(body, extra?: Record<string, unknown>)` to
   `(body, extra?: RuntimeContext)`,
   **When** the codebase is compiled with `tsc --noEmit`,
   **Then** no existing call sites fail (the change is
   backward-compatible additive).
6. **Given** the SELECT_OBJECT invariant (H5),
   **When** a node is clicked in a graph view OR a row is clicked in
   a block (callers/callees/hotspots/quality_issue_detail),
   **Then** the dispatched payload is
   `{ objectId: objId, viewId: viewId ?? display.view_id }` — the
   fallback expression is preserved verbatim.
7. **Given** the 5 pre-existing test failures and 39 lint errors are
   excluded from scope,
   **When** running `pnpm test src/components/ObjectInspector src/components/GraphView src/rendererRegistry`,
   **Then** all tests pass.