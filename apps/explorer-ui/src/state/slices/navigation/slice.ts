/**
 * Navigation slice — pane-stack navigation actions.
 *
 * Handles: SELECT_OBJECT, SET_ACTIVE_VIEW, SET_ACTIVE_LENS, PUSH_PANE,
 * CLOSE_PANE, ACTIVATE_PANE, REORDER_PANE, SET_PANE_SCROLL, UPDATE_PANE_VIEWPORT
 *
 * RESET is also handled here to reset navigation to initial state.
 */
import type { Action } from "../../context";
import { apply, getActiveFocus, makeInitialNavigationState } from "./reducer";
import type { NavigationAction, NavigationState } from "./types";

export type NavigationSliceAction = Extract<
  Action,
  | { type: "SELECT_OBJECT" }
  | { type: "SET_ACTIVE_VIEW" }
  | { type: "SET_ACTIVE_LENS" }
  | { type: "PUSH_PANE" }
  | { type: "CLOSE_PANE" }
  | { type: "ACTIVATE_PANE" }
  | { type: "REORDER_PANE" }
  | { type: "SET_PANE_SCROLL" }
  | { type: "UPDATE_PANE_VIEWPORT" }
  | { type: "RESET" }
>;

function toNavigationAction(
  action: NavigationSliceAction
): NavigationAction {
  switch (action.type) {
    case "SELECT_OBJECT":
      return { type: "SELECT_OBJECT", payload: action.payload };
    case "SET_ACTIVE_VIEW":
      return { type: "SET_ACTIVE_VIEW", payload: action.payload };
    case "SET_ACTIVE_LENS":
      return { type: "SET_ACTIVE_LENS", payload: action.payload };
    case "PUSH_PANE":
      return { type: "PUSH_PANE", payload: action.payload };
    case "CLOSE_PANE":
      return { type: "CLOSE_PANE", payload: action.payload };
    case "ACTIVATE_PANE":
      return { type: "ACTIVATE_PANE", payload: action.payload };
    case "REORDER_PANE":
      return { type: "REORDER_PANE", payload: action.payload };
    case "SET_PANE_SCROLL":
      return { type: "SET_PANE_SCROLL", payload: action.payload };
    case "UPDATE_PANE_VIEWPORT":
      return { type: "UPDATE_PANE_VIEWPORT", payload: action.payload };
    case "RESET":
      return { type: "RESET" };
  }
}

export type NavigationSliceState = {
  navigation: NavigationState;
  activeObjectId: string | null;
  activeViewId: string | null;
  activeLensId: string | null;
  activeView: NavigationState["panes"][number]["activeView"] | null;
};

export function navigationReducer(
  state: NavigationSliceState,
  action: Action
): NavigationSliceState {
  // Only handle navigation actions
  if (!isNavigationAction(action)) {
    return state;
  }

  const navAction = toNavigationAction(action);
  const navigation = apply(state.navigation, navAction);
  const focus = getActiveFocus(navigation);

  return {
    navigation,
    activeObjectId: focus.objectId,
    activeViewId: focus.viewId,
    activeLensId: focus.lensId,
    activeView: focus.view,
  };
}

function isNavigationAction(
  action: Action
): action is NavigationSliceAction {
  return (
    action.type === "SELECT_OBJECT" ||
    action.type === "SET_ACTIVE_VIEW" ||
    action.type === "SET_ACTIVE_LENS" ||
    action.type === "PUSH_PANE" ||
    action.type === "CLOSE_PANE" ||
    action.type === "ACTIVATE_PANE" ||
    action.type === "REORDER_PANE" ||
    action.type === "SET_PANE_SCROLL" ||
    action.type === "UPDATE_PANE_VIEWPORT" ||
    action.type === "RESET"
  );
}

export function makeInitialNavigationSliceState(): NavigationSliceState {
  return {
    navigation: makeInitialNavigationState(),
    activeObjectId: null,
    activeViewId: null,
    activeLensId: null,
    activeView: null,
  };
}
