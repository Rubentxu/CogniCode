/**
 * `useHealth` — polls `GET /api/health` and tracks the live status.
 *
 * The health endpoint is intentionally separate from the rest of the
 * SWR cache: it is a tiny response and we want it to drive the
 * connection indicator in the top bar without invalidating the
 * workspace cache on every poll.
 *
 * Returns `{ data, error, isOnline, refresh }`. `isOnline` is true
 * only after the first successful response — so the first paint
 * shows "checking…" rather than a false red dot during startup.
 */
import { useCallback, useEffect, useRef, useState } from "react";

import { ApiError, getApiBaseUrl } from "../api/client";
import { healthResponseSchema } from "../api/schemas";
import type { HealthResponse } from "../api/types";

export type HealthStatus = "unknown" | "online" | "offline";

export type UseHealthArgs = {
  /** Poll interval in milliseconds. Default 30s. */
  intervalMs?: number;
  /** Disable polling (and skip the first fetch). */
  enabled?: boolean;
};

export type UseHealthResult = {
  data: HealthResponse | null;
  error: Error | null;
  status: HealthStatus;
  isOnline: boolean;
  refresh: () => Promise<void>;
};

export function useHealth({
  intervalMs = 30_000,
  enabled = true,
}: UseHealthArgs = {}): UseHealthResult {
  const [data, setData] = useState<HealthResponse | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [status, setStatus] = useState<HealthStatus>("unknown");
  // Track the latest in-flight request so the interval does not pile
  // up overlapping fetches.
  const inFlightRef = useRef<Promise<void> | null>(null);
  // Mounted guard so async setState after unmount is a no-op (avoids
  // React's "set state on unmounted component" warning).
  const mountedRef = useRef(true);

  const probe = useCallback(async (): Promise<void> => {
    const base = getApiBaseUrl();
    try {
      const response = await fetch(`${base.replace(/\/$/, "")}/health`);
      if (!response.ok) {
        throw new ApiError({
          message: `Health probe failed: ${response.status} ${response.statusText}`,
          status: response.status,
          url: response.url,
        });
      }
      const raw = await response.json();
      const parsed = healthResponseSchema.parse(raw);
      if (!mountedRef.current) return;
      setData(parsed);
      setError(null);
      setStatus("online");
    } catch (e) {
      if (!mountedRef.current) return;
      setError(e instanceof Error ? e : new Error(String(e)));
      setStatus("offline");
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const refresh = useCallback(async (): Promise<void> => {
    if (inFlightRef.current) return inFlightRef.current;
    const p = probe().finally(() => {
      inFlightRef.current = null;
    });
    inFlightRef.current = p;
    return p;
  }, [probe]);

  useEffect(() => {
    if (!enabled) {
      // setState is intentional: the hook's contract says
      // `status` resets to "unknown" when polling is disabled.
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setStatus("unknown");
      return;
    }
    // First probe immediately, then on the interval.
    void refresh();
    const id = window.setInterval(() => {
      void refresh();
    }, intervalMs);
    return () => {
      window.clearInterval(id);
    };
  }, [enabled, intervalMs, refresh]);

  return {
    data,
    error,
    status,
    isOnline: status === "online",
    refresh,
  };
}
