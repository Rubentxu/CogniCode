/**
 * Integration tests for `fetchSnapshot` client function.
 *
 * Tests the REST endpoint integration:
 * - Happy path: PNG response
 * - Happy path: SVG response
 * - Error: 400 invalid format
 * - Error: 503 mmdc not found (feature disabled)
 *
 * Uses MSW to intercept fetch and return mock responses.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { http, HttpResponse } from "msw";
import { server } from "../mocks/node";

import { fetchSnapshot } from "./client";

describe("fetchSnapshot", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    server.resetHandlers();
  });

  afterEach(() => {
    server.resetHandlers();
  });

  it("returns a Blob for PNG format", async () => {
    const pngBytes = new Uint8Array([0x89, 0x50, 0x4e, 0x47]); // PNG magic bytes

    server.use(
      http.get("/api/workspaces/:workspaceId/snapshot", () => {
        return new HttpResponse(pngBytes, {
          status: 200,
          headers: {
            "Content-Type": "image/png",
            "Content-Disposition": 'attachment; filename="c4_context.png"',
          },
        });
      }),
    );

    const result = await fetchSnapshot("test-workspace", "c4_context", "png", ".");
    expect(result).toBeInstanceOf(Blob);
    expect(result.type).toBe("image/png");
  });

  it("returns a Blob for SVG format", async () => {
    const svgContent = '<svg xmlns="http://www.w3.org/2000/svg"><text>Test</text></svg>';
    const svgBytes = new TextEncoder().encode(svgContent);

    server.use(
      http.get("/api/workspaces/:workspaceId/snapshot", () => {
        return new HttpResponse(svgBytes, {
          status: 200,
          headers: {
            "Content-Type": "image/svg+xml",
            "Content-Disposition": 'attachment; filename="c4_context.svg"',
          },
        });
      }),
    );

    const result = await fetchSnapshot("test-workspace", "c4_context", "svg", ".");
    expect(result).toBeInstanceOf(Blob);
    expect(result.type).toBe("image/svg+xml");
  });

  it("passes correct query params to the endpoint", async () => {
    let capturedUrl: string | null = null;
    const pngBytes = new Uint8Array([0x89, 0x50, 0x4e, 0x47]);

    server.use(
      http.get("/api/workspaces/:workspaceId/snapshot", ({ request }) => {
        capturedUrl = request.url;
        return new HttpResponse(pngBytes, {
          status: 200,
          headers: { "Content-Type": "image/png" },
        });
      }),
    );

    await fetchSnapshot("my-workspace", "call_graph", "png", "sym:test::func");

    expect(capturedUrl).not.toBeNull();
    const url = new URL(capturedUrl!);
    expect(url.searchParams.get("view_kind")).toBe("call_graph");
    expect(url.searchParams.get("format")).toBe("png");
    expect(url.searchParams.get("target")).toBe("sym:test::func");
  });

  it("omits target param when not provided", async () => {
    let capturedUrl: string | null = null;
    const pngBytes = new Uint8Array([0x89, 0x50, 0x4e, 0x47]);

    server.use(
      http.get("/api/workspaces/:workspaceId/snapshot", ({ request }) => {
        capturedUrl = request.url;
        return new HttpResponse(pngBytes, {
          status: 200,
          headers: { "Content-Type": "image/png" },
        });
      }),
    );

    await fetchSnapshot("my-workspace", "c4_context", "png");

    expect(capturedUrl).not.toBeNull();
    const url = new URL(capturedUrl!);
    expect(url.searchParams.has("target")).toBe(false);
  });

  it("throws ApiError with status 400 for invalid format", async () => {
    server.use(
      http.get("/api/workspaces/:workspaceId/snapshot", () => {
        return HttpResponse.json(
          { error: "invalid format: pdf (expected: png, svg)" },
          { status: 400 },
        );
      }),
    );

    await expect(
      fetchSnapshot("test-workspace", "c4_context", "png"),
    ).rejects.toMatchObject({
      name: "ApiError",
      status: 400,
    });
  });

  it("throws ApiError with status 503 when mmdc is not found", async () => {
    server.use(
      http.get("/api/workspaces/:workspaceId/snapshot", () => {
        return HttpResponse.json(
          { error: "snapshot feature not available (mmdc not configured)" },
          { status: 503 },
        );
      }),
    );

    await expect(
      fetchSnapshot("test-workspace", "c4_context", "png"),
    ).rejects.toMatchObject({
      name: "ApiError",
      status: 503,
      detail: "snapshot feature not available (mmdc not configured)",
    });
  });

  it("throws ApiError with status 400 for unsupported view_kind", async () => {
    server.use(
      http.get("/api/workspaces/:workspaceId/snapshot", () => {
        return HttpResponse.json(
          { error: "invalid view_kind: unknown_kind (expected: c4_context, c4_container, c4_component, call_graph, impact_radius, vertical_slice)" },
          { status: 400 },
        );
      }),
    );

    await expect(
      fetchSnapshot("test-workspace", "unknown_kind", "png"),
    ).rejects.toMatchObject({
      name: "ApiError",
      status: 400,
    });
  });
});
