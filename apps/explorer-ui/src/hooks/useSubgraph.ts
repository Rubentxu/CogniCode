/**
 * `useSubgraph` — fetch + cache a `SubgraphResponse` for a given
 * root id, using SWR for revalidation. Used by the 4th-column
 * `InteractiveGraph` in ultrawide viewports.
 *
 * Defaults match the backend: depth=3, direction=both, max_nodes=500.
 * Callers can override per-call.
 */
import useSWR from "swr";

import { fetchSubgraph, type SubgraphQuery } from "../api/client";
import type { SubgraphResponse } from "../api/types";

export function useSubgraph(rootId: string | null, params: SubgraphQuery = {}) {
  const key = rootId
    ? ["/graph/:id/subgraph", rootId, params] as const
    : null;
  const { data, error, isLoading, mutate } = useSWR<SubgraphResponse>(
    key,
    async () => {
      if (!rootId) throw new Error("missing rootId");
      return fetchSubgraph(rootId, params);
    },
    { revalidateOnFocus: false, dedupingInterval: 5_000 },
  );
  return { data: data ?? null, error, isLoading, mutate };
}
