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
  WorkspaceSummary,
} from "../api/types";
import {
  apply,
  getActiveFocus,
  makeInitialNavigationState,
  type NavigationAction,
  type NavigationState,
  type ViewportState,
} from "./slices/navigation";
import {
  rootReducer,
  type LensSidebarAction,
  type LensSidebarState,
} from "./slices";

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
  /**
   * Active canvas perspective — graph (symbol neighbourhood via useSubgraph)
   * or c4 (workspace-wide components via useArchitecture).
   * Perspective morphs the canvas regardless of object selection (ADR-039 §3/§4).
   */
  perspective: "graph" | "c4";
  /**
   * LensPanel sidebar visibility.
   */
  lensSidebar: LensSidebarState;
};

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
    perspective: "graph",
    lensSidebar: { open: false },
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
  | { type: "UPDATE_PANE_VIEWPORT"; payload: { paneId: string; viewport: ViewportState } }
  | { type: "TOGGLE_SPOTTER" }
  | { type: "SET_SPOTTER"; payload: { open: boolean } }
  | { type: "RESET" }
  | { type: "SET_PERSPECTIVE"; payload: "graph" | "c4" }
  | { type: "TOGGLE_LENS_SIDEBAR" }
  | { type: "SET_LENS_SIDEBAR"; payload: { open: boolean } };

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
  perspective: "graph",
  lensSidebar: { open: false },
};

/**
 * Pure reducer — delegates to domain slices via rootReducer.
 */
export function appReducer(state: AppState, action: Action): AppState {
  return rootReducer(state, action);
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
