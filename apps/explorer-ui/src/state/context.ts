/**
 * App-level state shape, the reducer, and a typed Context.
 *
 * Single source of truth for the Explorer UI. The Shell reads from
 * `useAppState()`; interactions dispatch through `useAppDispatch()`.
 *
 * Actions are deliberately narrow and serialisable. Persisting the
 * whole state to a future server-side path is a one-liner.
 *
 * Navigation uses pane-stack mode only (column mode removed, ADR-039 E3).
 */
import { createContext, useContext, useReducer } from "react";
import type {
  ContextualView,
  ExplorationPath,
  WorkspaceSummary,
} from "../api/types";
import {
  apply,
  getActiveFocus,
  makeInitialNavigationState,
  type NavigationAction,
  type NavigationState,
} from "./navigation";

// ============================================================================
// State
// ============================================================================

export type AppState = {
  workspace: WorkspaceSummary | null;
  /**
   * Navigation state — pane-stack navigation.
   */
  navigation: NavigationState;
  /**
   * Active object id (the focused pane). Cached at the top level
   * for consumers that don't want to dig into `navigation`.
   */
  activeObjectId: string | null;
  /** The view id selected for `activeObjectId`, if any. */
  activeViewId: string | null;
  /** The lens id the user has applied to the active view, if any. */
  activeLensId: string | null;
  /** Whether the Spotter palette is open. */
  spotterOpen: boolean;
  /**
   * The last fully resolved contextual view — cached so the UI can
   * re-render instantly while SWR revalidates in the background.
   */
  activeView: ContextualView | null;
  /** Saved explorations the user has minted during the session. */
  explorations: ExplorationPath[];
  /**
   * Landing page perspective — graph (entry points) or c4 (component directories).
   * Toggle only applies when no object is selected (landing view).
   */
  perspective: "graph" | "c4";
};

/**
 * Selector for the column chain. Returns `state.navigation.chain`.
 */
export function selectChain(state: AppState): NavigationState["chain"] {
  return state.navigation.chain;
}

/**
 * Build an initial AppState focused on a given object. Used by
 * tests that want to skip the navigation setup and jump straight
 * to "object is selected, inspector is showing it".
 *
 * Opens a single pane with the object.
 */
export function initialStateWithFocus(
  activeObjectId: string,
  viewId: string | null = null,
  kind: string = "symbol",
): AppState {
  const base = makeInitialNavigationState();
  const navAction: NavigationAction = {
    type: "PUSH_PANE",
    payload: { objectId: activeObjectId, viewId: viewId ?? undefined, kind },
  };
  const nav = apply(base, navAction);
  const focus = getActiveFocus(nav);
  return {
    workspace: null,
    navigation: nav,
    activeObjectId: focus.objectId,
    activeViewId: focus.viewId,
    activeLensId: focus.lensId,
    spotterOpen: false,
    activeView: null,
    explorations: [],
    perspective: "graph",
  };
}

// ============================================================================
// Actions
// ============================================================================

/**
 * Public action vocabulary.
 */
export type Action =
  | { type: "SET_WORKSPACE"; payload: WorkspaceSummary }
  | {
      type: "SELECT_OBJECT";
      payload: { objectId: string; viewId?: string; kind?: string };
    }
  | { type: "SET_ACTIVE_VIEW"; payload: ContextualView }
  | { type: "SET_ACTIVE_LENS"; payload: { lensId: string | null } }
  | { type: "PUSH_PANE"; payload: { objectId: string; viewId?: string; kind?: string } }
  | { type: "CLOSE_PANE"; payload: { paneId: string } }
  | { type: "ACTIVATE_PANE"; payload: { paneId: string } }
  | { type: "REORDER_PANE"; payload: { fromIndex: number; toIndex: number } }
  | { type: "SET_PANE_SCROLL"; payload: { paneId: string; scrollY: number } }
  | { type: "TOGGLE_SPOTTER" }
  | { type: "SET_SPOTTER"; payload: { open: boolean } }
  | { type: "ADD_EXPLORATION"; payload: ExplorationPath }
  | { type: "RESET" }
  | { type: "SET_PERSPECTIVE"; payload: "graph" | "c4" };

// ============================================================================
// Reducer
// ============================================================================

/**
 * Initial state. Navigation mode is always "pane-stack".
 */
export const initialState: AppState = {
  workspace: null,
  navigation: makeInitialNavigationState(),
  activeObjectId: null,
  activeViewId: null,
  activeLensId: null,
  spotterOpen: false,
  activeView: null,
  explorations: [],
  perspective: "graph",
};

/**
 * Pure reducer. Navigation actions are handled inline; non-navigation
 * actions are handled inline as well.
 */
export function appReducer(state: AppState, action: Action): AppState {
  // Handle navigation actions directly
  if (isNavigationAction(action)) {
    const navAction = toNavigationAction(action);
    const navigation = apply(state.navigation, navAction);
    const focus = getActiveFocus(navigation);
    return {
      ...state,
      navigation,
      activeObjectId: focus.objectId,
      activeViewId: focus.viewId,
      activeLensId: focus.lensId,
      activeView: focus.view,
    };
  }

  switch (action.type) {
    case "SET_WORKSPACE":
      return { ...state, workspace: action.payload };

    case "TOGGLE_SPOTTER":
      return { ...state, spotterOpen: !state.spotterOpen };

    case "SET_SPOTTER":
      return { ...state, spotterOpen: action.payload.open };

    case "ADD_EXPLORATION":
      return { ...state, explorations: [...state.explorations, action.payload] };

    case "RESET":
      return initialState;

    case "SET_PERSPECTIVE":
      return { ...state, perspective: action.payload };

    default: {
      const _exhaustive: never = action;
      void _exhaustive;
      return state;
    }
  }
}

// ============================================================================
// Action routing
// ============================================================================

/**
 * Type guard: does this Action belong to the navigation vocabulary?
 */
function isNavigationAction(action: Action): action is Extract<Action,
  | { type: "SELECT_OBJECT" }
  | { type: "SET_ACTIVE_VIEW" }
  | { type: "SET_ACTIVE_LENS" }
  | { type: "PUSH_PANE" }
  | { type: "CLOSE_PANE" }
  | { type: "ACTIVATE_PANE" }
  | { type: "REORDER_PANE" }
  | { type: "SET_PANE_SCROLL" }
> {
  return (
    action.type === "SELECT_OBJECT" ||
    action.type === "SET_ACTIVE_VIEW" ||
    action.type === "SET_ACTIVE_LENS" ||
    action.type === "PUSH_PANE" ||
    action.type === "CLOSE_PANE" ||
    action.type === "ACTIVATE_PANE" ||
    action.type === "REORDER_PANE" ||
    action.type === "SET_PANE_SCROLL"
  );
}

/**
 * Translate the public Action into the internal NavigationAction.
 */
function toNavigationAction(action: Extract<Action,
  | { type: "SELECT_OBJECT" }
  | { type: "SET_ACTIVE_VIEW" }
  | { type: "SET_ACTIVE_LENS" }
  | { type: "PUSH_PANE" }
  | { type: "CLOSE_PANE" }
  | { type: "ACTIVATE_PANE" }
  | { type: "REORDER_PANE" }
  | { type: "SET_PANE_SCROLL" }
>): NavigationAction {
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
  }
}

// ============================================================================
// Context + hooks
// ============================================================================

type AppContextValue = {
  state: AppState;
  dispatch: React.Dispatch<Action>;
};

export const AppContext = createContext<AppContextValue | null>(null);

export function useAppState(): AppState {
  const ctx = useContext(AppContext);
  if (!ctx) {
    throw new Error("useAppState must be used inside <AppProvider>");
  }
  return ctx.state;
}

export function useAppDispatch(): React.Dispatch<Action> {
  const ctx = useContext(AppContext);
  if (!ctx) {
    throw new Error("useAppDispatch must be used inside <AppProvider>");
  }
  return ctx.dispatch;
}

/**
 * Convenience hook that combines state + dispatch and is the
 * intended public surface for components.
 */
export function useApp(): AppContextValue {
  const ctx = useContext(AppContext);
  if (!ctx) {
    throw new Error("useApp must be used inside <AppProvider>");
  }
  return ctx;
}

/**
 * Helper for the App.tsx root — wires up a single useReducer and
 * memoises the context value so consumers do not re-render on every
 * dispatch.
 */
export function useAppReducer() {
  const [state, dispatch] = useReducer(appReducer, initialState);
  return { state, dispatch };
}
