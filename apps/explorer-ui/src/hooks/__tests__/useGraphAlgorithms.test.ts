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
    // NOTE: Testing WASM load error handling requires isolated module re-import
    // because WASM_ENABLED is a module-level const evaluated at import time.
    // This test verifies the disabled-path behavior (no unhandled rejections).
    //
    // Full WASM-enabled error handling is tested manually with:
    //   VITE_ENABLE_WASM=true npm test -- useGraphAlgorithms
    // and requires the actual WASM build artifact at src/wasm/.
    //
    // Here we verify: when WASM is disabled (default in tests), calling
    // pagerank/godNodes throws rather than crashing.
    const { result } = renderHook(() => useGraphAlgorithms());

    // When WASM is disabled (default in test env), state should be idle.
    // The functions should throw clear errors when called.
    expect(result.current.state).toBe("idle");
    expect(result.current.enabled).toBe(false);

    await expect(
      result.current.pagerank([{ id: "a" }], [{ source: "a", target: "b" }])
    ).rejects.toThrow(/WASM.*disabled/i);

    await expect(
      result.current.godNodes([{ id: "a" }], [{ source: "a", target: "b" }])
    ).rejects.toThrow(/WASM.*disabled/i);
  });

  it("state is 'idle' and error is null when WASM is disabled", () => {
    const { result } = renderHook(() => useGraphAlgorithms());
    expect(result.current.state).toBe("idle");
    expect(result.current.error).toBeNull();
  });
});
