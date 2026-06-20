/**
 * GraphViewRenderer — renders a ContextualView with view_kind
 * in the graph set as an interactive SvgGraph.
 *
 * Part of the Moldable Development exploration flow. Click on
 * a node dispatches SELECT_OBJECT which opens a new pane in the
 * PaneStack (preserves exploration history).
 *
 * Routing happens in PaneInspector (early-return after ViewTabs).
 */
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