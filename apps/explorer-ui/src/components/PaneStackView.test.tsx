/**
 * PaneStackView — reducer invariant tests.
 *
 * These tests verify the navigation reducer's pane-stack behavior
 * without rendering the full component tree (avoids cytoscape/CSS
 * complexity in unit-test context).
 *
 * Coverage:
 * - Drill-down opens a new pane and sets fromObjectId/viaViewKind
 * - Reselect dedup activates existing pane without pushing duplicate
 * - Closing active pane moves focus to neighbour
 * - Breadcrumb-from navigation dedup works correctly
 */
import { describe, it, expect } from "vitest";
import { apply } from "../state/slices/navigation/reducer";
import { makeInitialNavigationState } from "../state/slices/navigation/types";

describe("PaneStackView reducer invariants", () => {
  // -------------------------------------------------------------------------
  // Drill-down: pane count grows, fromObjectId is captured
  // -------------------------------------------------------------------------

  it("drill-down increments pane count", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "A", kind: "symbol" } });
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "B", kind: "symbol" } });
    expect(s.panes).toHaveLength(2);
    expect(s.activePaneId).toBe(s.panes[1]!.id);
  });

  it("second pane captures first pane as its origin", () => {
    let s = makeInitialNavigationState();
    s = apply(s, {
      type: "PUSH_PANE",
      payload: { objectId: "A", viewId: "call_graph", kind: "symbol" },
    });
    s = apply(s, {
      type: "PUSH_PANE",
      payload: { objectId: "B", viewId: "source_view", kind: "symbol" },
    });
    expect(s.panes[1]!.fromObjectId).toBe("A");
    expect(s.panes[1]!.viaViewKind).toBe("call_graph");
  });

  it("first pane has no origin (no breadcrumb)", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "A", kind: "symbol" } });
    expect(s.panes[0]!.fromObjectId).toBeUndefined();
    expect(s.panes[0]!.viaViewKind).toBeUndefined();
  });

  // -------------------------------------------------------------------------
  // Dedup: reselecting the same object activates existing pane
  // -------------------------------------------------------------------------

  it("SELECT_OBJECT on existing object does not add a new pane", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "A", kind: "symbol" } });
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "B", kind: "symbol" } });
    const originalCount = s.panes.length;
    s = apply(s, { type: "SELECT_OBJECT", payload: { objectId: "A" } });
    expect(s.panes).toHaveLength(originalCount);
  });

  it("SELECT_OBJECT on existing object activates that pane", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "A", kind: "symbol" } });
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "B", kind: "symbol" } });
    const paneAId = s.panes[0]!.id;
    s = apply(s, { type: "SELECT_OBJECT", payload: { objectId: "A" } });
    expect(s.activePaneId).toBe(paneAId);
  });

  it("SELECT_OBJECT on existing object updates activeViewId", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "A", viewId: "overview", kind: "symbol" } });
    s = apply(s, {
      type: "SELECT_OBJECT",
      payload: { objectId: "A", viewId: "call_graph" },
    });
    expect(s.panes[0]!.activeViewId).toBe("call_graph");
  });

  // -------------------------------------------------------------------------
  // Close: neighbour focus when active pane closes
  // -------------------------------------------------------------------------

  it("closing active pane moves focus to previous pane", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "A", kind: "symbol" } });
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "B", kind: "symbol" } });
    // B is active. Close it → A should become active.
    s = apply(s, { type: "CLOSE_PANE", payload: { paneId: s.panes[1]!.id } });
    expect(s.activePaneId).toBe(s.panes[0]!.id);
  });

  it("closing only pane sets activePaneId to null", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "A", kind: "symbol" } });
    s = apply(s, { type: "CLOSE_PANE", payload: { paneId: s.panes[0]!.id } });
    expect(s.panes).toHaveLength(0);
    expect(s.activePaneId).toBeNull();
  });

  // -------------------------------------------------------------------------
  // Breadcrumb-from click navigation dedups
  // -------------------------------------------------------------------------

  it("navigating to breadcrumb origin via SELECT_OBJECT activates origin pane", () => {
    let s = makeInitialNavigationState();
    s = apply(s, {
      type: "PUSH_PANE",
      payload: { objectId: "A", viewId: "call_graph", kind: "symbol" },
    });
    s = apply(s, {
      type: "PUSH_PANE",
      payload: { objectId: "B", viewId: "source_view", kind: "symbol" },
    });
    // B's breadcrumb shows "From A". Clicking A should activate A, not push new.
    const paneAId = s.panes[0]!.id;
    s = apply(s, { type: "SELECT_OBJECT", payload: { objectId: "A", viewId: "call_graph" } });
    expect(s.activePaneId).toBe(paneAId);
    expect(s.panes).toHaveLength(2);
  });

  // -------------------------------------------------------------------------
  // ACTIVATE_PANE
  // -------------------------------------------------------------------------

  it("ACTIVATE_PANE on nonexistent pane is a no-op", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "A", kind: "symbol" } });
    const before = s.activePaneId;
    s = apply(s, { type: "ACTIVATE_PANE", payload: { paneId: "nonexistent" } });
    expect(s.activePaneId).toBe(before);
  });

  it("ACTIVATE_PANE on existing pane changes activePaneId", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "A", kind: "symbol" } });
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "B", kind: "symbol" } });
    const paneAId = s.panes[0]!.id;
    s = apply(s, { type: "ACTIVATE_PANE", payload: { paneId: paneAId } });
    expect(s.activePaneId).toBe(paneAId);
  });
});
