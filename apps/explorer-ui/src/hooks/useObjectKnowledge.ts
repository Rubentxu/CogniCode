/**
 * `useObjectKnowledge` — fetch knowledge objects linked to a given
 * `object_id` (E27.3 knowledge rail).
 *
 * Calls `GET /api/objects/:object_id/related-knowledge` which returns
 * `{ adrs, docs, evidence }` arrays. While the backend currently returns
 * empty arrays (Phase 1 stub), the frontend is wired against the locked
 * shape so future linking logic lights up automatically.
 */
import useSWR from "swr";

import { ApiError, makeSwrFetcher } from "../api/client";
import { z } from "zod";

export const relatedKnowledgeSchema = z.object({
  adrs: z.array(z.unknown()),
  docs: z.array(z.unknown()),
  evidence: z.array(z.unknown()),
});

export type RelatedKnowledge = z.infer<typeof relatedKnowledgeSchema>;

const fetcher = makeSwrFetcher(relatedKnowledgeSchema);

/**
 * Fetch knowledge linked to a given MVP id. Pass `null` to skip the
 * fetch. The hook always returns the shape `{ adrs, docs, evidence }`
 * (empty arrays by default) so callers don't need to handle null.
 */
export function useObjectKnowledge(
  objectId: string | null,
): { data: RelatedKnowledge; isLoading: boolean; error: ApiError | undefined } {
  const { data, isLoading, error } = useSWR<RelatedKnowledge, ApiError>(
    objectId ? `/objects/${encodeURIComponent(objectId)}/related-knowledge` : null,
    fetcher,
    { revalidateOnFocus: false },
  );
  return {
    data: data ?? { adrs: [], docs: [], evidence: [] },
    isLoading,
    error,
  };
}