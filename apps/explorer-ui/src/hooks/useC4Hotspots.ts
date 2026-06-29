/**
 * `useC4Hotspots` — aggregate code hotspots to C4 container/component nodes.
 *
 * Uses the `lens_hotspots` MCP tool as an anchor to get high-risk symbols,
 * then maps them to C4 nodes via file path prefix matching and aggregates
 * the hotspot scores.
 *
 * Hotspot score formula: `pagerank * 0.4 + in_degree * 0.6`
 * Only aggregates at `c4-container` and `c4-component` levels (skips `c4-code`).
 */
import { useMemo } from "react";
import useSWR from "swr";

import { apiPost } from "../api/client";
import { z } from "zod";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** Hotspot entry returned by the `lens_hotspots` MCP tool. */
interface HotspotEntryDto {
  symbol_id: string;
  label: string;
  pagerank: number;
  in_degree: number;
  out_degree: number;
}

// ---------------------------------------------------------------------------
// File path → C4 node mapping
// ---------------------------------------------------------------------------

/** Mapping from file path prefix to C4 node id. */
const FILE_PATH_PREFIXES: Array<{ prefix: string; c4NodeId: string }> = [
  { prefix: "apps/explorer-ui/src/", c4NodeId: "component:apps/explorer-ui" },
  { prefix: "crates/cognicode-explorer/src/", c4NodeId: "component:crates/cognicode-explorer" },
  { prefix: "apps/api/src/", c4NodeId: "container:ctr-api" },
  { prefix: "crates/*/api/", c4NodeId: "container:ctr-api" },
  { prefix: "**/ingest/", c4NodeId: "component:cmp-ingest" },
  { prefix: "**/extract/", c4NodeId: "component:cmp-ingest" },
  { prefix: "crates/cognicode-graph-algos/", c4NodeId: "component:crates/cognicode-graph-algos" },
  { prefix: "crates/cognicode-graph-wasm/", c4NodeId: "component:crates/cognicode-graph-wasm" },
];

/**
 * Extract file path from a symbol id.
 * Symbol id format: `file:path:name:line` → extract `file:path`
 */
function extractFilePath(symbolId: string): string {
  const parts = symbolId.split(":");
  if (parts.length >= 2 && parts[1] != null) {
    return parts[1];
  }
  return symbolId;
}

/**
 * Find the C4 node id for a given symbol by matching its file path
 * against the FILE_PATH_PREFIXES. Returns `null` if no match found.
 */
function findC4NodeForSymbol(symbolId: string): string | null {
  const filePath = extractFilePath(symbolId);
  for (const { prefix, c4NodeId } of FILE_PATH_PREFIXES) {
    // Handle glob patterns (simplified — only `**/` and `*` at segment boundaries)
    if (prefix.includes("**/")) {
      // Match any path containing the suffix (after **/)
      const suffix = prefix.replace("**/", "");
      if (filePath.endsWith(suffix) || filePath.includes(suffix)) {
        return c4NodeId;
      }
    } else if (prefix.includes("*")) {
      // Handle single wildcard at segment boundary
      const regex = new RegExp("^" + prefix.replace(/\*/g, "[^/]+") + ".*$");
      if (regex.test(filePath)) {
        return c4NodeId;
      }
    } else if (filePath.startsWith(prefix)) {
      return c4NodeId;
    }
  }
  return null;
}

// ---------------------------------------------------------------------------
// Score thresholds (exported for use in tests)
// ---------------------------------------------------------------------------

export const HOTSPOT_HIGH_THRESHOLD = 0.7;
export const HOTSPOT_MED_THRESHOLD = 0.4;

/** Aggregated hotspot data for a C4 node. */
export interface C4HotspotData {
  score: number;
  kind: "high" | "med";
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useC4Hotspots(workspaceId: string | null) {
  const key = workspaceId
    ? (["c4-hotspots", workspaceId] as const)
    : null;

  const { data, error, isLoading, mutate } = useSWR<HotspotEntryDto[]>(
    key,
    async () => {
      if (!workspaceId) throw new Error("missing workspaceId");

      const mcpEnvelopeSchema = z.object({
        tool_name: z.string(),
        version: z.string(),
        timestamp: z.string(),
        provenance: z.unknown().nullable(),
        payload: z.array(
          z.object({
            symbol_id: z.string(),
            label: z.string(),
            pagerank: z.number(),
            in_degree: z.number(),
            out_degree: z.number(),
          }),
        ),
      });

      const envelope = await apiPost(
        "/mcp/tools/call",
        {
          name: "lens_hotspots",
          args: { object_id: `workspace:${workspaceId}` },
        },
        mcpEnvelopeSchema,
      );

      return envelope.payload;
    },
    {
      revalidateOnFocus: false,
      dedupingInterval: 30_000,
    },
  );

  /** Map of c4NodeId → aggregated hotspot data. */
  const hotspots = useMemo(() => {
    const result = new Map<string, C4HotspotData>();

    if (!data) return result;

    // Aggregate hotspots by C4 node
    const scoresByNode = new Map<string, number[]>();

    for (const entry of data) {
      const c4NodeId = findC4NodeForSymbol(entry.symbol_id);
      if (!c4NodeId) continue;

      // Compute hotspot score: pagerank * 0.4 + in_degree * 0.6
      const score = entry.pagerank * 0.4 + entry.in_degree * 0.6;

      if (!scoresByNode.has(c4NodeId)) {
        scoresByNode.set(c4NodeId, []);
      }
      scoresByNode.get(c4NodeId)!.push(score);
    }

    // Compute aggregated hotspot data per C4 node
    // Per spec Req 2: Container with all-low-risk children is OMITTED
    for (const [c4NodeId, scores] of scoresByNode) {
      // Use max score for the node (represents the most risky hotspot)
      const maxScore = Math.max(...scores);
      if (maxScore < HOTSPOT_MED_THRESHOLD) continue; // omit low entries
      const kind: "high" | "med" = maxScore >= HOTSPOT_HIGH_THRESHOLD ? "high" : "med";
      result.set(c4NodeId, { score: maxScore, kind });
    }

    return result;
  }, [data]);

  return { data: hotspots, error, isLoading, mutate };
}
