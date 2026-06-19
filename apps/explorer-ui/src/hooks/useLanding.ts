/**
 * `useLanding` — fetch + cache a `LandingPayload` for a workspace,
 * using SWR for revalidation.
 *
 * Used by the `GraphLanding` component to display the initial graph
 * view when no object is selected.
 */
import useSWR from "swr";

import { fetchLanding } from "../api/client";
import type { LandingPayload } from "../api/types";

export function useLanding(workspaceId: string | null) {
  const key = workspaceId
    ? ["/workspaces/:id/landing", workspaceId] as const
    : null;
  const { data, error, isLoading, mutate } = useSWR<LandingPayload>(
    key,
    async () => {
      if (!workspaceId) throw new Error("missing workspaceId");
      return fetchLanding(workspaceId);
    },
    { revalidateOnFocus: false, dedupingInterval: 10_000 },
  );
  return { data: data ?? null, error, isLoading, mutate };
}
