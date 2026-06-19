/**
 * `useArchitecture` — fetch + cache a C4 component graph for a workspace,
 * using SWR for revalidation.
 *
 * Used by the `GraphLanding` component when `perspective === "c4"` to
 * display directory components instead of entry points.
 */
import useSWR from "swr";

import { fetchArchitecture } from "../api/client";
import type { ArchitecturePayload } from "../api/types";

export function useArchitecture(workspaceId: string | null) {
  const key = workspaceId
    ? ["/workspaces/:id/architecture", workspaceId] as const
    : null;
  const { data, error, isLoading, mutate } = useSWR<ArchitecturePayload>(
    key,
    async () => {
      if (!workspaceId) throw new Error("missing workspaceId");
      return fetchArchitecture(workspaceId);
    },
    { revalidateOnFocus: false, dedupingInterval: 10_000 },
  );
  return { data: data ?? null, error, isLoading, mutate };
}
