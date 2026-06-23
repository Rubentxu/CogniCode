/**
 * Navigation factory + persistence — pane-stack only.
 *
 * Column mode has been removed (ADR-039 E3).
 */
import { apply, getActiveFocus, hasFocus, MAX_PANES } from "./paneStack";
import { makeInitialNavigationState } from "./types";
import type { NavigationAction, NavigationState, Pane, Focus, ViewportState } from "./types";

export { apply, getActiveFocus, hasFocus, MAX_PANES };
export { makeInitialNavigationState };
export type { NavigationAction, NavigationState, Pane, Focus, ViewportState };

// Remove legacy localStorage key on module load
if (typeof window !== "undefined") {
  try {
    window.localStorage.removeItem("cognicode.navigationMode.v1");
  } catch {
    // ignore
  }
}
