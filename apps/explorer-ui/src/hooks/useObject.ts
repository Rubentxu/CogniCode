/**
 * `useObject` — the inspectable object summary endpoint.
 *
 * Endpoint: `GET /api/objects/:mvp_id`
 *
 * Returns the `InspectableObjectSummary` for a given MVP id. The
 * summary carries `available_views`, which the Object Inspector uses
 * to render the tab strip.
 */
import useSWR from "swr";

import { inspectableObjectSummarySchema } from "../api/schemas";
import { ApiError, makeSwrFetcher } from "../api/client";
import type { InspectableObjectSummary } from "../api/types";

const objectFetcher = makeSwrFetcher(inspectableObjectSummarySchema);

/**
 * Fetch the object summary for a given MVP id. Pass `null` to skip
 * the fetch (e.g. when no object is selected).
 */
export function useObject(
  objectId: string | null,
) {
  return useSWR<InspectableObjectSummary, ApiError>(
    objectId ? `/objects/${encodeURIComponent(objectId)}` : null,
    objectFetcher,
  );
}
