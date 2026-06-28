/**
 * Spotter slice — open/visibility + kind filter state for the Spotter palette.
 *
 * Handles: TOGGLE_SPOTTER, SET_SPOTTER, RESET
 */
import type { Action } from "../context";

export type SpotterAction = Extract<
  Action,
  { type: "TOGGLE_SPOTTER" } | { type: "SET_SPOTTER" } | { type: "RESET" }
>;

export interface SpotterState {
  open: boolean;
  kind: string | null;
}

export const initialSpotterState: SpotterState = { open: false, kind: null };

export function spotterReducer(state: SpotterState, action: Action): SpotterState {
  switch (action.type) {
    case "TOGGLE_SPOTTER":
      return { open: !state.open, kind: state.open ? null : state.kind };
    case "SET_SPOTTER":
      return {
        open: action.payload.open,
        kind: action.payload.open ? (action.payload.kind ?? state.kind) : null,
      };
    case "RESET":
      return initialSpotterState;
    default:
      return state;
  }
}
