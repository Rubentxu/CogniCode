/**
 * `useRationaleGraph` — fetch + cache a `SubgraphResponse` with
 * corroboration scores for a given focus id, using SWR.
 *
 * Wraps `fetchRationale` with the same dedup + key strategy as
 * `useSubgraph` (5s `dedupingInterval`). The hook returns the SWR
 * triple `{ data, error, isLoading, mutate }` — typed against the
 * `SubgraphResponse` schema (which includes `corroboration_scores`).
 *
 * Usage:
 *   const { data, error, isLoading } = useRationaleGraph("sym:foo::bar");
 *   // → data is null on first render, then resolves when SWR finishes
 *
 * Defaults match the backend: max_depth=3, max_nodes=50.
 * Callers can override per-call.
 */
import useSWR from "swr";

import { fetchRationale, type RationaleOptions } from "../api/client";
import type { SubgraphResponse } from "../api/types";

export function useRationaleGraph(
  focusId: string | null,
  options: RationaleOptions = {},
) {
  const key = focusId
    ? ["/graph/:id/rationale", focusId, options] as const
    : null;
  const { data, error, isLoading, mutate } = useSWR<SubgraphResponse>(
    key,
    async () => {
      if (!focusId) throw new Error("missing focusId");
      return fetchRationale(focusId, options);
    },
    { revalidateOnFocus: false, dedupingInterval: 5_000 },
  );
  return { data: data ?? null, error, isLoading, mutate };
}
