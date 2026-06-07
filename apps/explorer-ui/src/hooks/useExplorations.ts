/**
 * `useExplorations` — save and list saved explorations.
 *
 * The backend exposes:
 * - `POST /api/explorations` — save the current path
 *   (request: `SaveExplorationRequest`, response: `ExplorationPath`)
 *
 * The list endpoint is a derived SWR key — we cache the most recent
 * save so the UI can render without a follow-up fetch.
 */
import useSWR, { mutate } from "swr";
import { z } from "zod";

import {
  decisionArtifactSummarySchema,
  explorationPathSchema,
  generateArtifactRequestSchema,
  saveExplorationRequestSchema,
} from "../api/schemas";
import {
  ApiError,
  apiPost,
  makeSwrFetcher,
} from "../api/client";
import type {
  DecisionArtifactSummary,
  ExplorationPath,
} from "../api/types";

const explorationsListSchema = z.array(explorationPathSchema);
type ExplorationsList = z.infer<typeof explorationsListSchema>;

const explorationsListFetcher = makeSwrFetcher(explorationsListSchema);
const artifactFetcher = makeSwrFetcher(decisionArtifactSummarySchema);

/**
 * List the saved explorations for the current workspace.
 *
 * Pass `workspaceId === null` to skip the fetch. The list is cached
 * under a workspace-scoped key so two workspaces in the same session
 * do not collide.
 */
export function useExplorations(
  workspaceId: string | null,
) {
  return useSWR<ExplorationsList, ApiError>(
    workspaceId
      ? `/workspaces/${encodeURIComponent(workspaceId)}/explorations`
      : null,
    explorationsListFetcher,
    {
      revalidateOnFocus: false,
    },
  );
}

/**
 * Save the current exploration. The request is validated locally
 * before hitting the network, and the returned path is merged into
 * the cached list so `useExplorations` re-renders without a refetch.
 */
export async function saveExploration(
  request: unknown,
): Promise<ExplorationPath> {
  const parsedRequest = saveExplorationRequestSchema.parse(request);
  const path = await apiPost(
    "/explorations",
    parsedRequest,
    explorationPathSchema,
  );
  const listKey = `/workspaces/${encodeURIComponent(parsedRequest.workspace_id)}/explorations`;
  await mutate(listKey, (current: ExplorationsList | undefined) => {
    if (!current) return [path];
    return [...current, path];
  }, false);
  return path;
}

/**
 * Generate a decision artifact (markdown / html / json replay) for
 * a saved exploration. Returns the artifact summary; the full
 * content is in the `content` field.
 */
export async function generateArtifact(
  explorationId: string,
  format: "markdown" | "html" | "json_replay",
): Promise<DecisionArtifactSummary> {
  const parsed = generateArtifactRequestSchema.parse({ format });
  const artifact = await apiPost(
    `/explorations/${encodeURIComponent(explorationId)}/artifacts`,
    parsed,
    decisionArtifactSummarySchema,
  );
  // Warm the per-exploration cache so a follow-up UI render hits.
  await mutate(
    `/explorations/${encodeURIComponent(explorationId)}/artifacts/${format}`,
    artifact,
    false,
  );
  return artifact;
}

/**
 * Hook variant for the artifact endpoint. Pass `format = null` to
 * skip the fetch.
 */
export function useArtifact(
  explorationId: string | null,
  format: "markdown" | "html" | "json_replay" | null,
) {
  return useSWR<DecisionArtifactSummary, ApiError>(
    explorationId && format
      ? `/explorations/${encodeURIComponent(explorationId)}/artifacts/${format}`
      : null,
    artifactFetcher,
  );
}
