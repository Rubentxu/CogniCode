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
 *
 * Investigation views (ADR-005 E21-3, E21-4):
 * - `GET /api/investigations/:id/evidence-pack` — evidence pack view
 * - `GET /api/investigations/:id/composed-narrative` — composed narrative view
 */
import useSWR, { mutate } from "swr";

import {
  createInvestigationRequestSchema,
  investigationSchema,
  contextualViewSchema,
  pinEvidenceRequestSchema,
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
  AddArtifactRequestDto,
  CreateInvestigationRequestDto,
  InvestigationDto,
  ContextualView,
  PinEvidenceRequestDto,
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

/**
 * Pin evidence to an investigation (ADR-005 E21-2).
 */
export async function pinEvidence(
  investigationId: string,
  request: PinEvidenceRequestDto,
): Promise<void> {
  await apiPost(
    `/investigations/${encodeURIComponent(investigationId)}/evidence`,
    pinEvidenceRequestSchema.parse(request),
    // 204 No Content — no response body to validate.
    z.object({ ok: z.boolean() }),
  );
  // Invalidate the single-item cache so the new evidence appears immediately.
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

/**
 * Add an artifact to an investigation (ADR-005 E21-6).
 */
export async function addInvestigationArtifact(
  investigationId: string,
  request: AddArtifactRequestDto,
): Promise<void> {
  await apiPost(`investigations/${investigationId}/artifacts`, request, z.object({ ok: z.boolean() }));

  // Invalidate the investigation cache
  mutate(`investigations/${investigationId}`);
}

/**
 * Helper function to add a Mermaid diagram as an investigation artifact.
 * ADR-010 E24.1: provenance captures the source view context.
 */
export async function addMermaidArtifact(
  investigationId: string,
  title: string,
  mermaidText: string,
  generatedFrom?: string,
  provenance?: AddArtifactRequestDto["provenance"],
): Promise<void> {
  return addInvestigationArtifact(investigationId, {
    kind: "mermaid",
    title,
    content: mermaidText,
    generated_from: generatedFrom,
    provenance,
  });
}

/**
 * Helper function to add an SVG diagram as an investigation artifact.
 * ADR-010 E24.1: provenance captures the source view context.
 */
export async function addSvgArtifact(
  investigationId: string,
  title: string,
  svgContent: string,
  generatedFrom?: string,
  provenance?: AddArtifactRequestDto["provenance"],
): Promise<void> {
  return addInvestigationArtifact(investigationId, {
    kind: "svg",
    title,
    content: svgContent,
    generated_from: generatedFrom,
    provenance,
  });
}

/**
 * Helper function to add a draw.io export as an investigation artifact.
 * ADR-010 E24.1: provenance captures the source view context.
 */
export async function addDrawioArtifact(
  investigationId: string,
  title: string,
  drawioContent: string,
  generatedFrom?: string,
  provenance?: AddArtifactRequestDto["provenance"],
): Promise<void> {
  return addInvestigationArtifact(investigationId, {
    kind: "drawio",
    title,
    content: drawioContent,
    generated_from: generatedFrom,
    provenance,
  });
}

// Re-export z for use in this file
import { z } from "zod";

const evidencePackFetcher = makeSwrFetcher(contextualViewSchema);
const composedNarrativeFetcher = makeSwrFetcher(contextualViewSchema);

/**
 * Fetch the evidence pack view for an investigation (ADR-005 E21-3).
 *
 * Endpoint: `GET /api/investigations/:id/evidence-pack`
 * Response: `ContextualView`
 */
export function useInvestigationEvidencePack(investigationId: string | null) {
  return useSWR<ContextualView, ApiError>(
    investigationId
      ? `/investigations/${encodeURIComponent(investigationId)}/evidence-pack`
      : null,
    evidencePackFetcher,
    { revalidateOnFocus: false },
  );
}

/**
 * Fetch the composed narrative view for an investigation (ADR-005 E21-4).
 *
 * Endpoint: `GET /api/investigations/:id/composed-narrative`
 * Response: `ContextualView`
 */
export function useInvestigationComposedNarrative(investigationId: string | null) {
  return useSWR<ContextualView, ApiError>(
    investigationId
      ? `/investigations/${encodeURIComponent(investigationId)}/composed-narrative`
      : null,
    composedNarrativeFetcher,
    { revalidateOnFocus: false },
  );
}
