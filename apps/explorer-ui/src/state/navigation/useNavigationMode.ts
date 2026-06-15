/**
 * useNavigationMode — read/write the user's preferred navigation mode.
 *
 * Persists to localStorage. SSR-safe.
 */
import { useCallback, useState, useSyncExternalStore } from "react";
import {
  DEFAULT_NAVIGATION_MODE,
  readNavigationMode,
  writeNavigationMode,
  type NavigationMode,
} from "./index";

/**
 * useNavigationMode — read/write the user's preferred navigation mode.
 *
 * Persists to localStorage. SSR-safe.
 *
 * We use `useSyncExternalStore` (React 18+) to subscribe to
 * localStorage rather than `useEffect + setState`. This avoids
 * the cascading-render warning and gives us tear-free reads
 * during concurrent rendering.
 */
function subscribe(): () => void {
  // localStorage doesn't notify subscribers; the mode only changes
  // when we write it ourselves (via the setter below). Returning
  // a no-op unsubscribe is correct here.
  return () => {};
}

function getSnapshot(): NavigationMode {
  return readNavigationMode();
}

function getServerSnapshot(): NavigationMode {
  return DEFAULT_NAVIGATION_MODE;
}

export function useNavigationMode(
  dispatch?: (action: { type: "SET_NAVIGATION_MODE"; payload: { mode: NavigationMode } }) => void,
) {
  // Initialise from localStorage synchronously when on the client.
  // The setter below keeps the snapshot in sync.
  const [override, setOverride] = useState<NavigationMode | null>(null);
  const stored = useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
  const mode = override ?? stored;

  const setMode = useCallback(
    (next: NavigationMode) => {
      setOverride(next);
      writeNavigationMode(next);
      dispatch?.({ type: "SET_NAVIGATION_MODE", payload: { mode: next } });
    },
    [dispatch],
  );

  return [mode, setMode] as const;
}
