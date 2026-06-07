/**
 * App-level state shape, the reducer, and a typed Context.
 *
 * Single source of truth for the Explorer UI. The 3-panel Shell
 * (Miller Columns / Object Inspector / Lens Panel) reads from
 * `useAppState()`; interactions dispatch through `useAppDispatch()`.
 *
 * Actions are deliberately narrow and serialisable. Persisting the
 * whole state to a future server-side path is a one-liner.
 */
import { createContext, useContext, useReducer } from "react";
import type {
  ContextualView,
  ExplorationColumn,
  ExplorationPath,
  WorkspaceSummary,
} from "../api/types";

// ============================================================================
// State
// ============================================================================

export type AppState = {
  workspace: WorkspaceSummary | null;
  columns: ExplorationColumn[];
  /**
   * Active object id (the leaf column the user is looking at).
   * `null` until the user picks something.
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

// ============================================================================
// Actions
// ============================================================================

export type Action =
  | { type: "SET_WORKSPACE"; payload: WorkspaceSummary }
  | { type: "PUSH_COLUMN"; payload: ExplorationColumn }
  | { type: "POP_COLUMN"; payload: { index: number } }
  | {
      type: "SELECT_OBJECT";
      payload: { objectId: string; viewId?: string; kind?: string };
    }
  | { type: "SET_ACTIVE_VIEW"; payload: ContextualView }
  | { type: "SET_ACTIVE_LENS"; payload: { lensId: string | null } }
  | { type: "TOGGLE_SPOTTER" }
  | { type: "SET_SPOTTER"; payload: { open: boolean } }
  | { type: "ADD_EXPLORATION"; payload: ExplorationPath }
  | { type: "RESET" };

// ============================================================================
// Reducer
// ============================================================================

export const initialState: AppState = {
  workspace: null,
  columns: [],
  activeObjectId: null,
  activeViewId: null,
  activeLensId: null,
  spotterOpen: false,
  activeView: null,
  explorations: [],
};

/**
 * Pure reducer. Every action keeps invariants local:
 * - `SELECT_OBJECT` collapses trailing columns and pushes a new one
 *   with the chosen view id.
 * - `POP_COLUMN` truncates columns after `index` and clears the
 *   active view/lens if the leaf is dropped.
 */
export function appReducer(state: AppState, action: Action): AppState {
  switch (action.type) {
    case "SET_WORKSPACE":
      return { ...state, workspace: action.payload };

    case "PUSH_COLUMN":
      return { ...state, columns: [...state.columns, action.payload] };

    case "POP_COLUMN": {
      const idx = action.payload.index;
      if (idx < 0 || idx >= state.columns.length) return state;
      const next = state.columns.slice(0, idx);
      const wasLeaf = idx === state.columns.length - 1;
      return {
        ...state,
        columns: next,
        ...(wasLeaf
          ? { activeObjectId: null, activeViewId: null, activeLensId: null, activeView: null }
          : {}),
      };
    }

    case "SELECT_OBJECT": {
      const { objectId, viewId, kind } = action.payload;
      const newColumn: ExplorationColumn = {
        object_id: objectId,
        active_view: viewId ?? null,
        kind: kind ?? "symbol",
      };
      // Replace the last column if the user is re-selecting on the
      // same object; otherwise append.
      const last = state.columns[state.columns.length - 1];
      if (last && last.object_id === objectId) {
        const cols = state.columns.slice(0, -1);
        return {
          ...state,
          columns: [...cols, newColumn],
          activeObjectId: objectId,
          activeViewId: viewId ?? state.activeViewId,
        };
      }
      return {
        ...state,
        columns: [...state.columns, newColumn],
        activeObjectId: objectId,
        activeViewId: viewId ?? null,
        activeLensId: null,
        activeView: null,
      };
    }

    case "SET_ACTIVE_VIEW":
      return {
        ...state,
        activeView: action.payload,
        activeViewId: action.payload.view_id,
        // Sync the column's active_view with the resolved one
        columns: state.columns.map((c, i) =>
          i === state.columns.length - 1
            ? { ...c, active_view: action.payload.view_id }
            : c,
        ),
      };

    case "SET_ACTIVE_LENS":
      return { ...state, activeLensId: action.payload.lensId };

    case "TOGGLE_SPOTTER":
      return { ...state, spotterOpen: !state.spotterOpen };

    case "SET_SPOTTER":
      return { ...state, spotterOpen: action.payload.open };

    case "ADD_EXPLORATION":
      return { ...state, explorations: [...state.explorations, action.payload] };

    case "RESET":
      return initialState;
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
