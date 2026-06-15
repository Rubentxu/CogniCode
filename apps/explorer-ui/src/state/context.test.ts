/**
 * Reducer tests for the SET_NAVIGATION_MODE flow.
 *
 * Verifies that switching modes resets navigation state and clears
 * the cached focus. Other reducer behaviour is exercised by the
 * MillerColumns / Spotter / ObjectInspector / LensPanel tests
 * (the full reducer is wired in those harnesses).
 */
import { describe, it, expect } from "vitest";
import { appReducer, initialState, initialStateWithFocus } from "./context";

describe("appReducer — navigation mode switching", () => {
  it("SET_NAVIGATION_MODE to 'pane-stack' resets state and clears focus", () => {
    const start = initialStateWithFocus("a", "column", "overview", "symbol");
    // sanity check
    expect(start.navigation.mode).toBe("column");
    expect(start.activeObjectId).toBe("a");

    const next = appReducer(start, {
      type: "SET_NAVIGATION_MODE",
      payload: { mode: "pane-stack" },
    });
    expect(next.navigation.mode).toBe("pane-stack");
    expect(next.navigation.chain).toEqual([]);
    expect(next.navigation.panes).toEqual([]);
    expect(next.activeObjectId).toBeNull();
    expect(next.activeViewId).toBeNull();
    expect(next.activeLensId).toBeNull();
    expect(next.activeView).toBeNull();
  });

  it("SET_NAVIGATION_MODE to the same mode is a no-op", () => {
    const start = initialStateWithFocus("a", "column", "overview", "symbol");
    const next = appReducer(start, {
      type: "SET_NAVIGATION_MODE",
      payload: { mode: "column" },
    });
    expect(next).toBe(start); // referential equality
  });

  it("SET_NAVIGATION_MODE to 'column' from 'pane-stack' resets state", () => {
    const start = initialStateWithFocus("a", "pane-stack", "overview", "symbol");
    const next = appReducer(start, {
      type: "SET_NAVIGATION_MODE",
      payload: { mode: "column" },
    });
    expect(next.navigation.mode).toBe("column");
    expect(next.activeObjectId).toBeNull();
  });
});

describe("initialStateWithFocus", () => {
  it("builds a column-mode state with the object as the leaf", () => {
    const s = initialStateWithFocus("a", "column", "overview", "symbol");
    expect(s.navigation.mode).toBe("column");
    expect(s.navigation.chain).toHaveLength(1);
    expect(s.navigation.chain[0]!.object_id).toBe("a");
    expect(s.navigation.chain[0]!.active_view).toBe("overview");
    expect(s.activeObjectId).toBe("a");
    expect(s.activeViewId).toBe("overview");
  });

  it("defaults to column mode when omitted", () => {
    const s = initialStateWithFocus("a");
    expect(s.navigation.mode).toBe("column");
  });

  it("accepts pane-stack mode", () => {
    const s = initialStateWithFocus("a", "pane-stack", "call-graph", "file");
    expect(s.navigation.mode).toBe("pane-stack");
    expect(s.navigation.panes).toHaveLength(1);
    expect(s.navigation.panes[0]!.objectId).toBe("a");
    expect(s.navigation.panes[0]!.kind).toBe("file");
  });
});

describe("appReducer — initial state", () => {
  it("is in column mode with empty navigation", () => {
    expect(initialState.navigation.mode).toBe("column");
    expect(initialState.navigation.chain).toEqual([]);
    expect(initialState.navigation.panes).toEqual([]);
    expect(initialState.activeObjectId).toBeNull();
  });
});
