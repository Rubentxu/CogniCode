/**
 * `useSpotter` — fuzzy search across the workspace graph.
 *
 * Endpoint: `GET /api/workspaces/:workspace_id/spotter?q=&kind=`
 *
 * The query is the cache key — SWR dedupes identical keys while a
 * request is in flight, so opening the Spotter and typing fast does
 * not flood the backend.
 *
 * The backend returns `SpotterSearchResult` (a discriminated union with
 * `kind` + `result` payload). This hook validates the full union and
 * returns it directly so callers can switch on `kind` to render each
 * family appropriately (e13-wave-1).
 */
import useSWR from "swr";
import { z } from "zod";

import { spotterSearchResultSchema } from "../api/schemas";
import type { SpotterSearchResult } from "../api/schemas";
import { ApiError, makeSwrFetcher } from "../api/client";

/** Raw wire format — validated at the API boundary. */
const spotterWireSchema = z.array(spotterSearchResultSchema);
type SpotterWire = z.infer<typeof spotterWireSchema>;

const spotterFetcher = makeSwrFetcher(spotterWireSchema);

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
 *
 * The backend returns `SpotterSearchResult[]` (discriminated union).
 * We return the full union so callers can switch on `kind` to render
 * each family appropriately (investigation, scope, viewspec, etc.).
 */
export function useSpotter(
  { workspaceId, q, kind }: UseSpotterArgs,
) {
  const trimmed = q.trim();
  const enabled = Boolean(workspaceId) && trimmed.length > 0;

  async function fetcher(key: string | [string, RequestOpts["query"]]): Promise<SpotterSearchResult[]> {
    const wire = typeof key === "string"
      ? await (async () => {
          // Single-arg path — build URL from base + key
          const { apiGet } = await import("../api/client");
          return apiGet(key, spotterWireSchema);
        })()
      : await spotterFetcher(key);

    return wire;
  }

  return useSWR<SpotterSearchResult[], ApiError>(
    enabled
      ? [
          `/workspaces/${encodeURIComponent(workspaceId!)}/spotter`,
          { q: trimmed, kind: kind ?? undefined },
        ]
      : null,
    fetcher,
    {
      keepPreviousData: true,
      revalidateOnFocus: false,
    },
  );
}

export type SpotterResults = SpotterSearchResult[];
