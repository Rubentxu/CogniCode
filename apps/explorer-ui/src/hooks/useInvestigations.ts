/**
 * `useInvestigations` — list and manage investigations (ADR-005 INV-1).
 *
 * The backend exposes:
 * - `GET /api/investigations?workspace_id=<id>` — list investigations
 *   (response: `InvestigationDto[]`)
 * - `POST /api/investigations` — create investigation
 *   (body: `CreateInvestigationRequestDto`)
 * - `GET /api/investigations/:id` — get single investigation
 * - `PUT /api/investigations/:id` — update investigation
 *   (body: `UpdateInvestigationRequestDto`)
 * - `DELETE /api/investigations/:id` — delete investigation
 */
import useSWR, { mutate } from "swr";

import {
  createInvestigationRequestSchema,
  investigationSchema,
  updateInvestigationRequestSchema,
} from "../api/schemas";
import {
  ApiError,
  apiGet,
  apiPost,
  apiPut,
  makeSwrFetcher,
} from "../api/client";
import type {
  CreateInvestigationRequestDto,
  InvestigationDto,
  UpdateInvestigationRequestDto,
} from "../api/types";

const investigationsListSchema = z.array(investigationSchema);
const investigationsListFetcher = makeSwrFetcher(investigationsListSchema);

type InvestigationsList = z.infer<typeof investigationsListSchema>;

// Zod schema for single investigation response (used by apiGet)
const investigationFetcherSchema = investigationSchema;

/**
 * List investigations for a workspace.
 *
 * Pass `workspaceId === null` to skip the fetch. The list is cached
 * under a workspace-scoped key so two workspaces do not collide.
 */
export function useInvestigations(workspaceId: string | null) {
  return useSWR<InvestigationsList, ApiError>(
    workspaceId ? `/investigations?workspace_id=${encodeURIComponent(workspaceId)}` : null,
    investigationsListFetcher,
    {
      revalidateOnFocus: false,
    },
  );
}

/**
 * Get a single investigation by ID.
 */
export function useInvestigation(investigationId: string | null) {
  return useSWR<InvestigationDto, ApiError>(
    investigationId ? `/investigations/${encodeURIComponent(investigationId)}` : null,
    (url) => apiGet(url, investigationFetcherSchema),
    {
      revalidateOnFocus: false,
    },
  );
}

/**
 * Create a new investigation.
 */
export async function createInvestigation(
  request: CreateInvestigationRequestDto,
): Promise<InvestigationDto> {
  const result = await apiPost(
    "/investigations",
    createInvestigationRequestSchema.parse(request),
    investigationSchema,
  );
  // Invalidate the list cache so the new investigation appears immediately.
  await mutate(
    (key: string) =>
      typeof key === "string" && key.startsWith("/investigations"),
    undefined,
    { revalidate: true },
  );
  return result;
}

/**
 * Update an existing investigation.
 */
export async function updateInvestigation(
  request: UpdateInvestigationRequestDto,
): Promise<void> {
  const parsed = updateInvestigationRequestSchema.parse(request);
  await apiPut(
    `/investigations/${encodeURIComponent(parsed.id)}`,
    parsed,
  );
  // Invalidate both the list cache and the single-item cache.
  await mutate(
    (key: string) =>
      typeof key === "string" &&
      (key.startsWith("/investigations") || key.includes(parsed.id)),
    undefined,
    { revalidate: true },
  );
}

/**
 * Delete an investigation by ID.
 */
export async function deleteInvestigation(investigationId: string): Promise<void> {
  await apiDelete(`/investigations/${encodeURIComponent(investigationId)}`);
  // Invalidate both the list cache and the single-item cache.
  await mutate(
    (key: string) =>
      typeof key === "string" &&
      (key.startsWith("/investigations") || key.includes(investigationId)),
    undefined,
    { revalidate: true },
  );
}

// Re-export z for use in the file
import { z } from "zod";
import { apiDelete } from "../api/client";
