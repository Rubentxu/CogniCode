/**
 * `useScanJob` — trigger workspace scans and poll job status.
 *
 * Endpoints:
 * - `POST /api/workspaces/:id/scan` → returns `ScanAccepted`
 * - `GET  /api/jobs/:job_id`      → returns `JobStatus`
 * - `GET  /api/workspaces/:id/graph/stats` → returns `GraphStats`
 */
import useSWR, { type SWRConfiguration } from "swr";
import { z } from "zod";

import { apiPost, ApiError, makeSwrFetcher, apiGet } from "../api/client";

// ── Schemas ────────────────────────────────────────────────────

const scanAcceptedSchema = z.object({
  job_id: z.string(),
  status: z.string(),
  message: z.string(),
});
export type ScanAccepted = z.infer<typeof scanAcceptedSchema>;

const scanProgressSchema = z.object({
  stage: z.string(),
  processed: z.number(),
  total: z.number(),
  failed: z.number(),
}).nullable();

const scanResultSchema = z.object({
  symbols: z.number(),
  edges: z.number(),
  duration_ms: z.number(),
  community_count: z.number().optional(),
  health_score: z.number().optional(),
}).nullable();

export const jobStatusSchema = z.object({
  job_id: z.string(),
  workspace_id: z.string(),
  status: z.enum(["running", "completed", "failed"]),
  progress: scanProgressSchema,
  result: scanResultSchema,
  started_at: z.string(),
  finished_at: z.string().nullable(),
});
export type JobStatus = z.infer<typeof jobStatusSchema>;

const graphStatsSchema = z.object({
  workspace_id: z.string(),
  symbol_count: z.number(),
  edge_count: z.number(),
  last_scan_at: z.string().nullable(),
});
export type GraphStats = z.infer<typeof graphStatsSchema>;

// ── API calls ──────────────────────────────────────────────────

/** Start a scan for a workspace. Returns job_id. */
export async function scanWorkspace(workspaceId: string): Promise<ScanAccepted> {
  return apiPost(
    `/workspaces/${encodeURIComponent(workspaceId)}/scan`,
    {}, // POST body can be empty
    scanAcceptedSchema,
  );
}

/** Get graph stats for a workspace. */
export async function getGraphStats(workspaceId: string): Promise<GraphStats> {
  return apiGet(
    `/workspaces/${encodeURIComponent(workspaceId)}/graph/stats`,
    graphStatsSchema,
  );
}

// ── Hooks ──────────────────────────────────────────────────────

/** Poll job status. Pass job_id or null to skip. */
export function useJobStatus(
  jobId: string | null,
  options?: SWRConfiguration<JobStatus, ApiError>,
) {
  const fetcher = makeSwrFetcher(jobStatusSchema);
  return useSWR<JobStatus, ApiError>(
    jobId ? `/jobs/${encodeURIComponent(jobId)}` : null,
    fetcher,
    {
      refreshInterval: jobId ? 500 : 0, // poll every 500ms while job exists
      revalidateOnFocus: false,
      ...options,
    },
  );
}

/** Get graph stats for a workspace. */
export function useGraphStats(
  workspaceId: string | null,
  options?: SWRConfiguration<GraphStats, ApiError>,
) {
  const fetcher = makeSwrFetcher(graphStatsSchema);
  return useSWR<GraphStats, ApiError>(
    workspaceId ? `/workspaces/${encodeURIComponent(workspaceId)}/graph/stats` : null,
    fetcher,
    {
      refreshInterval: 10_000, // refresh every 10s
      revalidateOnFocus: true,
      ...options,
    },
  );
}
