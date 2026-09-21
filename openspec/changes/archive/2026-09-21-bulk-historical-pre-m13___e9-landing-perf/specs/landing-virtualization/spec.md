# Spec: e9 — Landing Node-List Virtualization

## Purpose

Virtualize the node-list fallback in `GraphLanding` to prevent DOM bloat when
`LandingPayload.nodes` exceeds ~200 items. The existing `LANDING_NODE_CAP = 50`
constant in the backend means the landing endpoint already caps at 50 nodes, but
the E8 verify-report flagged W-3: "virtualise node-list fallback for
workspaces >500 nodes" — indicating real-world usage may exceed this in
subsequent paging or wider workspaces.

This spec covers only the frontend virtualization. The backend `LANDING_NODE_CAP`
stays unchanged.

---

## ADDED Requirements

### Requirement: 1. Node-List Virtualization Threshold

The `GraphLanding` component MUST render the node-list fallback using a
virtualized list when `data.nodes.length > 200`. Below or equal to 200, the
current flat `.map()` rendering is acceptable.

The threshold of 200 is chosen as a safe margin below the backend cap of 50,
accounting for future API changes where the cap might increase.

#### Scenario: Virtualization activates above threshold

- GIVEN `landingData.nodes.length === 300`
- WHEN `GraphLanding` renders
- THEN a virtualized list renders only visible nodes (window of ~20 items)
- AND the list scrolls smoothly in the fallback container
- AND all 300 nodes remain navigable via keyboard/scroll

#### Scenario: Flat rendering below threshold

- GIVEN `landingData.nodes.length === 50`
- WHEN `GraphLanding` renders
- THEN the current flat `.map()` renders all 50 buttons directly
- AND no virtualization overhead is incurred

### Requirement: 2. Virtualization Container Behaviour

The virtualized list MUST:

1. Preserve the existing visual styling (`.flex.flex-wrap.gap-2.px-3.py-2`,
   button styles, `data-testid="graph-node-{id}"` per node).
2. Maintain keyboard navigability (Tab through visible items, Enter to select).
3. Preserve `data-testid` attributes on each rendered button for test stability.
4. Show a scroll indicator when the full list exceeds the visible viewport.
5. Use a fixed row height estimation (40px per row, 8 columns) for the
   virtualization window.

### Requirement: 3. Scroll Position Reset on Data Change

When the `landingData` changes (new workspace, refresh), the node-list scroll
position MUST reset to the top.

#### Scenario: Scroll resets on payload change

- GIVEN the node list is scrolled to position Y
- WHEN the user navigates to a different workspace (new `landingData`)
- THEN the node list scroll position resets to 0
- AND the first visible nodes correspond to `nodes[0]` in the new payload

---

## UNCHANGED Requirements

- The `data-testid="graph-landing-node-list"` container testid is preserved.
- The per-node `data-testid="graph-node-{node.id}"` is preserved.
- The truncation banner behaviour (Req 1 of `graphlanding-affordances/spec.md`) is
  unchanged.
- The `onClick` → `selectObject(node.id)` behaviour is unchanged.

---

## Implementation Notes

- Use a lightweight virtualization approach (fixed-size window, no heavy library).
- A simple "render window" approach: compute `startIndex = floor(scrollTop / ROW_HEIGHT)`,
  render items from `startIndex` to `startIndex + WINDOW_SIZE`, use `paddingTop`
  and `paddingBottom` to maintain scrollable height.
- WINDOW_SIZE = 20 items. ROW_HEIGHT = 28px + 8px gap = 36px.
- Keep the implementation focused to avoid over-engineering (ponytail).

---

## Acceptance Criteria

- [ ] `landingData.nodes.length === 300` renders without freezing the main thread.
- [ ] All nodes remain reachable via scroll (full height preserved).
- [ ] `data-testid="graph-node-{id}"` exists in DOM for each node when visible.
- [ ] Keyboard navigation (Tab / Shift+Tab) works through visible items.
- [ ] `just explorer-test` (vitest) passes.
- [ ] `just explorer-e2e` passes.
