# Kernel Specs: GraphViewRenderer

## Router Context Used
- Knowledge Coverage: sufficient (ADR-040, roadmap MOLDABLE-VIEW-PANE-STATE-2026, UX wireframes, CONTEXT.md)
- Context Quality: C2
- Taxonomy: routing-gap, schema-stamp, persistence
- Domain Language: GraphViewRenderer, ViewKind routing, Pane Stack, ContextualView, SvgGraph, ViewportState (all resolved)
- Recommended Effort: deepen

## Knowledge Provenance
- Scope source: `openspec/changes/moldable-view-call-graph/proposal.md` (Capabilities, Approach, Affected Areas)
- Invariant source: `docs/adr/ADR-040-graph-view-renderer.md` + `docs/roadmap/MOLDABLE-VIEW-PANE-STATE-2026.md` + grill session 2026-06-20
- Memory-only hints excluded from spec truth: None — all routing/invariant facts are ADR-backed

## Capability: graph-view-routing

### Requirement: GraphViewRenderer routes graph views
The PaneInspector component SHALL route `ContextualView` with `view_kind` in the set `{call_graph, dependency_graph, data_flow, impact_radius, seam_map}` to the GraphViewRenderer component instead of the Blocks renderer.

#### Scenario: call_graph view is rendered via GraphViewRenderer
- GIVEN a `ContextualView` with `view_kind === "call_graph"` and at least one node
- WHEN the PaneInspector renders the active view
- THEN the SvgGraph component SHALL display the graph with computed layout
- AND the metadata blocks (identity, call_metrics, signature) SHALL be bypassed

#### Scenario: empty graph shows empty state
- GIVEN a `ContextualView` with `view_kind === "call_graph"` and `layout.nodes.length <= 1`
- WHEN the GraphViewRenderer renders
- THEN the GraphEmptyState component SHALL be displayed
- AND the empty state SHALL include a message and a link to "Overview" view

#### Scenario: click on node opens new pane
- GIVEN a user viewing a call_graph with at least 2 nodes
- WHEN the user clicks on a non-root node
- THEN `SELECT_OBJECT` SHALL be dispatched with `payload.objectId = nodeId`
- AND `payload.viewId` SHALL be the current `view.id` (preserves view context)
- AND a new pane SHALL be added to the PaneStack
- AND the new pane SHALL become active

### Requirement: ContextualView schema validates view_kind
The Zod schema `contextualViewSchema` SHALL validate an optional `view_kind` field.

#### Scenario: backend sends view_kind
- GIVEN a JSON payload with `view_kind: "call_graph"`
- WHEN the schema validates the payload
- THEN the resulting object SHALL have `view_kind === "call_graph"`

#### Scenario: backend omits view_kind (legacy)
- GIVEN a JSON payload without `view_kind`
- WHEN the schema validates the payload
- THEN the resulting object SHALL have `view_kind === undefined`
- AND no validation error SHALL be thrown

### Requirement: Backend stamps view_kind in ContextualView DTO
The Rust `ContextualView` struct SHALL include a `view_kind: ViewKind` field, and the service layer SHALL stamp it from the `ViewDescriptor` after `build()`.

#### Scenario: get_object_view returns view_kind
- GIVEN a call graph view is requested
- WHEN `get_object_view` returns the response
- THEN the JSON SHALL contain `view_kind: "call_graph"`
- AND `renderer_kind` SHALL be `"graph"`

#### Scenario: missing descriptor metadata
- GIVEN a view is requested but `ViewDescriptor` is unavailable
- WHEN `get_object_view` returns the response
- THEN `view_kind` SHALL default to `RendererKind::Json` for backward safety
- AND `renderer_kind` SHALL default to `RendererKind::Json`

### Requirement: Layout memoization
GraphViewRenderer SHALL memoize the layout calculation using `useMemo` with dependencies `[view.object_id, view.blocks]`.

#### Scenario: re-render with same object_id and blocks
- GIVEN a re-render is triggered by SWR validating state
- WHEN `view.object_id` and `view.blocks` references are unchanged
- THEN the layout SHALL NOT be recalculated

#### Scenario: blocks change (new callers/callees)
- GIVEN a re-render with new `view.blocks` array
- WHEN the blocks reference changes
- THEN the layout SHALL be recalculated

## Invariants Covered
- `MAX_PANES = 8` cap — verified by `click on node opens new pane` (regression test for pane-stack.spec.ts)
- `SELECT_OBJECT` deduplicates by `objectId` — verified by `click on node opens new pane`
- Edge label highlight-only (Decision 6) — **out of scope for this spec; tracked in tasks phase**
- Routing set is closed (`call_graph, dependency_graph, data_flow, impact_radius, seam_map`) — verified by `GraphViewRenderer routes graph views`

## Open Questions
- None. Schema gap was resolved by the corrected Task 0 (backend stamp + frontend schema + MSW fixtures) per proposal §"Knowledge Decisions".
