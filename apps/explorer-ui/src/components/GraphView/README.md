# GraphView — Interactive Graph Rendering

## Overview

`GraphView` renders `ContextualView` objects whose `view_kind` is a structural graph type (call graph, dependency graph, data flow, etc.) as an interactive SVG via `SvgGraph`.

## Components

### `GraphViewRenderer`

The main component. Receives a `ContextualView` and renders it as an interactive graph.

**Props:**

```typescript
interface Props {
  /** The contextual view to render */
  view: ContextualView;
  /** The object ID this view is showing */
  objectId: string;
  /** Optional pane ID for viewport snapshot dispatch */
  paneId?: string;
  /** Close callback (pane-stack mode) */
  onClose?: () => void;
}
```

**Responsibilities:**

1. **Route** structural ViewKinds to `SvgGraph` (bypass the block-based `Blocks` renderer)
2. **Compute layout** from the `ContextualView` using `layoutFromContextualView`
3. **Handle node clicks** by dispatching `SELECT_OBJECT` to open a new pane
4. **Capture viewport** changes via `onViewportChange` callback for snapshot persistence
5. **Render empty state** when the graph has ≤1 nodes

### `GraphEmptyState`

Shown when a graph view has no connections to display.

## ViewKind Routing

`GraphViewRenderer` is used by `PaneInspector` when the view's `view_kind` matches one of:

- `call_graph`
- `dependency_graph`
- `data_flow`
- `impact_radius`
- `seam_map`

The routing check in `PaneInspector`:

```typescript
function isGraphViewKind(kind: string | undefined): boolean {
  return (
    kind === "call_graph" ||
    kind === "dependency_graph" ||
    kind === "data_flow" ||
    kind === "impact_radius" ||
    kind === "seam_map"
  );
}
```

## Viewport Capture (ADR-040 Wave 3)

When a user pans or zooms the graph, `GraphViewRenderer` receives the new viewport via `onViewportChange` and dispatches `UPDATE_PANE_VIEWPORT`:

```typescript
const handleViewportChange = useCallback(
  (viewport: ViewportState) => {
    if (effectivePaneId) {
      dispatch({
        type: "UPDATE_PANE_VIEWPORT",
        payload: { paneId: effectivePaneId, viewport },
      });
    }
  },
  [dispatch, effectivePaneId],
);
```

The viewport state `{ x, y, scale }` is stored in the `Pane` state and included in `ExplorationSession` saves.

## Testing

- **Unit tests**: `GraphViewRenderer.test.tsx` — rendering, empty state, node selection
- **E2E tests**: `exploration-snapshot.spec.ts` — multi-pane, viewport capture, session persistence

## File Structure

```
GraphView/
├── GraphEmptyState.tsx      # Empty state component
├── GraphViewRenderer.tsx    # Main renderer component
└── GraphViewRenderer.test.tsx # Unit tests
```
