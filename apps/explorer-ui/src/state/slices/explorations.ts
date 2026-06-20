/**
 * Explorations slice — saved exploration paths.
 *
 * Handles: ADD_EXPLORATION, RESET
 */
import type { Action } from "../context";
import type { ExplorationPath } from "../../api/types";

export type ExplorationsAction = Extract<
  Action,
  { type: "ADD_EXPLORATION" } | { type: "RESET" }
>;

export function explorationsReducer(
  state: ExplorationPath[],
  action: Action
): ExplorationPath[] {
  switch (action.type) {
    case "ADD_EXPLORATION":
      return [...state, action.payload];
    case "RESET":
      return [];
    default:
      return state;
  }
}
