/**
 * Tests for `useSpotter` hook (e13-wave-1).
 *
 * Validates:
 * - Returns SpotterSearchResult[] (discriminated union)
 * - Preserves viewspec hits (not dropped)
 * - Returns investigation and scope variants
 */
import { describe, expect, it } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { server } from "../mocks/node";

import { useSpotter } from "./useSpotter";
import type { SpotterSearchResult } from "../api/schemas";
import { spotterResultSchema, viewSpecSummarySchema } from "../api/schemas";

// Test fixtures
const baseResult = {
  object: {
    id: "symbol:test::func",
    object_type: "symbol" as const,
    label: "test::func",
    subtitle: "test.rs:1",
    properties: [],
    available_views: [],
  },
  score: 0.9,
  match_type: "name_exact",
};

const baseViewSpec = {
  id: "550e8400-e29b-41d4-a716-446655440000",
  title: "Overview",
  view_kind: "vertical_slice",
  applies_to: "symbol",
  owner: "system",
  updated_at: "2026-06-27T00:00:00Z",
};

describe("useSpotter", () => {
  // Reset handlers after each test
  afterEach(() => {
    server.resetHandlers();
  });

  it("returns SpotterSearchResult[] (discriminated union)", async () => {
    const wire = [
      { kind: "symbol", result: baseResult },
      { kind: "file", result: baseResult },
    ];

    server.use(
      http.get("/api/workspaces/:workspace_id/spotter", () => {
        return HttpResponse.json(wire);
      }),
    );

    const { result } = renderHook(() =>
      useSpotter({ workspaceId: "ws-1", q: "test" }),
    );

    await waitFor(() => {
      expect(result.current.data).toBeDefined();
    });

    // Assert return type is SpotterSearchResult[]
    expect(Array.isArray(result.current.data)).toBe(true);
    const data = result.current.data as SpotterSearchResult[];
    expect(data).toHaveLength(2);
    expect(data[0]!.kind).toBe("symbol");
    expect(data[1]!.kind).toBe("file");
  });

  it("preserves viewspec hits (not dropped)", async () => {
    const wire = [
      { kind: "symbol", result: baseResult },
      { kind: "viewspec", result: baseViewSpec },
    ];

    server.use(
      http.get("/api/workspaces/:workspace_id/spotter", () => {
        return HttpResponse.json(wire);
      }),
    );

    // Use unique query to avoid SWR cache collision with other tests
    const { result } = renderHook(() =>
      useSpotter({ workspaceId: "ws-1", q: "viewspec-test" }),
    );

    await waitFor(() => {
      expect(result.current.data).toBeDefined();
    });

    const data = result.current.data as SpotterSearchResult[];
    // viewspec hit must be preserved
    expect(data).toHaveLength(2);
    const viewSpecHit = data.find((h) => h.kind === "viewspec");
    expect(viewSpecHit).toBeDefined();
    expect(viewSpecHit!.kind).toBe("viewspec");
    // viewspec result has ViewSpecSummary shape
    expect((viewSpecHit as { result: typeof baseViewSpec }).result.title).toBe("Overview");
  });

  it("returns investigation and scope variants", async () => {
    const wire = [
      { kind: "investigation", result: baseResult },
      { kind: "scope", result: baseResult },
    ];

    server.use(
      http.get("/api/workspaces/:workspace_id/spotter", () => {
        return HttpResponse.json(wire);
      }),
    );

    // Use unique query to avoid SWR cache collision with other tests
    const { result } = renderHook(() =>
      useSpotter({ workspaceId: "ws-1", q: "investigation-scope-test" }),
    );

    await waitFor(() => {
      expect(result.current.data).toBeDefined();
    });

    const data = result.current.data as SpotterSearchResult[];
    const investigationHit = data.find((h) => h.kind === "investigation");
    const scopeHit = data.find((h) => h.kind === "scope");
    expect(investigationHit).toBeDefined();
    expect(scopeHit).toBeDefined();
  });
});
