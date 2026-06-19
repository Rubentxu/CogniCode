/**
 * Reducer tests for pane-stack navigation.
 *
 * Verifies navigation state management. Other reducer behaviour is
 * exercised by the Spotter / ObjectInspector / LensPanel tests
 * (the full reducer is wired in those harnesses).
 */
import { describe, it, expect } from "vitest";
import { appReducer, initialState, initialStateWithFocus } from "./context";

describe("initialStateWithFocus", () => {
  it("builds a pane-stack state with the object as the active pane", () => {
    const s = initialStateWithFocus("a", "overview", "symbol");
    expect(s.navigation.panes).toHaveLength(1);
    expect(s.navigation.panes[0]!.objectId).toBe("a");
    expect(s.navigation.panes[0]!.kind).toBe("symbol");
    expect(s.activeObjectId).toBe("a");
    expect(s.activeViewId).toBe("overview");
  });

  it("accepts optional viewId and kind parameters", () => {
    const s = initialStateWithFocus("b", "call-graph", "file");
    expect(s.navigation.panes).toHaveLength(1);
    expect(s.navigation.panes[0]!.objectId).toBe("b");
    expect(s.navigation.panes[0]!.kind).toBe("file");
    expect(s.activeObjectId).toBe("b");
    expect(s.activeViewId).toBe("call-graph");
  });
});

describe("appReducer — initial state", () => {
  it("has empty navigation with no active pane", () => {
    expect(initialState.navigation.chain).toEqual([]);
    expect(initialState.navigation.panes).toEqual([]);
    expect(initialState.activeObjectId).toBeNull();
  });
});

describe("appReducer — navigation", () => {
  it("PUSH_PANE adds a new pane", () => {
    const next = appReducer(initialState, {
      type: "PUSH_PANE",
      payload: { objectId: "obj-1", kind: "symbol" },
    });
    expect(next.navigation.panes).toHaveLength(1);
    expect(next.navigation.panes[0]!.objectId).toBe("obj-1");
    expect(next.activeObjectId).toBe("obj-1");
  });

  it("SELECT_OBJECT on a new object opens a new pane", () => {
    const next = appReducer(initialState, {
      type: "SELECT_OBJECT",
      payload: { objectId: "obj-x", kind: "file" },
    });
    expect(next.navigation.panes).toHaveLength(1);
    expect(next.navigation.panes[0]!.objectId).toBe("obj-x");
  });

  it("SET_ACTIVE_VIEW updates the active pane", () => {
    let s = initialState;
    s = appReducer(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    const view = { object_id: "a", view_id: "call-graph" } as any;
    const next = appReducer(s, { type: "SET_ACTIVE_VIEW", payload: view });
    expect(next.navigation.panes[0]!.activeViewId).toBe("call-graph");
  });

  it("RESET returns to initial state", () => {
    let s = initialState;
    s = appReducer(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    const next = appReducer(s, { type: "RESET" });
    expect(next.navigation.panes).toEqual([]);
    expect(next.activeObjectId).toBeNull();
  });
});

describe("appReducer — perspective", () => {
  it("SET_PERSPECTIVE to 'c4' updates perspective", () => {
    const next = appReducer(initialState, {
      type: "SET_PERSPECTIVE",
      payload: "c4",
    });
    expect(next.perspective).toBe("c4");
  });

  it("SET_PERSPECTIVE to 'graph' updates perspective", () => {
    const stateWithC4 = { ...initialState, perspective: "c4" as const };
    const next = appReducer(stateWithC4, {
      type: "SET_PERSPECTIVE",
      payload: "graph",
    });
    expect(next.perspective).toBe("graph");
  });

  it("RESET returns perspective to 'graph'", () => {
    const stateWithC4 = { ...initialState, perspective: "c4" as const };
    const next = appReducer(stateWithC4, { type: "RESET" });
    expect(next.perspective).toBe("graph");
  });
});
