/**
 * GraphView — unified deep module for graph-shaped ViewKinds.
 *
 * Consolidates GraphViewRenderer (84 LOC) and SvgGraph (424 LOC) into
 * a single functional unit with internal adapters:
 * - layout.ts: LayoutAdapter (inject layoutFromContextualView or mock)
 * - render.tsx: SVG rendering with pan/zoom
 * - routing.tsx: RoutingAdapter (dispatch to navigation)
 *
 * Part of the Moldable Development exploration flow. Click on a node
 * dispatches SELECT_OBJECT which opens a new pane in the PaneStack
 * (preserves exploration history).
 */
import { useMemo } from "react";
import { useApp, useAppDispatch } from "../../state/context";
import type { ContextualView } from "../../api/types";
import { GraphEmptyState } from "./GraphEmptyState";
import { RenderSvgGraph } from "./render";
import { createDispatchRouting, type RoutingAdapter } from "./routing";
import { type LayoutAdapter, defaultLayoutAdapter } from "./layout";

export interface GraphViewProps {
  view: ContextualView;
  objectId: string;
  /** Pane ID for viewport snapshot dispatch. Uses activePaneId from state if omitted. */
  paneId?: string;
  onClose?: () => void;
  /** Layout strategy. Defaults to the deterministic circular-layout mock. */
  layoutAdapter?: LayoutAdapter;
  /** Routing strategy. Defaults to dispatch-based navigation. */
  routingAdapter?: RoutingAdapter;
}

/**
 * GraphView — renders a ContextualView with a graph-shaped ViewKind
 * as an interactive SVG graph.
 */
export function GraphView({
  view,
  objectId,
  paneId,
  onClose,
  layoutAdapter = defaultLayoutAdapter,
  routingAdapter,
}: GraphViewProps) {
  const dispatch = useAppDispatch();
  const { state } = useApp();
  const activePaneId = state.navigation.activePaneId;
  const effectivePaneId = paneId ?? activePaneId;

  const routing: RoutingAdapter = useMemo(
    () => routingAdapter ?? createDispatchRouting(dispatch, effectivePaneId, view.view_id),
    [dispatch, effectivePaneId, view.view_id, routingAdapter],
  );

  const layout = useMemo(
    () => layoutAdapter.compute(view),
    [layoutAdapter, view.object_id, view.blocks],
  );

  if (layout.nodes.length <= 1) {
    return <GraphEmptyState />;
  }

  return (
    <div
      data-testid="graph-view-renderer"
      className="flex h-full flex-col"
    >
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
      <RenderSvgGraph
        layout={layout}
        selectedId={objectId}
        onSelectObject={routing.onSelectObject}
        onViewportChange={routing.onViewportChange}
      />
    </div>
  );
}

// Re-export aliases for backward compatibility
export { GraphView as GraphViewRenderer } from "./GraphView";
