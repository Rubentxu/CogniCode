/**
 * Tests for useGraphAlgorithms hook.
 *
 * These tests verify the hook's behavior without actually loading
 * the WASM module (which would require a build artifact).
 *
 * The WASM feature is opt-in (VITE_ENABLE_WASM must be 'true'), so in
 * the default vitest environment pagerank/godNodes throw clear errors.
 */

import { describe, expect, it, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useGraphAlgorithms } from "../useGraphAlgorithms";

describe("useGraphAlgorithms", () => {
  it("reports enabled=false when VITE_ENABLE_WASM is not 'true'", () => {
    // vitest env defaults to undefined for import.meta.env
    const { result } = renderHook(() => useGraphAlgorithms());
    expect(result.current.enabled).toBe(false);
    expect(result.current.state).toBe("idle");
  });

  it("throws a clear error when pagerank is called with WASM disabled", async () => {
    const { result } = renderHook(() => useGraphAlgorithms());
    await expect(
      result.current.pagerank([{ id: "A" }], [{ source: "A", target: "B" }])
    ).rejects.toThrow(/WASM.*disabled.*backend endpoint/);
  });

  it("throws a clear error when godNodes is called with WASM disabled", async () => {
    const { result } = renderHook(() => useGraphAlgorithms());
    await expect(
      result.current.godNodes([{ id: "A" }], [{ source: "A", target: "B" }])
    ).rejects.toThrow(/WASM.*disabled.*backend endpoint/);
  });

  it("handles WASM load errors gracefully when WASM is enabled", async () => {
    // Mock VITE_ENABLE_WASM=true and a failing dynamic import.
    vi.stubEnv("VITE_ENABLE_WASM", "true");
    // The dynamic import would fail in jsdom; the hook should catch it
    // and expose error state without unhandled rejection.
    const { result } = renderHook(() => useGraphAlgorithms());
    await act(async () => {
      // Allow the useEffect-triggered load to complete (or fail).
      await new Promise((r) => setTimeout(r, 100));
    });
    // Either state === 'error' (load failed in jsdom) or state === 'ready'
    // (somehow worked — either way no unhandled rejection).
    expect(["error", "ready"]).toContain(result.current.state);
    vi.unstubAllEnvs();
  });

  it("state is 'idle' and error is null when WASM is disabled", () => {
    const { result } = renderHook(() => useGraphAlgorithms());
    expect(result.current.state).toBe("idle");
    expect(result.current.error).toBeNull();
  });
});
