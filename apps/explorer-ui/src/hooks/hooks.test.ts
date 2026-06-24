/**
 * Integration tests for the SWR hooks.
 *
 * Each hook test runs against the MSW server (started in
 * `src/test/setup.ts`). We use `renderHook` from @testing-library/react
 * + `waitFor` to assert the loading → success and error → retry flows.
 *
 * Cache + dedup is verified by mounting two consumers with the same
 * key and asserting SWR fires a single network request.
 */
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { http, HttpResponse } from "msw";
import { SWRConfig } from "swr";
import { createElement, type ReactNode } from "react";

import { server } from "../mocks/node";
import { useLenses } from "./useLenses";
import { useLensResult } from "./useLensResult";
import { useObject } from "./useObject";
import { useSpotter } from "./useSpotter";
import { useAvailableViews, useViews } from "./useViews";
import { useExplorations, generateArtifact } from "./useExplorations";
import {
  contextualViewFixture,
  decisionArtifactFixture,
  inspectableObjectFixture,
  lensDescriptorsFixture,
  lensResultFixture,
  spotterResultsFixture,
} from "../mocks/fixtures";

// eslint-disable-next-line @typescript-eslint/no-unused-vars -- intentionally stubbed
let _fetchCount = 0;
let spotterFetchCount = 0;
let artifactFetchCount = 0;

beforeEach(() => {
  _fetchCount = 0;
  spotterFetchCount = 0;
  artifactFetchCount = 0;
  server.resetHandlers();
});

afterEach(() => {
  // Defensive — reset() already clears runtime handlers, but make
  // sure we do not leak state between tests.
  server.resetHandlers();
});

// ============================================================================
// Helpers
// ============================================================================

/**
 * Wrap a hook in a fresh SWRConfig with a deterministic cache key
 * prefix so tests do not pollute each other.
 */
function withSWR() {
  return function Wrapper({ children }: { children: ReactNode }) {
    return createElement(
      SWRConfig,
      {
        value: {
          provider: () => new Map(),
          dedupingInterval: 0,
        },
      },
      children,
    );
  };
}

// ============================================================================
// useSpotter
// ============================================================================

describe("useSpotter", () => {
  it("loading → success: returns data after the fetch resolves", async () => {
    const wrapper = withSWR();
    const { result } = renderHook(
      () => useSpotter({ workspaceId: "ws-1", q: "build" }),
      { wrapper },
    );

    expect(result.current.isLoading).toBe(true);
    expect(result.current.data).toBeUndefined();

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.data).toBeDefined();
    expect(result.current.data!.length).toBe(spotterResultsFixture.length);
    expect(result.current.data![0]!.object.label).toBe(
      spotterResultsFixture[0]!.object.label,
    );
  });

  it("does not fetch when q is empty", async () => {
    const wrapper = withSWR();
    const { result } = renderHook(
      () => useSpotter({ workspaceId: "ws-1", q: "" }),
      { wrapper },
    );

    // SWR returns a "mutating: false, data: undefined" state with no
    // network call. Wait one tick to make sure the request has had
    // a chance to fire (it should not).
    await new Promise((r) => setTimeout(r, 25));
    expect(result.current.data).toBeUndefined();
    expect(result.current.isLoading).toBe(false);
  });

  it("dedupes concurrent renders with the same key", async () => {
    // Render a single hook that calls useSpotter twice — both share
    // the same SWR provider, so the second call should hit the
    // cache and resolve immediately.
    const wrapper = withSWR();
    const { result } = renderHook(
      () => {
        const a = useSpotter({ workspaceId: "ws-1", q: "build" });
        const b = useSpotter({ workspaceId: "ws-1", q: "build" });
        return { a, b };
      },
      { wrapper },
    );

    await waitFor(() => {
      expect(result.current.a.isLoading).toBe(false);
      expect(result.current.b.isLoading).toBe(false);
    });

    expect(result.current.a.data).toBeDefined();
    expect(result.current.b.data).toBeDefined();
    // Both consumers share the same cache entry — same array reference.
    expect(result.current.a.data).toBe(result.current.b.data);
  });
});

// ============================================================================
// useObject
// ============================================================================

describe("useObject", () => {
  it("returns the object summary on success", async () => {
    const wrapper = withSWR();
    const { result } = renderHook(
      () => useObject(inspectableObjectFixture.id),
      { wrapper },
    );

    await waitFor(() => {
      expect(result.current.data).toBeDefined();
    });

    expect(result.current.data!.label).toBe(inspectableObjectFixture.label);
  });

  it("error → retry: SWR surfaces the ApiError and retries", async () => {
    // First call returns 404, second call returns the fixture.
    let calls = 0;
    server.use(
      http.get("/api/objects/:object_id", () => {
        calls += 1;
        if (calls === 1) {
          return HttpResponse.json(
            { error: "Object not found: ghost" },
            { status: 404 },
          );
        }
        return HttpResponse.json(inspectableObjectFixture);
      }),
    );

    const wrapper = withSWR();
    const { result } = renderHook(
      () => useObject(inspectableObjectFixture.id, ),
      { wrapper },
    );

    // First attempt fails.
    await waitFor(() => {
      expect(result.current.error).toBeDefined();
    });
    expect(result.current.error!.status).toBe(404);

    // Trigger a retry — SWR exposes `mutate` from the result.
    await act(async () => {
      await result.current.mutate();
    });

    await waitFor(() => {
      expect(result.current.data).toBeDefined();
    });
    expect(result.current.data!.id).toBe(inspectableObjectFixture.id);
    expect(calls).toBeGreaterThanOrEqual(2);
  });
});

// ============================================================================
// useViews + useAvailableViews
// ============================================================================

describe("useAvailableViews", () => {
  it("returns the list of view descriptors", async () => {
    const wrapper = withSWR();
    const { result } = renderHook(
      () => useAvailableViews(inspectableObjectFixture.id),
      { wrapper },
    );

    // Wait for data to have actual items (not just defined as empty array from initial merge)
    await waitFor(() => {
      expect(result.current.data).toBeDefined();
      expect(result.current.data!.length).toBeGreaterThan(0);
    });
    expect(result.current.data!.length).toBe(
      inspectableObjectFixture.available_views.length,
    );
  });
});

describe("useViews", () => {
  it("returns the contextual view", async () => {
    const wrapper = withSWR();
    const { result } = renderHook(
      () => useViews(inspectableObjectFixture.id, "overview"),
      { wrapper },
    );

    await waitFor(() => {
      expect(result.current.data).toBeDefined();
    });
    expect(result.current.data!.blocks.length).toBe(
      contextualViewFixture.blocks.length,
    );
  });

  it("skips the fetch when viewId is null", async () => {
    const wrapper = withSWR();
    const { result } = renderHook(
      () => useViews(inspectableObjectFixture.id, null),
      { wrapper },
    );

    await new Promise((r) => setTimeout(r, 25));
    expect(result.current.data).toBeUndefined();
    expect(result.current.isLoading).toBe(false);
  });
});

// ============================================================================
// useLenses + useLensResult
// ============================================================================

describe("useLenses", () => {
  it("returns the lens descriptors", async () => {
    const wrapper = withSWR();
    const { result } = renderHook(
      () => useLenses(inspectableObjectFixture.id),
      { wrapper },
    );

    await waitFor(() => {
      expect(result.current.data).toBeDefined();
    });
    expect(result.current.data!.length).toBe(lensDescriptorsFixture.length);
  });
});

describe("useLensResult", () => {
  it("returns the lens result with a target query param", async () => {
    let sawTarget: string | null = null;
    server.use(
      http.get(
        "/api/objects/:object_id/lenses/:lens_id/apply",
        ({ request }) => {
          const url = new URL(request.url);
          sawTarget = url.searchParams.get("target");
          return HttpResponse.json(lensResultFixture);
        },
      ),
    );

    const wrapper = withSWR();
    const { result } = renderHook(
      () =>
        useLensResult({
          objectId: inspectableObjectFixture.id,
          lensId: "lens.hotspots",
          target: "scope:src",
        }),
      { wrapper },
    );

    await waitFor(() => {
      expect(result.current.data).toBeDefined();
    });
    expect(result.current.data!.lens_id).toBe("lens.hotspots");
    expect(sawTarget).toBe("scope:src");
  });
});

// ============================================================================
// useExplorations + helpers
// ============================================================================

describe("useExplorations", () => {
  it("returns an empty list when no explorations are saved", async () => {
    const wrapper = withSWR();
    server.use(
      http.get("/api/workspaces/:workspace_id/explorations", () => {
        return HttpResponse.json([]);
      }),
    );
    const { result } = renderHook(
      () => useExplorations("ws-empty"),
      { wrapper },
    );

    await waitFor(() => {
      expect(result.current.data).toBeDefined();
    });
    expect(result.current.data).toEqual([]);
  });
});

describe("generateArtifact", () => {
  it("posts the format and returns the artifact summary", async () => {
    server.use(
      http.post(
        "/api/explorations/:exploration_id/artifacts",
        async ({ request }) => {
          artifactFetchCount += 1;
          const body = (await request.json()) as { format: string };
          return HttpResponse.json({
            ...decisionArtifactFixture,
            format: body.format,
          });
        },
      ),
    );

    const result = await generateArtifact("exploration-1", "markdown");
    expect(result.format).toBe("markdown");
    expect(artifactFetchCount).toBe(1);
  });
});

// ============================================================================
// Cache + dedup verification (cross-hook)
// ============================================================================

describe("SWR cache + dedup", () => {
  it("two useSpotter calls with the same args share the same request", async () => {
    server.use(
      http.get("/api/workspaces/:workspace_id/spotter", () => {
        spotterFetchCount += 1;
        return HttpResponse.json(spotterResultsFixture);
      }),
    );

    const wrapper = withSWR();
    const { result } = renderHook(
      () => {
        const a = useSpotter({ workspaceId: "ws-dedup", q: "alpha" });
        const b = useSpotter({ workspaceId: "ws-dedup", q: "alpha" });
        return { a, b };
      },
      { wrapper },
    );

    await waitFor(() => {
      expect(result.current.a.data).toBeDefined();
      expect(result.current.b.data).toBeDefined();
    });

    // SWR's cache returns the same data object to both consumers.
    expect(result.current.a.data).toBe(result.current.b.data);
    // Spotter only fired once across the two consumers.
    expect(spotterFetchCount).toBe(1);
  });
});
