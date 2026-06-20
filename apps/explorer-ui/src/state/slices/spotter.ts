/**
 * Spotter slice — toggle/visibility state for the Spotter palette.
 *
 * Handles: TOGGLE_SPOTTER, SET_SPOTTER, RESET
 */
import type { Action } from "../context";

export type SpotterAction = Extract<
  Action,
  { type: "TOGGLE_SPOTTER" } | { type: "SET_SPOTTER" } | { type: "RESET" }
>;

export function spotterReducer(state: boolean, action: Action): boolean {
  switch (action.type) {
    case "TOGGLE_SPOTTER":
      return !state;
    case "SET_SPOTTER":
      return action.payload.open;
    case "RESET":
      return false;
    default:
      return state;
  }
}
