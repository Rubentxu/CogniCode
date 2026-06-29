/**
 * `c4Overlay` slice — controls C4 overlay visibility (drift + hotspots).
 *
 * These toggles control whether drift findings and hotspot scores are
 * rendered on top of the C4 graph view.
 */

export interface C4OverlayState {
  driftEnabled: boolean;
  hotspotsEnabled: boolean;
  boundaryViolationsEnabled: boolean;
}

export const initialC4OverlayState: C4OverlayState = {
  driftEnabled: false,
  hotspotsEnabled: false,
  boundaryViolationsEnabled: false,
};

export type C4OverlayAction =
  | { type: "c4-overlay/toggleDrift" }
  | { type: "c4-overlay/toggleHotspots" }
  | { type: "c4-overlay/toggleBoundaryViolations" };

export function c4OverlayReducer(
  state: C4OverlayState = initialC4OverlayState,
  action: C4OverlayAction,
): C4OverlayState {
  switch (action.type) {
    case "c4-overlay/toggleDrift":
      return { ...state, driftEnabled: !state.driftEnabled };
    case "c4-overlay/toggleHotspots":
      return { ...state, hotspotsEnabled: !state.hotspotsEnabled };
    case "c4-overlay/toggleBoundaryViolations":
      return { ...state, boundaryViolationsEnabled: !state.boundaryViolationsEnabled };
    default:
      return state;
  }
}
