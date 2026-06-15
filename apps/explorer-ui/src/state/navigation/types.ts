/**
 * NavigationAdapter — polymorphic interface for navigation modes.
 *
 * The Explorer supports two navigation modes (configurable per user):
 *
 * - `column`  — vertical drill-down. Selecting an object replaces the
 *               leaf column and the Object Inspector always shows the
 *               last object. Modeled on Finder/Miller columns.
 *
 * - `pane-stack` — lateral side-by-side panels. Selecting an object
 *                  opens a NEW pane to the right. Multiple objects can
 *                  be inspected in parallel. Modeled on gtoolkit's
 *                  GtPager.
 *
 * The adapter abstracts the difference so the Shell, Spotter, and
 * other consumers dispatch a single `Action` and don't care which
 * mode is active. The mode is chosen via the user's settings (see
 * `useNavigationMode`) and persisted to localStorage.
 *
 * See ADR-016 (jun-15) for the full rationale.
 */
import type { ContextualView, ExplorationColumn } from "../../api/types";

// ============================================================================
// Shared types
// ============================================================================

/**
 * Focus = what the Object Inspector is currently showing.
 * Both modes expose this so consumers (e.g. Shell, InteractiveGraph)
 * can read the active object without knowing the mode.
 */
export type Focus = {
  objectId: string | null;
  viewId: string | null;
  lensId: string | null;
  view: ContextualView | null;
};

/**
 * Navigation mode. New modes can be added by extending this enum and
 * providing a new `NavigationAdapter` implementation.
 */
export type NavigationMode = "column" | "pane-stack";

/**
 * Per-pane state. Pane-stack uses one of these per open pane; column
 * mode uses exactly one (the "active leaf" of the column chain).
 *
 * The `kind` field is the object's type (symbol, file, scope, etc.)
 * — needed by MillerColumns to map parent-kind → child view id.
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
  localFilters: Record<string, unknown>;
};

/**
 * NavigationState — the slice of AppState that the adapter owns.
 * Each adapter implementation may interpret this shape differently:
 *
 * - ColumnNavigation: `chain` is the linear column list;
 *   `panes` is always `[activePane]` (the leaf, with `kind: chain[last].kind`).
 *
 * - PaneStackNavigation: `panes` is the open pane list;
 *   `chain` is always `[activePane.objectId]` (the focused pane's object,
 *   or `[]` if no pane is focused).
 *
 * The Shell reads `focus()` (from `getActiveFocus`) and `panes` (for
 * pane-stack rendering). It does NOT read `chain` directly.
 */
export type NavigationState = {
  mode: NavigationMode;
  /** Linear path of objects — drill-down history. */
  chain: ExplorationColumn[];
  /** Open panes — gtoolkit-style side-by-side. */
  panes: Pane[];
  /** Id of the focused pane (where the inspector renders). */
  activePaneId: string | null;
};

// ============================================================================
// Action vocabulary
// ============================================================================

/**
 * ActionType — the public vocabulary an adapter must support.
 *
 * Not all actions are meaningful in all modes (e.g. CLOSE_PANE in
 * column mode is a no-op). Adapters are free to interpret or ignore
 * an action — the typed return contract is the responsibility of the
 * adapter, not the dispatcher.
 */
export type NavigationAction =
  | { type: "PUSH_COLUMN"; payload: ExplorationColumn }
  | { type: "POP_COLUMN"; payload: { index: number } }
  | { type: "SELECT_OBJECT"; payload: { objectId: string; viewId?: string; kind?: string } }
  | { type: "SET_ACTIVE_VIEW"; payload: ContextualView }
  | { type: "SET_ACTIVE_LENS"; payload: { lensId: string | null } }
  | { type: "PUSH_PANE"; payload: { objectId: string; viewId?: string; kind?: string } }
  | { type: "CLOSE_PANE"; payload: { paneId: string } }
  | { type: "ACTIVATE_PANE"; payload: { paneId: string } }
  | { type: "REORDER_PANE"; payload: { fromIndex: number; toIndex: number } }
  | { type: "SET_PANE_SCROLL"; payload: { paneId: string; scrollY: number } }
  | { type: "RESET" };

// ============================================================================
// Adapter contract
// ============================================================================

/**
 * NavigationAdapter — the polymorphic contract.
 *
 * Each mode provides a pure function `apply(action) -> NavigationState`.
 * The reducer in `context.ts` calls `adapter.apply(state, action)`
 * and merges the result back into AppState.
 *
 * Adapters are PURE — no I/O, no random IDs from the global counter.
 * The reducer injects ids before calling (see `newPaneId` in
 * `context.ts`). This keeps adapters trivially testable.
 */
export interface NavigationAdapter {
  readonly mode: NavigationMode;
  /**
   * Apply an action and return the new navigation state.
   * Implementations MUST be pure functions of (state, action).
   */
  apply(state: NavigationState, action: NavigationAction): NavigationState;
  /**
   * Compute the current focus — what the Object Inspector should show.
   * In column mode this is the leaf column. In pane-stack mode this
   * is the activePane.
   */
  getActiveFocus(state: NavigationState): Focus;
  /**
   * Whether the inspector should render at all (i.e. there is a
   * focused object). Used by Shell to decide layout.
   */
  hasFocus(state: NavigationState): boolean;
}

// ============================================================================
// Factory + helpers
// ============================================================================

/**
 * Initial state factory. Each mode provides its own default; this
 * helper is the single point where new fields are added.
 */
export function makeInitialNavigationState(mode: NavigationMode): NavigationState {
  return {
    mode,
    chain: [],
    panes: [],
    activePaneId: null,
  };
}
