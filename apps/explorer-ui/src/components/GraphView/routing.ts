/**
 * Routing adapter — injects navigation dispatch strategy.
 *
 * The default implementation dispatches to the real Redux-style reducer
 * via `useAppDispatch`. Pass a custom adapter when testing or when
 * the component is used outside the Explorer shell.
 */
import type { ViewportState } from "../../state/navigation/types";
import type { Action } from "../../state/context";

export interface RoutingAdapter {
  onSelectObject(nodeId: string): void;
  onViewportChange(viewport: ViewportState): void;
}

export function createDispatchRouting(
  dispatch: React.Dispatch<Action>,
  paneId: string | null,
  viewId: string | null,
): RoutingAdapter {
  return {
    onSelectObject: (nodeId) => {
      dispatch({
        type: "SELECT_OBJECT",
        payload: { objectId: nodeId, viewId: viewId ?? undefined },
      });
    },
    onViewportChange: (viewport) => {
      if (paneId) {
        dispatch({
          type: "UPDATE_PANE_VIEWPORT",
          payload: { paneId, viewport },
        });
      }
    },
  };
}
