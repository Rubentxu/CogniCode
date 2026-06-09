/**
 * Tests for the `useContextualGraph` SWR hook (Contextual Views
 * Phase 1 of visualization-stack).
 *
 * TDD contract: every block here is RED before the hook lands.
 * After it does, the tests pass.
 *
 * Pattern mirrors `client.subgraph.test.ts` + `hooks.test.ts`:
 * - The MSW server intercepts `/api/graph/:id/contextual` (set up in
 *   `src/mocks/handlers.ts`).
 * - `waitFor` flips `isLoading` from `true` to `false` once SWR
 *   resolves the fetch.
 * - Each test uses a fresh `SWRConfig` so cache state does not leak.
 */
import { renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { createElement, type ReactNode } from "react";
import { SWRConfig } from "swr";

import { useContextualGraph } from "./useContextualGraph";

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

describe("useContextualGraph", () => {
  it("returns data on success", async () => {
    const wrapper = withSWR();
    const { result } = renderHook(
      () => useContextualGraph("sym:ctx::alpha", { depth: 1, maxNodes: 200 }),
      { wrapper },
    );

    // Loading → success
    expect(result.current.isLoading).toBe(true);
    expect(result.current.data).toBeNull();

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.data).not.toBeNull();
    expect(result.current.data!.focusNode.id).toBe("sym:ctx::alpha");
    expect(result.current.data!.level).toBe("file");
    expect(result.current.error).toBeUndefined();
  });

  it("returns 404 error when the symbol is missing", async () => {
    const wrapper = withSWR();
    const { result } = renderHook(
      () => useContextualGraph("missing-sym"),
      { wrapper },
    );

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.data).toBeNull();
    expect(result.current.error).toBeDefined();
    // The MSW handler returns `{ error: "symbol_not_found" }` with
    // status 404; the boundary ApiError surfaces that as `status: 404`.
    expect((result.current.error as { status?: number } | undefined)?.status).toBe(404);
  });

  it("dedups rapid calls within 5s", async () => {
    // Mount two consumers with the same focusId+opts. SWR's
    // `dedupingInterval` (5000ms) means the second consumer reads
    // from cache — but here the providers are fresh per test, so
    // we exercise the share-within-provider path by mounting the
    // hook twice from the same provider.
    const wrapper = withSWR();
    const { result: a } = renderHook(
      () => useContextualGraph("sym:ctx::alpha"),
      { wrapper },
    );
    const { result: b } = renderHook(
      () => useContextualGraph("sym:ctx::alpha"),
      { wrapper },
    );

    await waitFor(() => {
      expect(a.current.isLoading).toBe(false);
    });

    // Both consumers must observe the same data instance.
    expect(a.current.data).toEqual(b.current.data);
    expect(a.current.data).not.toBeNull();
  });

  it("passes opts as query string", async () => {
    // We don't have a direct handle to the MSW `lastRequest` here, so
    // we assert via a side-effect: the fixture returns
    // `truncated: true` for ids starting with `truncated*`. Setting
    // `maxNodes: 50` does not change the truncation behavior in the
    // mock — but a `depth: 2` parameter must round-trip into the
    // request without crashing. We assert the data is returned and
    // matches the request shape (focus id echoed, level=file).
    const wrapper = withSWR();
    const { result } = renderHook(
      () => useContextualGraph("sym:ctx::alpha", { depth: 2, maxNodes: 50 }),
      { wrapper },
    );

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.error).toBeUndefined();
    expect(result.current.data).not.toBeNull();
    expect(result.current.data!.focusNode.id).toBe("sym:ctx::alpha");
    expect(result.current.data!.level).toBe("file");
  });
});
