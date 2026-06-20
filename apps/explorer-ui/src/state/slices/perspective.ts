/**
 * Perspective slice — landing page view mode (graph or c4).
 *
 * Handles: SET_PERSPECTIVE, RESET
 */
import type { Action } from "../context";

export type PerspectiveAction = Extract<
  Action,
  { type: "SET_PERSPECTIVE" } | { type: "RESET" }
>;

export function perspectiveReducer(
  state: "graph" | "c4",
  action: Action
): "graph" | "c4" {
  switch (action.type) {
    case "SET_PERSPECTIVE":
      return action.payload;
    case "RESET":
      return "graph";
    default:
      return state;
  }
}
