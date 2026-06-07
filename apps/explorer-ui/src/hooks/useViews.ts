/**
 * `useViews` — the contextual view endpoints.
 *
 * Two sub-endpoints:
 * - `GET /api/objects/:object_id/views` — list of view descriptors
 *   (id + title) the object supports.
 * - `GET /api/objects/:object_id/views/:view_id` — the full
 *   `ContextualView` payload (blocks + relations + evidence).
 *
 * Exposed as two hooks so callers can pick the granularity.
 */
import useSWR from "swr";
import { z } from "zod";

import {
  contextualViewSchema,
  viewDescriptorSchema,
} from "../api/schemas";
import { ApiError, makeSwrFetcher } from "../api/client";
import type { ContextualView } from "../api/types";

const viewListSchema = z.array(viewDescriptorSchema);
type ViewList = z.infer<typeof viewListSchema>;

const viewListFetcher = makeSwrFetcher(viewListSchema);
const viewFetcher = makeSwrFetcher(contextualViewSchema);

/** List the available views for an object. */
export function useAvailableViews(
  objectId: string | null,
) {
  return useSWR<ViewList, ApiError>(
    objectId ? `/objects/${encodeURIComponent(objectId)}/views` : null,
    viewListFetcher,
  );
}

/**
 * Fetch a specific contextual view. Pass `null` for either id to
 * skip the fetch.
 */
export function useViews(
  objectId: string | null,
  viewId: string | null,
) {
  return useSWR<ContextualView, ApiError>(
    objectId && viewId
      ? `/objects/${encodeURIComponent(objectId)}/views/${encodeURIComponent(viewId)}`
      : null,
    viewFetcher,
    {
      keepPreviousData: true,
    },
  );
}
