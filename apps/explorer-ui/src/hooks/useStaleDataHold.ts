/**
 * `useStaleDataHold` — keep the last good value across revalidation.
 *
 * While `isLoading`, returns the held value (or the current value if no
 * hold exists); when loading completes with new data, the hold updates.
 *
 * Used by InteractiveGraphPanel to prevent unmounts during perspective
 * toggles with warm cache (E5.5 stale-data hold pattern).
 *
 * Implementation uses the React-recommended "set state during render"
 * pattern (https://react.dev/reference/react/useState#storing-information-from-previous-renders)
 * to sync `held` with `value` without an effect.
 */
import { useState } from "react";

export function useStaleDataHold<T>(
  value: T,
  isLoading: boolean,
  hasError: boolean,
): T {
  const [held, setHeld] = useState<T | null>(null);

  // Set state during render: when a new good value arrives (not loading,
  // not error), sync `held` to it. React handles this efficiently — it
  // discards the in-progress render and re-renders with the new state.
  if (!isLoading && !hasError && value !== held) {
    setHeld(value);
  }

  if (isLoading && held !== null) return held;
  return value;
}
