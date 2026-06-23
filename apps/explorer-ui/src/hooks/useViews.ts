/**
 * `useViews` — the contextual view endpoints.
 *
 * Three sub-endpoints:
 * - `GET /api/objects/:object_id/views` — list of view descriptors
 *   (id + title) the object supports.
 * - `GET /api/objects/:object_id/views/:view_id` — the full
 *   `ContextualView` payload (blocks + relations + evidence).
 * - `GET /api/viewspecs` — list runtime ViewSpecs for (workspace_id, owner).
 *
 * `useAvailableViews` merges built-in descriptors with runtime ViewSpecs
 * from `listViewSpecs`, adding `is_builtin: false` and `source: "runtime"`
 * to the runtime entries so ViewTabs can badge them.
 */
import useSWR from "swr";
import { z } from "zod";

import {
  contextualViewSchema,
  viewDescriptorSchema,
} from "../api/schemas";
import {
  ApiError,
  listViewSpecs,
  makeSwrFetcher,
} from "../api/client";
import type { ContextualView } from "../api/types";
import type { ViewSpec } from "../api/schemas";

// Extended ViewDescriptor with runtime metadata
export interface ViewDescriptorPlus extends z.infer<typeof viewDescriptorSchema> {
  is_builtin: boolean;
  source: string | null;
}

const viewListSchema = z.array(viewDescriptorSchema);
type ViewList = z.infer<typeof viewListSchema>;

const viewListFetcher = makeSwrFetcher(viewListSchema);
const viewFetcher = makeSwrFetcher(contextualViewSchema);

/**
 * List the available views for an object, merged with runtime ViewSpecs.
 *
 * When `workspaceId` and `owner` are provided, runtime ViewSpecs from
 * `listViewSpecs(workspaceId, owner)` are merged after built-ins.
 * Runtime entries carry `is_builtin: false` and `source: "runtime"`.
 *
 * Built-ins come first (id-ordered from the backend); runtime entries
 * are appended after (title-ordered from the store).
 *
 * Pass `null` for `objectId` to skip the fetch entirely.
 * Pass `null` for `workspaceId` to get only built-ins.
 */
export function useAvailableViews(
  objectId: string | null,
  workspaceId: string | null = null,
  owner: string | null = null,
) {
  // Built-in views from the backend (with workspace context for runtime merge)
  const params = workspaceId ? `?workspace_id=${encodeURIComponent(workspaceId)}` : "";
  const builtinsKey = objectId
    ? `/objects/${encodeURIComponent(objectId)}/views${params}`
    : null;

  const { data: builtins } = useSWR<ViewList, ApiError>(builtinsKey, viewListFetcher, {
    revalidateOnFocus: false,
  });

  // Runtime ViewSpecs from the store
  const { data: runtimeSpecs } = useSWR<ViewSpec[] | null, ApiError>(
    workspaceId && owner ? ["viewspecs-list", workspaceId, owner] : null,
    async () => listViewSpecs(workspaceId!, owner!),
    { revalidateOnFocus: false },
  );

  // Merge built-ins with runtime entries
  const merged = mergeAvailableViews(builtins, runtimeSpecs ?? undefined);

  return { data: merged };
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

/**
 * Fetch runtime ViewSpecs for (workspaceId, owner) and merge them into
 * the available views list. The merge adds `is_builtin: false` and
 * `source: "runtime"` to each runtime entry.
 *
 * Returns `null` when workspaceId or owner is not provided.
 */
export function useRuntimeViewSpecs(
  workspaceId: string | null,
  owner: string | null,
) {
  return useSWR<ViewSpec[] | null, ApiError>(
    workspaceId && owner ? ["viewspecs-list", workspaceId, owner] : null,
    async () => {
      const specs = await listViewSpecs(workspaceId!, owner!);
      return specs;
    },
    { revalidateOnFocus: false },
  );
}

/**
 * Merge built-in view descriptors with runtime ViewSpecs.
 * Runtime entries get `is_builtin: false` and `source: "runtime"`.
 *
 * Built-ins come first (id-ordered from the backend); runtime entries
 * are appended after (title-ordered from the store).
 */
export function mergeAvailableViews(
  builtins: ViewList | undefined,
  runtimeSpecs: ViewSpec[] | undefined,
): ViewDescriptorPlus[] {
  const builtInDescriptors: ViewDescriptorPlus[] = (builtins ?? []).map((v) => ({
    ...v,
    is_builtin: v.is_builtin ?? true,
    source: v.source ?? null,
  }));

  const runtimeDescriptors: ViewDescriptorPlus[] = (runtimeSpecs ?? []).map((spec) => ({
    id: spec.id,
    title: spec.title,
    is_builtin: false,
    source: "runtime" as const,
  }));

  return [...builtInDescriptors, ...runtimeDescriptors];
}
