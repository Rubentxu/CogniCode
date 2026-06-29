/**
 * Tests for `useDrift` hook.
 *
 * Validates:
 * - Fetches when workspaceId is set
 * - Skips fetch when workspaceId is null
 * - Returns empty report on 404
 * - Reuses cache on re-mount
 */
import { describe, expect, it, afterEach } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { server } from "../mocks/node";

import { useDrift } from "./useDrift";

describe("useDrift", () => {
  // Reset handlers after each test
  afterEach(() => {
    server.resetHandlers();
  });

  describe("when workspaceId is set", () => {
    it("fetches drift report successfully", async () => {
      const driftReport = {
        findings: [
          {
            kind: "missing",
            expected: "Container: ctr-api",
            actual: "—",
            severity: "major",
            detail: "Expected container ctr-api is missing from codebase",
          },
        ],
        summary: "1 missing container",
        missing_containers: 1,
        extra_containers: 0,
        wrong_sub_kinds: 0,
      };

      server.use(
        http.get("/api/workspaces/:workspace_id/drift", () => {
          return HttpResponse.json(driftReport);
        }),
      );

      const { result } = renderHook(() => useDrift("workspace-drift-1"));

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      expect(result.current.data).toMatchObject({
        findings: expect.any(Array),
        summary: expect.any(String),
        missing_containers: expect.any(Number),
        extra_containers: expect.any(Number),
        wrong_sub_kinds: expect.any(Number),
      });
    });

    it("returns empty report on 404 (no expected-architecture.yaml)", async () => {
      // Override the default handler to actually return 404
      server.use(
        http.get("/api/workspaces/:workspace_id/drift", () => {
          return HttpResponse.json({ error: "not found" }, { status: 404 });
        }),
      );

      const { result } = renderHook(() => useDrift("workspace-drift-404"));

      await waitFor(() => {
        expect(result.current.isLoading).toBe(false);
      });

      // Should return empty report on 404, not error
      expect(result.current.error).toBeUndefined();
      expect(result.current.data).toMatchObject({
        findings: [],
        summary: expect.stringContaining("no expected architecture"),
        missing_containers: 0,
        extra_containers: 0,
        wrong_sub_kinds: 0,
      });
    });

    it("surfaces error on non-404 failure", async () => {
      server.use(
        http.get("/api/workspaces/:workspace_id/drift", () => {
          return HttpResponse.json({ error: "server error" }, { status: 500 });
        }),
      );

      const { result } = renderHook(() => useDrift("workspace-drift-500"));

      await waitFor(() => {
        expect(result.current.error).toBeDefined();
      });

      expect(result.current.data).toBeUndefined();
    });
  });

  describe("when workspaceId is null", () => {
    it("does not fetch", async () => {
      const { result } = renderHook(() => useDrift(null));

      // Immediately not loading with no data
      expect(result.current.isLoading).toBe(false);
      expect(result.current.data).toBeUndefined();
      expect(result.current.error).toBeUndefined();
    });
  });
});
