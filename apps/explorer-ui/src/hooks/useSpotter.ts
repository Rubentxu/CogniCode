/**
 * `useSpotter` — fuzzy search across the workspace graph.
 *
 * Endpoint: `GET /api/workspaces/:workspace_id/spotter?q=&kind=`
 *
 * The query is the cache key — SWR dedupes identical keys while a
 * request is in flight, so opening the Spotter and typing fast does
 * not flood the backend.
 */
import useSWR from "swr";
import { z } from "zod";

import { spotterResultSchema } from "../api/schemas";
import { ApiError, makeSwrFetcher } from "../api/client";

const spotterResultsSchema = z.array(spotterResultSchema);
export type SpotterResults = z.infer<typeof spotterResultsSchema>;

const spotterFetcher = makeSwrFetcher(spotterResultsSchema);

export type UseSpotterArgs = {
  workspaceId: string | null;
  q: string;
  kind?: string;
};

/**
 * Return the spotter hits for `(workspaceId, q, kind)`.
 *
 * - `workspaceId === null` or `q.length === 0` short-circuits to a
 *   SWR `null` key (no fetch).
 * - SWR's built-in dedup means concurrent renders of the same query
 *   share a single in-flight request.
 */
export function useSpotter(
  { workspaceId, q, kind }: UseSpotterArgs,
) {
  const trimmed = q.trim();
  const enabled = Boolean(workspaceId) && trimmed.length > 0;
  return useSWR<SpotterResults, ApiError>(
    enabled
      ? [
          `/workspaces/${encodeURIComponent(workspaceId!)}/spotter`,
          { q: trimmed, kind: kind ?? undefined },
        ]
      : null,
    spotterFetcher,
    {
      keepPreviousData: true,
      revalidateOnFocus: false,
    },
  );
}
