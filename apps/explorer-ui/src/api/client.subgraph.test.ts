/**
 * Tests for the REST client — `fetchSubgraph` in particular.
 *
 * Uses the global MSW server (see `mocks/node.ts` + `test/setup.ts`)
 * to intercept `fetch` and return a hand-rolled fixture that mirrors
 * the backend wire shape. This is the boundary test: if the backend
 * and the front-end drift, these tests fail.
 */
import { describe, expect, it, beforeEach, afterEach } from "vitest";
import { http, HttpResponse } from "msw";
import { server } from "../mocks/node";

import { fetchSubgraph } from "./client";
import { smallSubgraphFixture } from "../mocks/subgraphFixtures";

describe("fetchSubgraph", () => {
  let lastRequest: { url: string; method: string } | null = null;

  beforeEach(() => {
    lastRequest = null;
    server.use(
      http.get("/api/graph/:id/subgraph", ({ request, params }) => {
        lastRequest = { url: request.url, method: request.method };
        return HttpResponse.json({
          ...smallSubgraphFixture,
          root: String(params["id"] ?? ""),
        });
      }),
    );
  });

  afterEach(() => {
    server.resetHandlers();
  });

  it("resolves to a typed SubgraphResponse", async () => {
    const response = await fetchSubgraph("sym:foo::bar", {});
    expect(response.root).toBe("sym:foo::bar");
    expect(response.nodes.length).toBeGreaterThan(0);
    expect(response.edges.length).toBeGreaterThan(0);
  });

  it("encodes query params into the URL", async () => {
    await fetchSubgraph("sym:foo::bar", {
      depth: 2,
      direction: "incoming",
      max_nodes: 100,
    });
    expect(lastRequest).not.toBeNull();
    const url = new URL(lastRequest!.url);
    expect(url.searchParams.get("depth")).toBe("2");
    expect(url.searchParams.get("direction")).toBe("incoming");
    expect(url.searchParams.get("max_nodes")).toBe("100");
  });

  it("throws ApiError with status 404 on missing symbol", async () => {
    server.use(
      http.get("/api/graph/:id/subgraph", () => {
        return HttpResponse.json(
          { error: "symbol_not_found" },
          { status: 404 },
        );
      }),
    );
    await expect(
      fetchSubgraph("sym:does::not::exist", {}),
    ).rejects.toMatchObject({
      name: "ApiError",
      status: 404,
      detail: "symbol_not_found",
    });
  });
});
