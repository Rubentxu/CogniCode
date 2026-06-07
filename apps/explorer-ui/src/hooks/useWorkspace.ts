/**
 * `useWorkspace` — list of available workspaces + the open/index actions.
 *
 * Endpoints:
 * - `GET  /api/workspaces` — list `WorkspaceSummary` for the UI
 * - `POST /api/workspaces/open` — open by root path (returns summary)
 * - `POST /api/workspaces/:id/index` — currently `501 Not Implemented`
 *
 * The list hook is the source of truth for the "pick a workspace"
 * landing screen; the mutations mutate the cache so any consumers
 * re-render without a follow-up fetch.
 */
import useSWR, { mutate, type SWRConfiguration } from "swr";
import { z } from "zod";

import {
  indexWorkspaceRequestSchema,
  workspaceSummarySchema,
} from "../api/schemas";
import { apiPost, ApiError, makeSwrFetcher } from "../api/client";
import type { WorkspaceSummary } from "../api/types";

const workspaceListSchema = z.array(workspaceSummarySchema);
export type WorkspaceList = z.infer<typeof workspaceListSchema>;

const workspaceListFetcher = makeSwrFetcher(workspaceListSchema);

/**
 * List the workspaces the UI knows about. The list is the source of
 * truth for the workspace picker; pass `null` to skip the fetch.
 */
export function useWorkspaceList(
  options?: SWRConfiguration<WorkspaceList, ApiError>,
) {
  return useSWR<WorkspaceList, ApiError>("/workspaces", workspaceListFetcher, {
    revalidateOnFocus: false,
    ...options,
  });
}

/**
 * Read a single workspace summary by id. Uses the list cache when
 * possible to avoid an extra fetch.
 */
export function useWorkspace(
  workspaceId: string | null,
  options?: SWRConfiguration<WorkspaceSummary, ApiError>,
) {
  return useSWR<WorkspaceSummary, ApiError>(
    workspaceId ? `/workspaces/${encodeURIComponent(workspaceId)}` : null,
    async (key: string) => {
      // Try the list cache first — if the workspace is already loaded
      // there, return the matching summary without a network call.
      const listKey = "/workspaces";
      const list = (await mutate(listKey)) as WorkspaceList | undefined;
      const found = list?.find((w) => w.id === workspaceId);
      if (found) return found;
      // Fall back to a synthetic fetch — we don't have a single-workspace
      // endpoint, so return what the list cache had or throw.
      throw new ApiError({
        message: `Workspace not in list cache: ${workspaceId}`,
        status: 404,
        url: key,
      });
    },
    {
      revalidateOnFocus: false,
      ...options,
    },
  );
}

/**
 * Open a workspace by `rootPath` and update the cache so any
 * `useWorkspaceList` consumers see the new entry.
 */
export async function openWorkspace(rootPath: string): Promise<WorkspaceSummary> {
  // Validate the request at the call site so a bad shape fails fast
  // before hitting the network.
  z.object({ root_path: z.string().min(1) }).parse({ root_path: rootPath });

  const summary = await apiPost(
    "/workspaces/open",
    { root_path: rootPath },
    workspaceSummarySchema,
  );
  await mutate(
    "/workspaces",
    (current: WorkspaceList | undefined) => {
      if (!current) return [summary];
      const idx = current.findIndex((w) => w.id === summary.id);
      if (idx === -1) return [...current, summary];
      const next = current.slice();
      next[idx] = summary;
      return next;
    },
    false,
  );
  return summary;
}

/**
 * Trigger an index pass for an open workspace. The backend currently
 * returns `501 Not Implemented` for this route — we surface the
 * ApiError to the caller.
 */
export async function indexWorkspace(
  workspaceId: string,
  strategy?: string,
): Promise<never> {
  indexWorkspaceRequestSchema.parse({ strategy: strategy ?? null });
  await apiPost(
    `/workspaces/${encodeURIComponent(workspaceId)}/index`,
    { strategy: strategy ?? null },
    // The endpoint returns ApiError JSON; we never reach a successful
    // parse. Use `z.unknown()` as a permissive receiver.
    z.unknown(),
  );
  throw new ApiError({
    message: "index_workspace returned a success response but is not implemented",
    status: 501,
    url: `/workspaces/${workspaceId}/index`,
  });
}
