/**
 * Tests for the `graphSearch` REST client — T22.
 *
 * Uses the global MSW server to intercept the POST to
 * `/api/mcp/tools/call` and return a hand-rolled fixture that
 * mirrors the `McpResultEnvelope` from the Rust side. The
 * fixture is parsed at the boundary via the
 * `graphSearchResponseSchema` (Zod) and the typed result is
 * asserted.
 */
import { describe, expect, it, beforeEach, afterEach } from "vitest";
import { http, HttpResponse } from "msw";
import { server } from "../mocks/node";

import { graphSearch } from "./client";

describe("graphSearch", () => {
  let lastRequest: {
    url: string;
    method: string;
    body: unknown;
  } | null = null;

  beforeEach(() => {
    lastRequest = null;
    server.use(
      http.post("/api/mcp/tools/call", async ({ request }) => {
        const body = (await request.clone().json()) as {
          name?: string;
          args?: Record<string, unknown>;
        };
        lastRequest = {
          url: request.url,
          method: request.method,
          body,
        };
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
            total_count: 2,
            next_cursor: null,
            raw_rank: 1.0,
            normalized_score: 1.0,
          },
          suggested_follow_ups: [],
        });
      }),
    );
  });

  afterEach(() => {
    server.resetHandlers();
  });

  it("client_graphSearch_returns_typed_response", async () => {
    const response = await graphSearch({ query: "ADR" });
    expect(response.total_count).toBe(2);
    expect(response.results).toHaveLength(2);
    expect(response.results[0]?.node.kind).toBe("decision");
    expect(response.next_cursor).toBeNull();
  });

  it("client_graphSearch_pagination passes cursor", async () => {
    const response = await graphSearch({ query: "ADR", cursor: "1" });
    expect(response).toBeDefined();
    expect(lastRequest).not.toBeNull();
    expect(lastRequest!.body).toMatchObject({
      name: "graph_search",
      args: { query: "ADR", cursor: "1" },
    });
  });

  it("client_graphSearch_passes_kinds_and_limit", async () => {
    await graphSearch({
      query: "schema",
      node_kinds: ["decision", "doc"],
      limit: 25,
    });
    expect(lastRequest).not.toBeNull();
    const body = lastRequest!.body as { args: Record<string, unknown> };
    expect(body.args["node_kinds"]).toEqual(["decision", "doc"]);
    expect(body.args["limit"]).toBe(25);
  });
});
