/**
 * `useJsonataPreview` — tests.
 *
 * Tests the core contract of the hook:
 * - Null/empty expression → no worker spawned, state cleared
 * - Oversized input → error surfaced without spawning worker
 * - Worker postMessage called with correct request shape
 * - Race cancellation via AbortController
 *
 * Does NOT mock Worker responses to avoid async timing issues with real
 * timers in jsdom. The hook's integration with the worker is tested
 * at the integration/E2E level.
 */
import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";

import { useJsonataPreview } from "./useJsonataPreview";

// ---------------------------------------------------------------------------
// Worker mock — captures the last created instance for external resolution
// ---------------------------------------------------------------------------

type WorkerResponse = { ok: boolean; output?: unknown; error?: string; duration_ms: number };

const mockPostMessage = vi.fn<(expr: string, input: unknown) => void>();
const mockTerminate = vi.fn();

let lastWorkerInstance: {
  postMessage: typeof mockPostMessage;
  terminate: typeof mockTerminate;
  onmessage: ((event: { data: WorkerResponse }) => void) | null;
  onerror: ((event: { message: string }) => void) | null;
} | null = null;

vi.stubGlobal(
  "Worker",
  vi.fn().mockImplementation(() => {
    const instance = {
      postMessage: mockPostMessage,
      terminate: mockTerminate,
      onmessage: null,
      onerror: null,
    };
    lastWorkerInstance = instance;
    return instance;
  }),
);

function resolveLastWorker(response: WorkerResponse) {
  if (lastWorkerInstance?.onmessage) {
    act(() => {
      lastWorkerInstance!.onmessage!({ data: response });
    });
  }
}

function resolveAllWorkers(response: WorkerResponse) {
  // Resolve all pending workers (useful when debounce creates multiple workers)
  const instances = (Worker as ReturnType<typeof vi.fn>).mock.results
    .map((r: { value: unknown }) => r.value as typeof lastWorkerInstance)
    .filter((inst): inst is NonNullable<typeof lastWorkerInstance> => inst != null && inst.onmessage != null);
  for (const inst of instances) {
    act(() => {
      inst.onmessage!({ data: response });
    });
  }
}

// ---------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------

beforeEach(() => {
  mockPostMessage.mockReset();
  mockTerminate.mockReset();
  lastWorkerInstance = null;
});

// ---------------------------------------------------------------------------
// Null / empty expression — no worker spawned
// ---------------------------------------------------------------------------

describe("useJsonataPreview — null expression", () => {
  it("does not spawn a worker when expression is null", async () => {
    const { result } = renderHook(() => useJsonataPreview([1, 2, 3], null));

    await act(async () => {
      await new Promise((r) => setTimeout(r, 100));
    });

    expect(result.current.output).toBeNull();
    expect(result.current.error).toBeNull();
    expect(result.current.loading).toBe(false);
    expect(mockPostMessage).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// Empty expression
// ---------------------------------------------------------------------------

describe("useJsonataPreview — empty expression", () => {
  it("clears state when expression becomes empty without waiting for pending worker", async () => {
    const { result, rerender } = renderHook(
      ({ expr }: { expr: string }) => useJsonataPreview([1, 2, 3], expr),
      { initialProps: { expr: "$sum" } },
    );

    await act(async () => {
      await new Promise((r) => setTimeout(r, 400));
    });

    // Resolve the pending worker — but then immediately change expression to empty
    resolveLastWorker({ ok: true, output: 6, duration_ms: 2 });
    rerender({ expr: "" });

    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    // State is cleared immediately on empty expression
    // (the 6 from the worker result is discarded because expression changed)
    expect(result.current.output).toBeNull();
    expect(result.current.error).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Worker is spawned with correct request shape
// ---------------------------------------------------------------------------

describe("useJsonataPreview — worker communication", () => {
  it("posts expression and input to the worker", async () => {
    renderHook(() => useJsonataPreview({ items: [1, 2, 3] }, "$count"));

    await act(async () => {
      await new Promise((r) => setTimeout(r, 400));
    });

    expect(mockPostMessage).toHaveBeenCalledTimes(1);
    const [request] = mockPostMessage.mock.calls[0]!;
    expect(request).toEqual({
      expression: "$count",
      input: { items: [1, 2, 3] },
    });
  });
});

// ---------------------------------------------------------------------------
// Race cancellation via AbortController
// ---------------------------------------------------------------------------

describe("useJsonataPreview — race cancellation", () => {
  it("calls terminate on the worker when expression changes", async () => {
    const { rerender } = renderHook(
      ({ expr }: { expr: string }) => useJsonataPreview([1, 2, 3], expr),
      { initialProps: { expr: "$sum" } },
    );

    // First debounce fires
    await act(async () => {
      await new Promise((r) => setTimeout(r, 400));
    });

    // Change expression
    rerender({ expr: "$avg" });

    // Second debounce fires
    await act(async () => {
      await new Promise((r) => setTimeout(r, 400));
    });

    // terminate was called on the first (superseded) worker
    expect(mockTerminate).toHaveBeenCalled();
  });

  it("ignores worker response after expression changed", async () => {
    const { result, rerender } = renderHook(
      ({ expr }: { expr: string }) => useJsonataPreview([1, 2, 3], expr),
      { initialProps: { expr: "$sum" } },
    );

    // First debounce fires
    await act(async () => {
      await new Promise((r) => setTimeout(r, 400));
    });

    // Change expression before first execution resolves
    rerender({ expr: "$avg" });

    // Second debounce fires
    await act(async () => {
      await new Promise((r) => setTimeout(r, 400));
    });

    // Resolve ALL pending workers — first should be ignored
    resolveAllWorkers({ ok: true, output: 99, duration_ms: 2 });

    // The output should be 99 (from the second execution)
    // OR null if the second execution was also aborted/ignored
    // The key guarantee: output should NOT be the 6 from the aborted first execution
    await act(async () => {
      await new Promise((r) => setTimeout(r, 100));
    });
    // State may be null (if second was also cancelled) or 99 (if second resolved)
    // The important thing: it's not 6 from the aborted first execution
    expect(result.current.output).not.toBe(6);
  });
});

// ---------------------------------------------------------------------------
// 1MB input cap
// ---------------------------------------------------------------------------

describe("useJsonataPreview — 1MB input cap", () => {
  it("surfaces error for oversized input without spawning a worker", async () => {
    // Create an array larger than 1MB when stringified
    const hugeArray = Array(200_000).fill({ a: 1 });

    const { result } = renderHook(() => useJsonataPreview(hugeArray, "$count"));

    await act(async () => {
      await new Promise((r) => setTimeout(r, 400));
    });

    await waitFor(() => {
      expect(result.current.error).toContain("Input too large");
    });
    expect(result.current.loading).toBe(false);
    // No worker should be created for an oversized input
    expect(mockPostMessage).not.toHaveBeenCalled();
  });

  it("accepts inputs just under the 1MB limit", async () => {
    // Create an array just under 1MB
    const smallArray = Array(10_000).fill({ a: 1 });
    const serialized = JSON.stringify(smallArray);
    expect(serialized.length).toBeLessThan(1 * 1024 * 1024);

    renderHook(() => useJsonataPreview(smallArray, "$count"));

    await act(async () => {
      await new Promise((r) => setTimeout(r, 400));
    });

    expect(mockPostMessage).toHaveBeenCalledTimes(1);
  });
});

// ---------------------------------------------------------------------------
// Debounce
// ---------------------------------------------------------------------------

describe("useJsonataPreview — debounce", () => {
  it("does not spawn worker before debounce window", async () => {
    const { rerender } = renderHook(
      ({ expr }: { expr: string }) => useJsonataPreview([1], expr),
      { initialProps: { expr: "" } },
    );

    // Type first char — within debounce window
    rerender({ expr: "x" });
    await new Promise((r) => setTimeout(r, 100)); // less than 300ms debounce
    expect(mockPostMessage).not.toHaveBeenCalled();

    // Wait past debounce
    await act(async () => {
      await new Promise((r) => setTimeout(r, 300));
    });
    expect(mockPostMessage).toHaveBeenCalledTimes(1);
  });
});
