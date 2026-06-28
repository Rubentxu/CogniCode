/**
 * Landing Workbench slice — active tab in the LandingWorkbench.
 *
 * The landing page is a tabbed workbench (Start From / Investigations /
 * Resume / Graph). This slice holds which tab is active. The Graph tab
 * embeds the existing GraphLanding component unchanged.
 *
 * Handles: SET_LANDING_TAB, RESET
 */
import type { Action } from "../context";

export type LandingTabId = "start" | "investigations" | "resume" | "graph";

export interface LandingWorkbenchState {
  activeTab: LandingTabId;
}

export const initialLandingWorkbenchState: LandingWorkbenchState = {
  activeTab: "graph",
};

export type LandingWorkbenchAction = Extract<
  Action,
  { type: "SET_LANDING_TAB" } | { type: "RESET" }
>;

export function landingWorkbenchReducer(
  state: LandingWorkbenchState,
  action: Action
): LandingWorkbenchState {
  switch (action.type) {
    case "SET_LANDING_TAB":
      return { activeTab: action.payload.tab };
    case "RESET":
      return initialLandingWorkbenchState;
    default:
      return state;
  }
}
