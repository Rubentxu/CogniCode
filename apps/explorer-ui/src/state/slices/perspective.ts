/**
 * Perspective slice — landing page view mode (graph or c4 level).
 *
 * Handles: SET_PERSPECTIVE, RESET
 */
import type { Action } from "../context";
import type { Perspective } from "../c4Levels";

export type PerspectiveAction = Extract<
  Action,
  { type: "SET_PERSPECTIVE" } | { type: "RESET" }
>;

export function perspectiveReducer(
  state: Perspective,
  action: Action
): Perspective {
  switch (action.type) {
    case "SET_PERSPECTIVE":
      return action.payload;
    case "RESET":
      return "graph";
    default:
      return state;
  }
}
