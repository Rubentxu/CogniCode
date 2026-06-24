/**
 * PaneStackNavigation — gtoolkit-style side-by-side panes.
 *
 * Each `SELECT_OBJECT` opens a NEW pane; the user can have multiple
 * panes open in parallel. The active pane is what the Object Inspector
 * shows.
 *
 * Invariants:
 * - `panes` is a list of open panes (capped at MAX_PANES, see
 *   `MAX_PANES`). Pane id is supplied by the reducer (not generated
 *   here, to keep the adapter pure).
 * - `activePaneId` selects which pane the inspector renders.
 * - Closing the active pane moves focus to the previous one
 *   (or null if none).
 */
import type { Focus, NavigationAction, NavigationState, Pane } from "./types";
import { makeInitialNavigationState } from "./types";

/**
 * Cap to bound memory + DOM cost. gtoolkit's GtPager has no hard
 * cap; we impose one to avoid pathological cases (a 200-pane stack
 * would freeze the renderer). 12 supports richer side-by-side
 * comparisons (e.g., call-graph + dependency-graph + seam-map +
 * source + quality + 7 drill-downs). Bumped from 8 to 12 in v0.8.2
 * (H8 audit fix).
 */
export const MAX_PANES = 12;

/**
 * Apply a navigation action and return the new state.
 */
export function apply(state: NavigationState, action: NavigationAction): NavigationState {
  switch (action.type) {
    case "PUSH_PANE": {
      const { objectId, viewId, kind } = action.payload;
      const pane: Pane = {
        id: `pane-${Date.now()}-${state.panes.length}`,
        objectId,
        activeViewId: viewId ?? null,
        activeLensId: null,
        kind: kind ?? "symbol",
        activeView: null,
        scrollY: 0,
        localFilters: {},
      };
      // Drop oldest if at cap (FIFO).
      const panes = state.panes.length >= MAX_PANES
        ? [...state.panes.slice(1), pane]
        : [...state.panes, pane];
      return { ...state, panes, activePaneId: pane.id };
    }

    case "CLOSE_PANE": {
      const idx = state.panes.findIndex((p) => p.id === action.payload.paneId);
      if (idx < 0) return state;
      const panes = state.panes.filter((p) => p.id !== action.payload.paneId);
      // If we closed the active pane, move focus to neighbour.
      let activePaneId = state.activePaneId;
      if (state.activePaneId === action.payload.paneId) {
        if (panes.length === 0) {
          activePaneId = null;
        } else {
          const newIdx = Math.min(idx, panes.length - 1);
          const neighbour = panes[newIdx];
          activePaneId = neighbour ? neighbour.id : null;
        }
      }
      return { ...state, panes, activePaneId };
    }

    case "ACTIVATE_PANE": {
      if (!state.panes.some((p) => p.id === action.payload.paneId)) return state;
      return { ...state, activePaneId: action.payload.paneId };
    }

    case "REORDER_PANE": {
      const { fromIndex, toIndex } = action.payload;
      if (
        fromIndex < 0 || fromIndex >= state.panes.length ||
        toIndex < 0 || toIndex >= state.panes.length ||
        fromIndex === toIndex
      ) {
        return state;
      }
      const next = state.panes.slice();
      const moved = next.splice(fromIndex, 1)[0];
      if (moved) {
        next.splice(toIndex, 0, moved);
      }
      return { ...state, panes: next };
    }

    case "SET_PANE_SCROLL": {
      const idx = state.panes.findIndex((p) => p.id === action.payload.paneId);
      const target = idx >= 0 ? state.panes[idx] : undefined;
      if (!target) return state;
      const panes = state.panes.slice();
      panes[idx] = { ...target, scrollY: action.payload.scrollY };
      return { ...state, panes };
    }

    case "UPDATE_PANE_VIEWPORT": {
      const { paneId, viewport } = action.payload;
      return {
        ...state,
        panes: state.panes.map((pane) =>
          pane.id === paneId ? { ...pane, viewport } : pane
        ),
      };
    }

    case "SELECT_OBJECT": {
      const { objectId, viewId, kind } = action.payload;
      const existing = state.panes.find((p) => p.objectId === objectId);
      if (existing) {
        return apply(
          { ...state, panes: state.panes.map((p) => p.id === existing.id ? { ...p, activeViewId: viewId ?? p.activeViewId } : p) },
          { type: "ACTIVATE_PANE", payload: { paneId: existing.id } },
        );
      }
      return apply(state, {
        type: "PUSH_PANE",
        payload: { objectId, viewId, kind },
      });
    }

    case "SET_ACTIVE_VIEW": {
      const pane = state.panes.find((p) => p.id === state.activePaneId);
      if (!pane) return state;
      const panes = state.panes.map((p) =>
        p.id === pane.id
          ? { ...p, activeViewId: action.payload.view_id, activeView: action.payload }
          : p,
      );
      return { ...state, panes };
    }

    case "SET_ACTIVE_LENS": {
      const pane = state.panes.find((p) => p.id === state.activePaneId);
      if (!pane) return state;
      const panes = state.panes.map((p) =>
        p.id === pane.id ? { ...p, activeLensId: action.payload.lensId } : p,
      );
      return { ...state, panes };
    }

    case "RESET":
      return makeInitialNavigationState();

    default: {
      const _exhaustive: never = action;
      void _exhaustive;
      return state;
    }
  }
}

/**
 * Compute the current focus — what the Object Inspector should show.
 */
export function getActiveFocus(state: NavigationState): Focus {
  const pane = state.panes.find((p) => p.id === state.activePaneId);
  if (!pane) {
    return { objectId: null, viewId: null, lensId: null, view: null };
  }
  return {
    objectId: pane.objectId,
    viewId: pane.activeViewId,
    lensId: pane.activeLensId,
    view: pane.activeView,
  };
}

/**
 * Whether the inspector should render at all (i.e. there is a
 * focused object).
 */
export function hasFocus(state: NavigationState): boolean {
  return state.panes.length > 0 && state.activePaneId !== null;
}
