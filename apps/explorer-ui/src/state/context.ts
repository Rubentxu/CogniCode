/**
 * App-level state shape, the reducer, and a typed Context.
 *
 * Single source of truth for the Explorer UI. The 3-panel Shell
 * (Miller Columns / Object Inspector / Lens Panel) reads from
 * `useAppState()`; interactions dispatch through `useAppDispatch()`.
 *
 * Actions are deliberately narrow and serialisable. Persisting the
 * whole state to a future server-side path is a one-liner.
 *
 * Navigation is delegated to a `NavigationAdapter` (see
 * `./navigation/`). The reducer reads the current mode from state,
 * asks the factory for the matching adapter, and applies the action.
 * Adding a new navigation mode = adding a new adapter, no changes
 * here.
 */
import { createContext, useContext, useReducer } from "react";
import type {
  ContextualView,
  ExplorationPath,
  WorkspaceSummary,
} from "../api/types";
import {
  getAdapter,
  makeInitialNavigationState,
  type NavigationAction,
  type NavigationMode,
  type NavigationState,
} from "./navigation";

// ============================================================================
// State
// ============================================================================

export type AppState = {
  workspace: WorkspaceSummary | null;
  /**
   * Navigation state — owned by the active `NavigationAdapter`. The
   * reducer in this file never inspects the internal shape; it only
   * forwards actions to the adapter and exposes the focus.
   */
  navigation: NavigationState;
  /**
   * Active object id (the leaf column / focused pane). Cached at the
   * top level for consumers that don't want to dig into `navigation`.
   * The reducer keeps this in sync with `navigationAdapter.getActiveFocus()`.
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
};

/**
 * Selector for the column chain. Returns `state.navigation.chain`.
 *
 * Use this instead of reading `state.columns` (which is now gone).
 * The chain is the linear drill-down history; in `column` mode it
 * equals the column list, in `pane-stack` mode it mirrors the
 * focused pane's object.
 */
export function selectChain(state: AppState): NavigationState["chain"] {
  return state.navigation.chain;
}

/**
 * Build an initial AppState focused on a given object. Used by
 * tests that want to skip the navigation setup and jump straight
 * to "object is selected, inspector is showing it".
 *
 * In column mode, this materialises a single-pane navigation with
 * the object as the leaf. In pane-stack mode, it opens a single
 * pane with the object. The reducer will keep `activeObjectId`
 * etc. in sync on subsequent dispatches.
 */
export function initialStateWithFocus(
  activeObjectId: string,
  mode: NavigationMode = "column",
  viewId: string | null = null,
  kind: string = "symbol",
): AppState {
  const adapter = getAdapter(mode);
  const base = makeInitialNavigationState(mode);
  // The action type depends on the mode. Both adapters translate
  // the public Action into their own shape, but the adapter's
  // `apply` takes NavigationAction — so we use the mode-appropriate
  // primitive (PUSH_COLUMN for column, PUSH_PANE for pane-stack).
  const navAction: Extract<NavigationAction, { type: "PUSH_COLUMN" | "PUSH_PANE" }> =
    mode === "pane-stack"
      ? { type: "PUSH_PANE", payload: { objectId: activeObjectId, viewId: viewId ?? undefined, kind } }
      : { type: "PUSH_COLUMN", payload: { object_id: activeObjectId, active_view: viewId, kind } };
  const nav = adapter.apply(base, navAction);
  const focus = adapter.getActiveFocus(nav);
  return {
    workspace: null,
    navigation: nav,
    activeObjectId: focus.objectId,
    activeViewId: focus.viewId,
    activeLensId: focus.lensId,
    spotterOpen: false,
    activeView: null,
    explorations: [],
  };
}

// ============================================================================
// Actions
// ============================================================================

/**
 * Public action vocabulary. The reducer converts the high-level
 * "PUSH_COLUMN" / "SELECT_OBJECT" etc. into the adapter's action
 * shape. New actions that don't affect navigation live alongside.
 */
export type Action =
  | { type: "SET_WORKSPACE"; payload: WorkspaceSummary }
  | { type: "PUSH_COLUMN"; payload: { object_id: string; active_view: string | null; kind?: string } }
  | { type: "POP_COLUMN"; payload: { index: number } }
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
  | { type: "TOGGLE_SPOTTER" }
  | { type: "SET_SPOTTER"; payload: { open: boolean } }
  | { type: "ADD_EXPLORATION"; payload: ExplorationPath }
  | { type: "SET_NAVIGATION_MODE"; payload: { mode: NavigationMode } }
  | { type: "RESET" };

// ============================================================================
// Reducer
// ============================================================================

/**
 * Initial state. Default navigation mode is "column" (the legacy
 * behaviour). The mode is overridden by `useAppReducerWithMode` when
 * the user's preference is read from localStorage on mount.
 */
export const initialState: AppState = {
  workspace: null,
  navigation: makeInitialNavigationState("column"),
  activeObjectId: null,
  activeViewId: null,
  activeLensId: null,
  spotterOpen: false,
  activeView: null,
  explorations: [],
};

/**
 * Pure reducer. Navigation actions are delegated to the active
 * adapter; non-navigation actions are handled inline.
 */
export function appReducer(state: AppState, action: Action): AppState {
  // SET_NAVIGATION_MODE is special — it can change the mode without
  // going through the adapter. The adapter of the new mode is then
  // asked to RESET (no payload) so the new mode starts clean.
  if (action.type === "SET_NAVIGATION_MODE") {
    if (state.navigation.mode === action.payload.mode) return state;
    const adapter = getAdapter(action.payload.mode);
    const freshNav: NavigationState = adapter.apply(
      makeInitialNavigationState(action.payload.mode),
      { type: "RESET" },
    );
    return {
      ...state,
      navigation: freshNav,
      activeObjectId: null,
      activeViewId: null,
      activeLensId: null,
      activeView: null,
    };
  }

  // Translate the public action into a NavigationAction and ask the
  // adapter to apply it. Non-navigation actions skip the adapter.
  if (isNavigationAction(action)) {
    const adapter = getAdapter(state.navigation.mode);
    const navAction = toNavigationAction(action);
    const navigation = adapter.apply(state.navigation, navAction);
    const focus = adapter.getActiveFocus(navigation);
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
 * Used by the reducer to decide whether to delegate to the adapter.
 */
function isNavigationAction(action: Action): action is Extract<Action,
  | { type: "PUSH_COLUMN" }
  | { type: "POP_COLUMN" }
  | { type: "SELECT_OBJECT" }
  | { type: "SET_ACTIVE_VIEW" }
  | { type: "SET_ACTIVE_LENS" }
  | { type: "PUSH_PANE" }
  | { type: "CLOSE_PANE" }
  | { type: "ACTIVATE_PANE" }
  | { type: "REORDER_PANE" }
> {
  return (
    action.type === "PUSH_COLUMN" ||
    action.type === "POP_COLUMN" ||
    action.type === "SELECT_OBJECT" ||
    action.type === "SET_ACTIVE_VIEW" ||
    action.type === "SET_ACTIVE_LENS" ||
    action.type === "PUSH_PANE" ||
    action.type === "CLOSE_PANE" ||
    action.type === "ACTIVATE_PANE" ||
    action.type === "REORDER_PANE"
  );
}

/**
 * Translate the public Action into the adapter's NavigationAction.
 * Public actions use slightly different field names (`objectId` vs
 * `object_id`, `viewId` vs `view_id`); the adapter uses one canonical
 * shape.
 */
function toNavigationAction(action: Extract<Action,
  | { type: "PUSH_COLUMN" }
  | { type: "POP_COLUMN" }
  | { type: "SELECT_OBJECT" }
  | { type: "SET_ACTIVE_VIEW" }
  | { type: "SET_ACTIVE_LENS" }
  | { type: "PUSH_PANE" }
  | { type: "CLOSE_PANE" }
  | { type: "ACTIVATE_PANE" }
  | { type: "REORDER_PANE" }
>): NavigationAction {
  switch (action.type) {
    case "PUSH_COLUMN":
      return {
        type: "PUSH_COLUMN",
        payload: {
          object_id: action.payload.object_id,
          active_view: action.payload.active_view,
          kind: action.payload.kind ?? "symbol",
        },
      };
    case "POP_COLUMN":
      return { type: "POP_COLUMN", payload: action.payload };
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
