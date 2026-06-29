/**
 * Tests for `useC4Hotspots` hook.
 *
 * Validates:
 * - Aggregates hotspots to C4 nodes by file prefix
 * - High threshold classification
 * - Med threshold classification
 * - Null for unmapped symbols
 */
import { describe, expect, it, afterEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { server } from "../mocks/node";

import { useC4Hotspots } from "./useC4Hotspots";

describe("useC4Hotspots", () => {
  // Reset handlers after each test
  afterEach(() => {
    server.resetHandlers();
  });

  describe("hotspot aggregation", () => {
    it("aggregates hotspots to C4 nodes by file prefix", async () => {
      const hotspotEntries = [
        {
          symbol_id: "file:apps/explorer-ui/src/main.ts:name:10",
          label: "main",
          pagerank: 0.8,
          in_degree: 5,
          out_degree: 3,
        },
        {
          symbol_id: "file:apps/explorer-ui/src/App.tsx:name:5",
          label: "App",
          pagerank: 0.6,
          in_degree: 10,
          out_degree: 2,
        },
      ];

      server.use(
        http.post("/api/mcp/tools/call", async () => {
          return HttpResponse.json({
            tool_name: "lens_hotspots",
            version: "0.0.0",
            timestamp: new Date().toISOString(),
            provenance: null,
            payload: hotspotEntries,
          });
        }),
      );

      const { result } = renderHook(() => useC4Hotspots("workspace-hotspot-1"));

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      const hotspots = result.current.data;
      expect(hotspots).toBeDefined();
      expect(hotspots.size).toBeGreaterThan(0);

      // Should have entry for component:apps/explorer-ui
      const explorerUIHotspot = hotspots.get("component:apps/explorer-ui");
      expect(explorerUIHotspot).toBeDefined();
      expect(explorerUIHotspot?.score).toBeGreaterThan(0);
    });

    it("classifies hotspots above HIGH_THRESHOLD (0.7) as 'high'", async () => {
      // Score = pagerank * 0.4 + in_degree * 0.6
      // To get score > 0.7, we need significant in_degree
      // Example: pagerank=0.5, in_degree=1.0 → 0.5*0.4 + 1.0*0.6 = 0.8 (> 0.7)
      const hotspotEntries = [
        {
          symbol_id: "file:apps/explorer-ui/src/component.ts:name:1",
          label: "component",
          pagerank: 0.5,
          in_degree: 1.0,
          out_degree: 3,
        },
      ];

      server.use(
        http.post("/api/mcp/tools/call", async () => {
          return HttpResponse.json({
            tool_name: "lens_hotspots",
            version: "0.0.0",
            timestamp: new Date().toISOString(),
            provenance: null,
            payload: hotspotEntries,
          });
        }),
      );

      const { result } = renderHook(() => useC4Hotspots("workspace-hotspot-high"));

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      const hotspots = result.current.data;
      const explorerHotspot = hotspots.get("component:apps/explorer-ui");
      expect(explorerHotspot?.kind).toBe("high");
    });

    it("classifies hotspots above MED_THRESHOLD (0.4) but below HIGH as 'med'", async () => {
      // Score = pagerank * 0.4 + in_degree * 0.6
      // Example: pagerank=0.5, in_degree=0.5 → 0.5*0.4 + 0.5*0.6 = 0.5 (> 0.4, < 0.7)
      const hotspotEntries = [
        {
          symbol_id: "file:crates/cognicode-graph-algos/src/lib.rs:name:1",
          label: "lib",
          pagerank: 0.5,
          in_degree: 0.5,
          out_degree: 1,
        },
      ];

      server.use(
        http.post("/api/mcp/tools/call", async () => {
          return HttpResponse.json({
            tool_name: "lens_hotspots",
            version: "0.0.0",
            timestamp: new Date().toISOString(),
            provenance: null,
            payload: hotspotEntries,
          });
        }),
      );

      const { result } = renderHook(() => useC4Hotspots("workspace-hotspot-med"));

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      const hotspots = result.current.data;
      const graphHotspot = hotspots.get("component:crates/cognicode-graph-algos");
      expect(graphHotspot?.kind).toBe("med");
    });

    it("low-score symbol is omitted from hotspot map", async () => {
      // Score = pagerank * 0.4 + in_degree * 0.6
      // Example: pagerank=0.2, in_degree=0.3 → 0.2*0.4 + 0.3*0.6 = 0.26 (< 0.4 MED threshold)
      // Per spec Req 2: Container with all-low-risk children is OMITTED
      const hotspotEntries = [
        {
          symbol_id: "file:apps/explorer-ui/src/main.ts:name:1",
          label: "main",
          pagerank: 0.2,
          in_degree: 0.3,
          out_degree: 1,
        },
      ];

      server.use(
        http.post("/api/mcp/tools/call", async () => {
          return HttpResponse.json({
            tool_name: "lens_hotspots",
            version: "0.0.0",
            timestamp: new Date().toISOString(),
            provenance: null,
            payload: hotspotEntries,
          });
        }),
      );

      const { result } = renderHook(() => useC4Hotspots("workspace-hotspot-low"));

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      // Low-score hotspot should NOT appear in the map
      expect(result.current.data.size).toBe(0);
    });

    it("returns empty for unmapped symbols (no prefix match)", async () => {
      const hotspotEntries = [
        {
          symbol_id: "file:totally/unknown/path/main.ts:name:1",
          label: "main",
          pagerank: 0.8,
          in_degree: 10,
          out_degree: 3,
        },
      ];

      server.use(
        http.post("/api/mcp/tools/call", async () => {
          return HttpResponse.json({
            tool_name: "lens_hotspots",
            version: "0.0.0",
            timestamp: new Date().toISOString(),
            provenance: null,
            payload: hotspotEntries,
          });
        }),
      );

      const { result } = renderHook(() => useC4Hotspots("workspace-hotspot-unknown"));

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      // No hotspot should be registered for unknown path
      expect(result.current.data.size).toBe(0);
    });

    it("handles glob patterns like **/ingest/", async () => {
      const hotspotEntries = [
        {
          symbol_id: "file:services/ingest/main.ts:name:1",
          label: "ingest_main",
          pagerank: 0.7,
          in_degree: 8,
          out_degree: 2,
        },
      ];

      server.use(
        http.post("/api/mcp/tools/call", async () => {
          return HttpResponse.json({
            tool_name: "lens_hotspots",
            version: "0.0.0",
            timestamp: new Date().toISOString(),
            provenance: null,
            payload: hotspotEntries,
          });
        }),
      );

      const { result } = renderHook(() => useC4Hotspots("workspace-hotspot-ingest"));

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      const hotspots = result.current.data;
      const ingestHotspot = hotspots.get("component:cmp-ingest");
      expect(ingestHotspot).toBeDefined();
    });
  });

  describe("when workspaceId is null", () => {
    it("does not fetch", async () => {
      const { result } = renderHook(() => useC4Hotspots(null));

      expect(result.current.isLoading).toBe(false);
      expect(result.current.data.size).toBe(0);
    });
  });
});
