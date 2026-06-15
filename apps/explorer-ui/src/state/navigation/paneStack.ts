/**
 * PaneStackNavigation — gtoolkit-style side-by-side panes.
 *
 * Skeleton implementation (ADR-016 Fase 1). Each `SELECT_OBJECT`
 * opens a NEW pane; the user can have multiple panes open in
 * parallel. The active pane is what the Object Inspector shows.
 *
 * Invariants:
 * - `panes` is a list of open panes (capped at MAX_PANES, see
 *   `MAX_PANES`). Pane id is supplied by the reducer (not generated
 *   here, to keep the adapter pure).
 * - `activePaneId` selects which pane the inspector renders.
 * - `chain` mirrors the active pane's history for drill-down
 *   (used by Miller Columns to show the path to the focused object).
 * - Closing the active pane moves focus to the previous one
 *   (or null if none).
 *
 * Fase 2 of ADR-016 wires this to the Shell (rendering) and the
 * viewport-aware degradation. This file is the state model only.
 */
import type { NavigationAdapter, NavigationState, Pane } from "./types";
import { makeInitialNavigationState } from "./types";
import type { ExplorationColumn } from "../../api/types";

/**
 * Cap to bound memory + DOM cost. gtoolkit's GtPager has no hard
 * cap; we impose one to avoid pathological cases (a 200-pane stack
 * would freeze the renderer). 8 is enough for most comparisons.
 */
export const MAX_PANES = 8;

/**
 * Mirror the active pane's drill-down history as a `chain` so
 * MillerColumns can render the path.
 */
function chainFromActivePane(state: NavigationState): ExplorationColumn[] {
  const pane = state.panes.find((p) => p.id === state.activePaneId);
  if (!pane) return [];
  return [
    {
      object_id: pane.objectId,
      active_view: pane.activeViewId,
      kind: pane.kind,
    },
  ];
}

export const PaneStackNavigation: NavigationAdapter = {
  mode: "pane-stack",

  apply(state, action): NavigationState {
    switch (action.type) {
      case "PUSH_PANE": {
        // Id is supplied by the reducer; if missing, generate a
        // deterministic one (caller error in production).
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
        return { ...state, panes, activePaneId: pane.id, chain: chainFromActivePane({ ...state, panes, activePaneId: pane.id }) };
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
        return { ...state, panes, activePaneId, chain: chainFromActivePane({ ...state, panes, activePaneId }) };
      }

      case "ACTIVATE_PANE": {
        if (!state.panes.some((p) => p.id === action.payload.paneId)) return state;
        return { ...state, activePaneId: action.payload.paneId, chain: chainFromActivePane({ ...state, activePaneId: action.payload.paneId }) };
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

      case "SELECT_OBJECT": {
        // In pane-stack mode, SELECT_OBJECT opens a new pane (or
        // focuses an existing one with the same objectId). Falling
        // through to PUSH_PANE behaviour.
        const { objectId, viewId, kind } = action.payload;
        const existing = state.panes.find((p) => p.objectId === objectId);
        if (existing) {
          return PaneStackNavigation.apply(
            { ...state, panes: state.panes.map((p) => p.id === existing.id ? { ...p, activeViewId: viewId ?? p.activeViewId } : p) },
            { type: "ACTIVATE_PANE", payload: { paneId: existing.id } },
          );
        }
        return PaneStackNavigation.apply(state, {
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
        return { ...state, panes, chain: chainFromActivePane({ ...state, panes }) };
      }

      case "SET_ACTIVE_LENS": {
        const pane = state.panes.find((p) => p.id === state.activePaneId);
        if (!pane) return state;
        const panes = state.panes.map((p) =>
          p.id === pane.id ? { ...p, activeLensId: action.payload.lensId } : p,
        );
        return { ...state, panes };
      }

      // Column-only actions are no-ops in pane-stack mode.
      case "PUSH_COLUMN":
      case "POP_COLUMN":
        return state;

      case "RESET":
        return makeInitialNavigationState("pane-stack");

      default: {
        const _exhaustive: never = action;
        void _exhaustive;
        return state;
      }
    }
  },

  getActiveFocus(state) {
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
  },

  hasFocus(state) {
    return state.panes.length > 0 && state.activePaneId !== null;
  },
};
