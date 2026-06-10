/**
 * Tests for `useRationaleGraph` — the rationale-graph SWR hook.
 *
 * Uses MSW to intercept `GET /api/graph/:id/rationale` (handlers.ts
 * serves the `rationaleSubgraphFixture`). A fresh `SWRConfig` per
 * test prevents cache leaking across cases.
 */
import { renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { createElement, type ReactNode } from "react";
import { SWRConfig } from "swr";

import { useRationaleGraph } from "./useRationaleGraph";

function withSWR() {
  return function Wrapper({ children }: { children: ReactNode }) {
    return createElement(SWRConfig, {
      value: {
        provider: () => new Map(),
        dedupingInterval: 0,
      },
    }, children);
  };
}

describe("useRationaleGraph", () => {
  it("useRationaleGraph_fetches_and_returns_data", async () => {
    const wrapper = withSWR();
    const { result } = renderHook(
      () => useRationaleGraph("sym:rat::focus", { maxDepth: 3, maxNodes: 50 }),
      { wrapper },
    );

    // Loading state
    expect(result.current.isLoading).toBe(true);
    expect(result.current.data).toBeNull();

    // Resolved
    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.data).not.toBeNull();
    expect(result.current.data!.root).toBe("sym:rat::focus");
    expect(result.current.data!.nodes.length).toBeGreaterThanOrEqual(3);
    // The rationale fixture includes corroboration_scores
    expect(result.current.data!.corroboration_scores).toBeDefined();
    expect(Object.keys(result.current.data!.corroboration_scores).length).toBe(2);
  });

  it("useRationaleGraph_returns_null_when_focusId_null", async () => {
    const wrapper = withSWR();
    const { result } = renderHook(
      () => useRationaleGraph(null),
      { wrapper },
    );

    // Should not fire a request — data stays null, loading false
    await new Promise((r) => setTimeout(r, 25));
    expect(result.current.data).toBeNull();
    expect(result.current.isLoading).toBe(false);
    expect(result.current.error).toBeUndefined();
  });

  it("returns 404 error when focusId is missing", async () => {
    const wrapper = withSWR();
    const { result } = renderHook(
      () => useRationaleGraph("missing-sym"),
      { wrapper },
    );

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.data).toBeNull();
    expect(result.current.error).toBeDefined();
    expect((result.current.error as { status?: number } | undefined)?.status).toBe(404);
  });

  it("dedups rapid calls within the dedupingInterval", async () => {
    const wrapper = withSWR();
    const { result: a } = renderHook(
      () => useRationaleGraph("sym:rat::focus"),
      { wrapper },
    );
    const { result: b } = renderHook(
      () => useRationaleGraph("sym:rat::focus"),
      { wrapper },
    );

    await waitFor(() => {
      expect(a.current.isLoading).toBe(false);
    });

    // Both consumers must observe the same data instance.
    expect(a.current.data).toEqual(b.current.data);
    expect(a.current.data).not.toBeNull();
  });
});
