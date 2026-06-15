/**
 * Navigation factory + persistence.
 *
 * The reducer in `context.ts` asks this module for the adapter that
 * matches the current `NavigationMode`. The mode is read from
 * localStorage on init and persisted on change.
 */
import { ColumnNavigation } from "./column";
import { PaneStackNavigation } from "./paneStack";
import type { NavigationAdapter, NavigationMode } from "./types";

/**
 * localStorage key. Versioned so future schema changes can detect
 * old values and reset to default.
 */
const STORAGE_KEY = "cognicode.navigationMode.v1";

/**
 * Default mode on first load. "column" matches the original behaviour
 * and is the right default for vertical slice tracing.
 */
export const DEFAULT_NAVIGATION_MODE: NavigationMode = "column";

/**
 * Adapter registry. Adding a new mode = adding a new entry here.
 * Compile-time check: every mode must have an adapter.
 */
const ADAPTERS: Record<NavigationMode, NavigationAdapter> = {
  column: ColumnNavigation,
  "pane-stack": PaneStackNavigation,
};

export function getAdapter(mode: NavigationMode): NavigationAdapter {
  return ADAPTERS[mode];
}

export function isNavigationMode(value: unknown): value is NavigationMode {
  return value === "column" || value === "pane-stack";
}

/**
 * Read the user's preferred mode from localStorage. Safe to call
 * during SSR (returns default if `window` is undefined).
 */
export function readNavigationMode(): NavigationMode {
  if (typeof window === "undefined") return DEFAULT_NAVIGATION_MODE;
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (isNavigationMode(raw)) return raw;
  } catch {
    // localStorage can throw in private-browsing mode or when the
    // storage quota is exhausted. Fall back to default.
  }
  return DEFAULT_NAVIGATION_MODE;
}

/**
 * Persist the user's mode preference. Failures are non-fatal — the
 * app keeps working with the in-memory mode even if storage throws.
 */
export function writeNavigationMode(mode: NavigationMode): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    // ignore — see readNavigationMode for context
  }
}

export type { NavigationAdapter, NavigationAction, NavigationMode, NavigationState, Pane, Focus } from "./types";
export { makeInitialNavigationState } from "./types";
export { MAX_PANES } from "./paneStack";
export { useNavigationMode } from "./useNavigationMode";
