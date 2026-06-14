/**
 * `useWizardDraft` — tests.
 *
 * Tests:
 * - Draft is loaded from localStorage when hook mounts
 * - Draft is null when no draft exists for the object
 * - save() persists to localStorage after debounce
 * - clear() removes the draft from localStorage
 * - LRU eviction when cap is exceeded
 * - Draft includes editSpecId when editing
 */
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useWizardDraft } from "./useWizardDraft";

// ---------------------------------------------------------------------------
// localStorage mock
// ---------------------------------------------------------------------------

const store: Map<string, string> = new Map();

const mockLocalStorage = {
  getItem: vi.fn((key: string) => store.get(key) ?? null),
  setItem: vi.fn((key: string, value: string) => store.set(key, value)),
  removeItem: vi.fn((key: string) => store.delete(key)),
  get length() { return store.size; },
  key: vi.fn((i: number) => Array.from(store.keys())[i] ?? null),
  clear: vi.fn(() => store.clear()),
};

Object.defineProperty(globalThis, "localStorage", {
  value: mockLocalStorage,
  writable: true,
});

// ---------------------------------------------------------------------------
// Setup / teardown
// ---------------------------------------------------------------------------

beforeEach(() => {
  store.clear();
  mockLocalStorage.getItem.mockClear();
  mockLocalStorage.setItem.mockClear();
  mockLocalStorage.removeItem.mockClear();
});

afterEach(() => {
  vi.restoreAllMocks();
});

// ---------------------------------------------------------------------------
// Helper — advance fake timers and flush pending microtasks
// ---------------------------------------------------------------------------

async function flushTimers(ms: number) {
  await act(async () => {
    await new Promise((r) => setTimeout(r, ms));
  });
}

// ---------------------------------------------------------------------------
// Draft not present
// ---------------------------------------------------------------------------

describe("useWizardDraft — no existing draft", () => {
  it("returns draft=null and hasDraft=false when no draft exists", async () => {
    const { result } = renderHook(() =>
      useWizardDraft({
        objectId: "obj-no-draft",
        onRestore: vi.fn(),
      }),
    );

    await flushTimers(0);

    expect(result.current.draft).toBeNull();
    expect(result.current.hasDraft).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Draft is loaded on mount
// ---------------------------------------------------------------------------

describe("useWizardDraft — draft loading", () => {
  it("loads a persisted draft on mount and calls onRestore", async () => {
    const onRestore = vi.fn();
    const savedAt = new Date().toISOString();
    const persistedDraft = {
      objectId: "obj-123",
      state: {
        viewKind: "call_graph",
        rendererKind: "graph",
        query: "symbols",
        transformKind: "none",
        jsonataExpression: "",
        title: "My custom view",
      },
      savedAt,
    };
    store.set("viewspec-draft-obj-123", JSON.stringify(persistedDraft));

    const { result } = renderHook(() =>
      useWizardDraft({
        objectId: "obj-123",
        onRestore,
      }),
    );

    await flushTimers(0);

    expect(result.current.hasDraft).toBe(true);
    expect(result.current.draft).toEqual(persistedDraft);
    expect(onRestore).toHaveBeenCalledWith(persistedDraft.state);
  });

  it("ignores corrupt localStorage entries", async () => {
    store.set("viewspec-draft-obj-bad", "not valid json");

    const { result } = renderHook(() =>
      useWizardDraft({
        objectId: "obj-bad",
        onRestore: vi.fn(),
      }),
    );

    await flushTimers(0);

    expect(result.current.hasDraft).toBe(false);
    expect(result.current.draft).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// Draft save (debounced 1s)
// ---------------------------------------------------------------------------

describe("useWizardDraft — draft save", () => {
  it("does not persist before debounce window", async () => {
    const { result } = renderHook(() =>
      useWizardDraft({
        objectId: "obj-save",
        onRestore: vi.fn(),
      }),
    );

    await flushTimers(0);

    result.current.save({
      viewKind: "vertical_slice",
      rendererKind: "graph",
      query: "symbols",
      transformKind: "none",
      jsonataExpression: "",
      title: "Test view",
    });

    await flushTimers(500); // less than 1s debounce

    expect(mockLocalStorage.setItem).not.toHaveBeenCalled();
    expect(result.current.hasDraft).toBe(false);
  });

  it("persists draft to localStorage after 1s debounce", async () => {
    const { result } = renderHook(() =>
      useWizardDraft({
        objectId: "obj-save-2",
        onRestore: vi.fn(),
      }),
    );

    await flushTimers(0);

    result.current.save({
      viewKind: "vertical_slice",
      rendererKind: "graph",
      query: "symbols",
      transformKind: "none",
      jsonataExpression: "",
      title: "Test view",
    });

    await flushTimers(1100); // past debounce

    expect(mockLocalStorage.setItem).toHaveBeenCalledTimes(1);
    const [, value] = mockLocalStorage.setItem.mock.calls[0]!;
    const parsed = JSON.parse(value as string);
    expect(parsed.objectId).toBe("obj-save-2");
    expect(parsed.state.title).toBe("Test view");
    expect(parsed.state.viewKind).toBe("vertical_slice");
  });

  it("includes editSpecId when editing an existing spec", async () => {
    const editSpec = { id: "spec-abc", title: "Old title" } as any;
    const { result } = renderHook(() =>
      useWizardDraft({
        objectId: "obj-edit",
        editSpec,
        onRestore: vi.fn(),
      }),
    );

    await flushTimers(0);

    result.current.save({
      viewKind: "call_graph",
      rendererKind: "graph",
      query: "symbols",
      transformKind: "none",
      jsonataExpression: "",
      title: "Updated title",
    });

    await flushTimers(1100);

    const [, value] = mockLocalStorage.setItem.mock.calls[0]!;
    const parsed = JSON.parse(value as string);
    expect(parsed.editSpecId).toBe("spec-abc");
  });
});

// ---------------------------------------------------------------------------
// Draft clear
// ---------------------------------------------------------------------------

describe("useWizardDraft — draft clear", () => {
  it("removes the draft from localStorage on clear()", async () => {
    // Pre-populate a draft
    const savedAt = new Date().toISOString();
    store.set("viewspec-draft-obj-clear", JSON.stringify({
      objectId: "obj-clear",
      state: { viewKind: null, rendererKind: null, query: "", transformKind: "none", jsonataExpression: "", title: "" },
      savedAt,
    }));

    const { result } = renderHook(() =>
      useWizardDraft({
        objectId: "obj-clear",
        onRestore: vi.fn(),
      }),
    );

    await flushTimers(0);
    expect(result.current.hasDraft).toBe(true);

    act(() => {
      result.current.clear();
    });

    expect(mockLocalStorage.removeItem).toHaveBeenCalledWith("viewspec-draft-obj-clear");
    expect(result.current.hasDraft).toBe(false);
    expect(result.current.draft).toBeNull();
  });

  it("clear cancels pending debounced saves", async () => {
    const { result } = renderHook(() =>
      useWizardDraft({
        objectId: "obj-clear-pending",
        onRestore: vi.fn(),
      }),
    );

    await flushTimers(0);

    result.current.save({
      viewKind: "vertical_slice",
      rendererKind: "graph",
      query: "symbols",
      transformKind: "none",
      jsonataExpression: "",
      title: "Test",
    });

    // Clear before debounce fires
    act(() => {
      result.current.clear();
    });

    await flushTimers(1100);

    // No setItem because the save was cancelled
    expect(mockLocalStorage.setItem).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// LRU cap — 20 drafts
// ---------------------------------------------------------------------------

describe("useWizardDraft — LRU cap", () => {
  it("evicts oldest drafts when cap is exceeded", async () => {
    // Write 20 drafts (the cap)
    // Higher index = earlier timestamp = older (obj-19 is oldest, obj-0 is newest)
    for (let i = 0; i < 20; i++) {
      store.set(`viewspec-draft-obj-${i}`, JSON.stringify({
        objectId: `obj-${i}`,
        state: { viewKind: null, rendererKind: null, query: "", transformKind: "none", jsonataExpression: "", title: "" },
        savedAt: new Date(Date.now() - i * 1000).toISOString(),
      }));
    }

    // Now save a new draft — should evict the oldest (obj-19, lowest timestamp)
    const { result } = renderHook(() =>
      useWizardDraft({
        objectId: "obj-new",
        onRestore: vi.fn(),
      }),
    );

    await flushTimers(0);

    result.current.save({
      viewKind: "vertical_slice",
      rendererKind: "graph",
      query: "symbols",
      transformKind: "none",
      jsonataExpression: "",
      title: "New view",
    });

    await flushTimers(1100);

    // obj-19 should be evicted (it was the oldest — lowest timestamp)
    expect(store.has("viewspec-draft-obj-19")).toBe(false);
    // obj-0 through obj-18 should still exist (newer drafts)
    for (let i = 0; i < 19; i++) {
      expect(store.has(`viewspec-draft-obj-${i}`)).toBe(true);
    }
    // obj-new should exist
    expect(store.has("viewspec-draft-obj-new")).toBe(true);
  });
});
