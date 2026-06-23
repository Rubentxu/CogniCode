/**
 * Navigation state types — pane-stack only (column mode removed, ADR-039 E3).
 */
import type { ContextualView } from "../../api/types";

// ============================================================================
// Shared types
// ============================================================================

/**
 * Viewport state for graph pan/zoom capture (ADR-040 Wave 3).
 */
export interface ViewportState {
  x: number;
  y: number;
  scale: number;
}

/**
 * Focus = what the Object Inspector is currently showing.
 */
export type Focus = {
  objectId: string | null;
  viewId: string | null;
  lensId: string | null;
  view: ContextualView | null;
};

/**
 * Navigation mode. Column mode has been removed.
 */
export type NavigationMode = "pane-stack";

/**
 * Per-pane state. Each open pane tracks its own object, view, lens,
 * and scroll position.
 *
 * The `kind` field is the object's type (symbol, file, scope, etc.).
 * `localFilters` is reserved for future per-pane state (scroll, search
 * filters). `scrollY` is the vertical scroll offset for restore.
 */
export type Pane = {
  id: string;
  objectId: string;
  activeViewId: string | null;
  activeLensId: string | null;
  kind: string;
  activeView: ContextualView | null;
  scrollY: number;
  viewport?: ViewportState;
  localFilters: Record<string, unknown>;
};

/**
 * Local chain entry — drill-down history for a single pane.
 * Replaces the removed `ExplorationColumn` type from the legacy
 * `ExplorationPath` model (ADR-045 Phase 1).
 */
export type ChainEntry = {
  object_id: string;
  active_view: string | null;
  kind: string;
};

/**
 * NavigationState — the slice of AppState that owns pane-stack navigation.
 *
 * - `panes` is the open pane list (gtoolkit-style side-by-side).
 * - `activePaneId` selects which pane the inspector renders.
 */
export type NavigationState = {
  /** Open panes — gtoolkit-style side-by-side. */
  panes: Pane[];
  /** Id of the focused pane (where the inspector renders). */
  activePaneId: string | null;
};

// ============================================================================
// Action vocabulary
// ============================================================================

/**
 * ActionType — the public vocabulary for pane-stack navigation.
 */
export type NavigationAction =
  | { type: "SELECT_OBJECT"; payload: { objectId: string; viewId?: string; kind?: string } }
  | { type: "SET_ACTIVE_VIEW"; payload: ContextualView }
  | { type: "SET_ACTIVE_LENS"; payload: { lensId: string | null } }
  | { type: "PUSH_PANE"; payload: { objectId: string; viewId?: string; kind?: string } }
  | { type: "CLOSE_PANE"; payload: { paneId: string } }
  | { type: "ACTIVATE_PANE"; payload: { paneId: string } }
  | { type: "REORDER_PANE"; payload: { fromIndex: number; toIndex: number } }
  | { type: "SET_PANE_SCROLL"; payload: { paneId: string; scrollY: number } }
  | { type: "UPDATE_PANE_VIEWPORT"; payload: { paneId: string; viewport: ViewportState } }
  | { type: "RESET" };

// ============================================================================
// Factory + helpers
// ============================================================================

/**
 * Initial state factory.
 */
export function makeInitialNavigationState(): NavigationState {
  return {
    panes: [],
    activePaneId: null,
  };
}
