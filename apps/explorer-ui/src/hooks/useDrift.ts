/**
 * `useDrift` — fetch + cache a `DriftReport` for a workspace,
 * using SWR for revalidation.
 *
 * Used by C4 overlay controls to display architecture drift findings.
 * Handles 404 gracefully by returning an empty report (no expected
 * architecture defined yet).
 */
import useSWR from "swr";

import { fetchDrift } from "../api/client";
import type { DriftReport } from "../api/types";

export function useDrift(workspaceId: string | null) {
  const key = workspaceId
    ? (["drift", workspaceId] as const)
    : null;
  const { data, error, isLoading, mutate } = useSWR<DriftReport>(
    key,
    async () => {
      if (!workspaceId) throw new Error("missing workspaceId");
      return fetchDrift(workspaceId);
    },
    { revalidateOnFocus: false, dedupingInterval: 10_000 },
  );
  return { data: data ?? undefined, error, isLoading, mutate };
}
