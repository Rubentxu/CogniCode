/**
 * ColumnNavigation — the default navigation adapter.
 *
 * Vertical drill-down, modeled on Finder/Miller columns. The Shell
 * renders a single Object Inspector; selecting an object either
 * replaces the leaf column (same object re-selected) or appends a
 * new column.
 *
 * Invariants:
 * - `chain` is the linear history of drill-down.
 * - `panes` is always exactly one element: the active leaf (or `[]`
 *   if nothing is selected). This makes `getActiveFocus` symmetric
 *   with `pane-stack` mode — the Shell can always read `state.panes`
 *   without branching on mode.
 * - `activePaneId` is the leaf's id, or `null` if no leaf.
 */
import type { NavigationAdapter, NavigationState, Pane } from "./types";
import { makeInitialNavigationState } from "./types";
import type { ExplorationColumn } from "../../api/types";

/**
 * Build the single-pane representation of the leaf column. We mirror
 * the column in `panes` so consumers can iterate `panes` regardless
 * of mode.
 */
function leafPane(chain: ExplorationColumn[]): Pane | null {
  const last = chain[chain.length - 1];
  if (!last) return null;
  return {
    id: `column-leaf-${last.object_id}`,
    objectId: last.object_id,
    activeViewId: last.active_view,
    activeLensId: null,
    kind: last.kind ?? "symbol",
    activeView: null,
    scrollY: 0,
    localFilters: {},
  };
}

export const ColumnNavigation: NavigationAdapter = {
  mode: "column",

  apply(state, action): NavigationState {
    switch (action.type) {
      case "PUSH_COLUMN": {
        const chain = [...state.chain, action.payload];
        return { ...state, chain, panes: leafPane(chain) ? [leafPane(chain)!] : [] };
      }

      case "POP_COLUMN": {
        const idx = action.payload.index;
        if (idx < 0 || idx >= state.chain.length) return state;
        const next = state.chain.slice(0, idx);
        return { ...state, chain: next, panes: leafPane(next) ? [leafPane(next)!] : [] };
      }

      case "SELECT_OBJECT": {
        const { objectId, viewId, kind } = action.payload;
        const newColumn: ExplorationColumn = {
          object_id: objectId,
          active_view: viewId ?? null,
          kind: kind ?? "symbol",
        };
        const last = state.chain[state.chain.length - 1];
        // Re-selecting the same object replaces the leaf (preserves
        // navigation history but updates the view). Different object
        // appends a new column.
        const next = last && last.object_id === objectId
          ? [...state.chain.slice(0, -1), newColumn]
          : [...state.chain, newColumn];
        return { ...state, chain: next, panes: leafPane(next) ? [leafPane(next)!] : [] };
      }

      case "SET_ACTIVE_VIEW": {
        const last = state.chain[state.chain.length - 1];
        if (!last) return state;
        const updated: ExplorationColumn = {
          ...last,
          active_view: action.payload.view_id,
        };
        const next = [...state.chain.slice(0, -1), updated];
        const pane = leafPane(next);
        return {
          ...state,
          chain: next,
          panes: pane ? [{ ...pane, activeViewId: action.payload.view_id, activeView: action.payload }] : [],
        };
      }

      case "SET_ACTIVE_LENS": {
        const pane = state.panes[0];
        if (!pane) return state;
        return { ...state, panes: [{ ...pane, activeLensId: action.payload.lensId }] };
      }

      // Pane-stack actions are no-ops in column mode.
      case "PUSH_PANE":
      case "CLOSE_PANE":
      case "ACTIVATE_PANE":
      case "REORDER_PANE":
      case "SET_PANE_SCROLL":
        return state;

      case "RESET":
        return makeInitialNavigationState("column");

      default: {
        // Exhaustiveness check — TypeScript will fail to compile if a
        // new NavigationAction variant is added without a handler.
        const _exhaustive: never = action;
        void _exhaustive;
        return state;
      }
    }
  },

  getActiveFocus(state) {
    const pane = state.panes[0] ?? null;
    return {
      objectId: pane?.objectId ?? null,
      viewId: pane?.activeViewId ?? null,
      lensId: pane?.activeLensId ?? null,
      view: pane?.activeView ?? null,
    };
  },

  hasFocus(state) {
    const first = state.panes[0];
    return state.panes.length > 0 && first !== undefined && first.objectId !== null;
  },
};
