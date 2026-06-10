/**
 * Tests for the `useGraphSearch` hook — T22.
 *
 * The hook is a thin wrapper over `client.graphSearch` (which
 * hits the MCP `graph_search` tool). These tests use the global
 * MSW server to intercept the call and return a fixture; the
 * hook's `useState` accumulator is then asserted.
 */
import { describe, expect, it, beforeEach, afterEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { server } from "../mocks/node";

import { useGraphSearch } from "./useGraphSearch";

describe("useGraphSearch — T22", () => {
  beforeEach(() => {
    server.use(
      http.post("/api/mcp/tools/call", async ({ request }) => {
        const body = (await request.clone().json()) as {
          name?: string;
          args?: { cursor?: string; limit?: number };
        };
        const cursor = body.args?.cursor;
        const limit = body.args?.limit ?? 50;

        // First page: 2 hits, next_cursor "1".
        if (cursor === undefined) {
          return HttpResponse.json({
            tool_name: "graph_search",
            version: "0.0.0",
            timestamp: "2026-06-10T00:00:00Z",
            provenance: null,
            payload: {
              results: [
                {
                  node: {
                    id: "doc:adr-0007.md#adr-7",
                    label: "ADR-0007: Adopt GraphQL",
                    kind: "decision",
                    source_path: "docs/adr/0007.md",
                    metadata: { status: "accepted" },
                  },
                  score: 1.0,
                  raw_rank: 1.0,
                },
                {
                  node: {
                    id: "doc:adr-0008.md#adr-8",
                    label: "ADR-0008: Use Federation",
                    kind: "decision",
                    source_path: "docs/adr/0008.md",
                    metadata: { status: "proposed" },
                  },
                  score: 1.0,
                  raw_rank: 1.0,
                },
              ],
              total_count: 3,
              next_cursor: "1",
              raw_rank: 1.0,
              normalized_score: 1.0,
            },
            suggested_follow_ups: [],
          });
        }
        // Second page: 1 hit, no more pages.
        return HttpResponse.json({
          tool_name: "graph_search",
          version: "0.0.0",
          timestamp: "2026-06-10T00:00:00Z",
          provenance: null,
          payload: {
            results: [
              {
                node: {
                  id: "doc:readme.md#intro",
                  label: "Project README",
                  kind: "doc",
                  source_path: "README.md",
                  metadata: { section: "intro" },
                },
                score: 0.5,
                raw_rank: 0.5,
              },
            ],
            total_count: 3,
            next_cursor: null,
            raw_rank: 0.5,
            normalized_score: 0.5,
          },
          suggested_follow_ups: [],
        });
      }),
    );
  });

  afterEach(() => {
    server.resetHandlers();
  });

  it("useGraphSearch_returns_results on first page", async () => {
    const { result } = renderHook(() =>
      useGraphSearch("ADR", { limit: 50, enabled: true }),
    );

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });
    expect(result.current.results).toHaveLength(2);
    expect(result.current.totalCount).toBe(3);
    expect(result.current.nextCursor).toBe("1");
  });

  it("useGraphSearch_loadMore appends the next page", async () => {
    const { result } = renderHook(() =>
      useGraphSearch("ADR", { limit: 50, enabled: true }),
    );

    await waitFor(() => {
      expect(result.current.results).toHaveLength(2);
    });
    await act(async () => {
      await result.current.loadMore();
    });
    await waitFor(() => {
      expect(result.current.results).toHaveLength(3);
    });
    expect(result.current.nextCursor).toBeNull();
  });

  it("useGraphSearch_reset clears the accumulator", async () => {
    const { result } = renderHook(() =>
      useGraphSearch("ADR", { limit: 50, enabled: true }),
    );

    await waitFor(() => {
      expect(result.current.results).toHaveLength(2);
    });
    act(() => {
      result.current.reset();
    });
    expect(result.current.results).toHaveLength(0);
    expect(result.current.totalCount).toBe(0);
    expect(result.current.nextCursor).toBeNull();
  });

  it("useGraphSearch is a no-op when disabled", async () => {
    const { result } = renderHook(() =>
      useGraphSearch("ADR", { enabled: false }),
    );
    // Even after a tick, the results stay empty.
    await new Promise((r) => setTimeout(r, 30));
    expect(result.current.results).toHaveLength(0);
    expect(result.current.isLoading).toBe(false);
  });
});
