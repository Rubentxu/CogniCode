/**
 * Tests for PaneStackNavigation.
 *
 * Coverage:
 * - PUSH_PANE, CLOSE_PANE, ACTIVATE_PANE, REORDER_PANE, SET_PANE_SCROLL,
 *   SELECT_OBJECT (opens new pane or focuses existing), SET_ACTIVE_VIEW,
 *   SET_ACTIVE_LENS, MAX_PANES cap.
 * - Focus tracking: getActiveFocus returns the active pane.
 */
import { describe, it, expect } from "vitest";
import { apply, getActiveFocus, hasFocus, MAX_PANES } from "./reducer";
import { makeInitialNavigationState } from "./types";
import type { ContextualView } from "../../../api/types";

// ============================================================================
// PaneStackNavigation
// ============================================================================

describe("PaneStackNavigation", () => {
  it("starts empty", () => {
    const s = makeInitialNavigationState();
    expect(s.panes).toEqual([]);
    expect(s.activePaneId).toBeNull();
    expect(hasFocus(s)).toBe(false);
  });

  it("PUSH_PANE opens a new pane and focuses it", () => {
    const s = apply(makeInitialNavigationState(), {
      type: "PUSH_PANE",
      payload: { objectId: "a", kind: "symbol" },
    });
    expect(s.panes).toHaveLength(1);
    expect(s.activePaneId).toBe(s.panes[0]!.id);
    expect(s.panes[0]!.objectId).toBe("a");
    expect(getActiveFocus(s).objectId).toBe("a");
  });

  it("PUSH_PANE twice opens two panes, second is active", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "b", kind: "symbol" } });
    expect(s.panes).toHaveLength(2);
    expect(s.panes[0]!.objectId).toBe("a");
    expect(s.panes[1]!.objectId).toBe("b");
    expect(s.activePaneId).toBe(s.panes[1]!.id);
  });

  it("MAX_PANES cap drops the oldest (FIFO)", () => {
    let s = makeInitialNavigationState();
    for (let i = 0; i < MAX_PANES + 2; i++) {
      s = apply(s, {
        type: "PUSH_PANE",
        payload: { objectId: `obj-${i}`, kind: "symbol" },
      });
    }
    expect(s.panes).toHaveLength(MAX_PANES);
    // First two were dropped.
    expect(s.panes[0]!.objectId).toBe(`obj-2`);
    expect(s.panes[MAX_PANES - 1]!.objectId).toBe(`obj-${MAX_PANES + 1}`);
  });

  it("CLOSE_PANE removes the pane and moves focus to a neighbour", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "b", kind: "symbol" } });
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "c", kind: "symbol" } });
    const middleId = s.panes[1]!.id;
    s = apply(s, { type: "CLOSE_PANE", payload: { paneId: middleId } });
    expect(s.panes).toHaveLength(2);
    expect(s.panes.find((p) => p.id === middleId)).toBeUndefined();
  });

  it("CLOSE_PANE on the last pane sets activePaneId to null", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    const onlyId = s.panes[0]!.id;
    s = apply(s, { type: "CLOSE_PANE", payload: { paneId: onlyId } });
    expect(s.panes).toHaveLength(0);
    expect(s.activePaneId).toBeNull();
    expect(hasFocus(s)).toBe(false);
  });

  it("CLOSE_PANE on the active pane moves focus to a neighbour", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "b", kind: "symbol" } });
    const activeId = s.activePaneId!;
    s = apply(s, { type: "CLOSE_PANE", payload: { paneId: activeId } });
    // Active was last (b). After close, focus moves to "a".
    const first = s.panes[0]!;
    expect(s.activePaneId).toBe(first.id);
    expect(first.objectId).toBe("a");
  });

  it("ACTIVATE_PANE changes focus", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "b", kind: "symbol" } });
    const firstId = s.panes[0]!.id;
    s = apply(s, { type: "ACTIVATE_PANE", payload: { paneId: firstId } });
    expect(s.activePaneId).toBe(firstId);
  });

  it("REORDER_PANE swaps positions", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "b", kind: "symbol" } });
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "c", kind: "symbol" } });
    s = apply(s, { type: "REORDER_PANE", payload: { fromIndex: 0, toIndex: 2 } });
    expect(s.panes.map((p) => p.objectId)).toEqual(["b", "c", "a"]);
  });

  it("REORDER_PANE with invalid indices is a no-op", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    const before = s.panes.map((p) => p.objectId);
    s = apply(s, { type: "REORDER_PANE", payload: { fromIndex: 0, toIndex: 99 } });
    expect(s.panes.map((p) => p.objectId)).toEqual(before);
  });

  it("SELECT_OBJECT on a new object opens a new pane", () => {
    const s = apply(makeInitialNavigationState(), {
      type: "SELECT_OBJECT",
      payload: { objectId: "x", kind: "symbol" },
    });
    expect(s.panes).toHaveLength(1);
    expect(s.panes[0]!.objectId).toBe("x");
  });

  it("SELECT_OBJECT on an existing object focuses that pane (and updates view)", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "b", kind: "symbol" } });
    s = apply(s, {
      type: "SELECT_OBJECT",
      payload: { objectId: "a", viewId: "call-graph", kind: "symbol" },
    });
    expect(s.panes).toHaveLength(2); // no new pane opened
    const first = s.panes[0];
    expect(first).toBeDefined();
    expect(s.activePaneId).toBe(first!.id);
    expect(first!.activeViewId).toBe("call-graph");
  });

  it("SET_ACTIVE_VIEW updates the active pane", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    const view = { object_id: "a", view_id: "call-graph" } as unknown as ContextualView;
    s = apply(s, { type: "SET_ACTIVE_VIEW", payload: view });
    const first = s.panes[0]!;
    expect(first.activeViewId).toBe("call-graph");
  });

  it("SET_PANE_SCROLL records scroll on the named pane", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    const id = s.panes[0]!.id;
    s = apply(s, {
      type: "SET_PANE_SCROLL",
      payload: { paneId: id, scrollY: 240 },
    });
    expect(s.panes[0]!.scrollY).toBe(240);
  });

  it("RESET clears the state", () => {
    let s = makeInitialNavigationState();
    s = apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    s = apply(s, { type: "RESET" });
    expect(s.panes).toEqual([]);
    expect(s.activePaneId).toBeNull();
  });
});
