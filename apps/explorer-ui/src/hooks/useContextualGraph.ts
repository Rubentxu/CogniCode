/**
 * `useContextualGraph` — fetch + cache a `ContextualGraphResponse`
 * for a given focus id, using SWR for revalidation.
 *
 * Wraps `fetchContextual` with the same dedup + key strategy as
 * `useSubgraph` (5s `dedupingInterval`). The hook returns the SWR
 * triple `{ data, error, isLoading, mutate }` — typed against the
 * `ContextualGraphResponse` schema.
 *
 * Usage:
 *   const { data, error, isLoading } = useContextualGraph("sym:foo::alpha", { depth: 1 });
 *   // → data is null on first render, then resolves to the response
 *   //   once SWR finishes the fetch.
 *
 * The `opts` are passed through to the backend as query params. When
 * `opts` is undefined (default), the backend applies its own
 * defaults: `level=file`, `depth=1`, `max_nodes=200`.
 */
import useSWR from "swr";

import { fetchContextual, type ContextualOptions } from "../api/client";
import type { ContextualGraphResponse } from "../api/types";

export function useContextualGraph(
  focusId: string | null,
  opts: ContextualOptions = {},
) {
  const key = focusId
    ? ["/graph/:id/contextual", focusId, opts] as const
    : null;
  const { data, error, isLoading, mutate } = useSWR<ContextualGraphResponse>(
    key,
    async () => {
      if (!focusId) throw new Error("missing focusId");
      return fetchContextual(focusId, opts);
    },
    { revalidateOnFocus: false, dedupingInterval: 5_000 },
  );
  return { data: data ?? null, error, isLoading, mutate };
}
