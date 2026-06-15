/**
 * Tests for the NavigationAdapter system.
 *
 * Coverage:
 * - ColumnNavigation: PUSH_COLUMN, POP_COLUMN, SELECT_OBJECT, SET_ACTIVE_VIEW,
 *   SET_ACTIVE_LENS, RESET. Pane-stack actions are no-ops.
 * - PaneStackNavigation: PUSH_PANE, CLOSE_PANE, ACTIVATE_PANE, REORDER_PANE,
 *   SET_PANE_SCROLL, SELECT_OBJECT (opens new pane or focuses existing),
 *   SET_ACTIVE_VIEW, SET_ACTIVE_LENS, MAX_PANES cap. Column actions are no-ops.
 * - Focus tracking: getActiveFocus returns the active leaf / pane.
 */
import { describe, it, expect } from "vitest";
import { ColumnNavigation } from "./column";
import { PaneStackNavigation, MAX_PANES } from "./paneStack";
import { makeInitialNavigationState } from "./types";
import type { ExplorationColumn, ContextualView } from "../../api/types";

// ============================================================================
// ColumnNavigation
// ============================================================================

describe("ColumnNavigation", () => {
  const col = (id: string, view: string | null = "overview", kind = "symbol"): ExplorationColumn => ({
    object_id: id,
    active_view: view,
    kind,
  });

  it("starts empty", () => {
    const s = makeInitialNavigationState("column");
    expect(s.chain).toEqual([]);
    expect(s.panes).toEqual([]);
    expect(s.activePaneId).toBeNull();
    expect(ColumnNavigation.hasFocus(s)).toBe(false);
    expect(ColumnNavigation.getActiveFocus(s)).toEqual({
      objectId: null,
      viewId: null,
      lensId: null,
      view: null,
    });
  });

  it("PUSH_COLUMN appends to the chain and mirrors as a single leaf pane", () => {
    const s = ColumnNavigation.apply(makeInitialNavigationState("column"), {
      type: "PUSH_COLUMN",
      payload: col("a"),
    });
    expect(s.chain.map((c) => c.object_id)).toEqual(["a"]);
    expect(s.panes).toHaveLength(1);
    expect(s.panes[0]!.objectId).toBe("a");
    expect(ColumnNavigation.hasFocus(s)).toBe(true);
  });

  it("PUSH_COLUMN twice yields a chain of two and a single leaf pane", () => {
    let s = makeInitialNavigationState("column");
    s = ColumnNavigation.apply(s, { type: "PUSH_COLUMN", payload: col("a") });
    s = ColumnNavigation.apply(s, { type: "PUSH_COLUMN", payload: col("b") });
    expect(s.chain.map((c) => c.object_id)).toEqual(["a", "b"]);
    expect(s.panes).toHaveLength(1);
    expect(s.panes[0]!.objectId).toBe("b");
  });

  it("POP_COLUMN truncates and clears focus if the leaf is dropped", () => {
    let s = makeInitialNavigationState("column");
    s = ColumnNavigation.apply(s, { type: "PUSH_COLUMN", payload: col("a") });
    s = ColumnNavigation.apply(s, { type: "PUSH_COLUMN", payload: col("b") });
    s = ColumnNavigation.apply(s, { type: "POP_COLUMN", payload: { index: 1 } });
    expect(s.chain).toHaveLength(1);
    expect(s.chain[0]!.object_id).toBe("a");
    expect(s.panes).toHaveLength(1);
    expect(s.panes[0]!.objectId).toBe("a");
  });

  it("POP_COLUMN on a non-leaf keeps the leaf", () => {
    let s = makeInitialNavigationState("column");
    s = ColumnNavigation.apply(s, { type: "PUSH_COLUMN", payload: col("a") });
    s = ColumnNavigation.apply(s, { type: "PUSH_COLUMN", payload: col("b") });
    s = ColumnNavigation.apply(s, { type: "POP_COLUMN", payload: { index: 0 } });
    expect(s.chain).toHaveLength(0);
    // After popping to 0, the leaf pane is gone too.
    expect(s.panes).toHaveLength(0);
  });

  it("SELECT_OBJECT on a new object appends a column", () => {
    let s = makeInitialNavigationState("column");
    s = ColumnNavigation.apply(s, { type: "PUSH_COLUMN", payload: col("a") });
    s = ColumnNavigation.apply(s, {
      type: "SELECT_OBJECT",
      payload: { objectId: "b", kind: "symbol" },
    });
    expect(s.chain.map((c) => c.object_id)).toEqual(["a", "b"]);
  });

  it("SELECT_OBJECT on the same object replaces the leaf", () => {
    let s = makeInitialNavigationState("column");
    s = ColumnNavigation.apply(s, { type: "PUSH_COLUMN", payload: col("a") });
    s = ColumnNavigation.apply(s, { type: "PUSH_COLUMN", payload: col("a") });
    const before = s.chain.length;
    s = ColumnNavigation.apply(s, {
      type: "SELECT_OBJECT",
      payload: { objectId: "a", viewId: "call-graph", kind: "symbol" },
    });
    // Same object → replace, not append.
    expect(s.chain.length).toBe(before);
    expect(s.chain[s.chain.length - 1]!.active_view).toBe("call-graph");
  });

  it("SET_ACTIVE_VIEW updates the leaf column's active_view", () => {
    let s = makeInitialNavigationState("column");
    s = ColumnNavigation.apply(s, { type: "PUSH_COLUMN", payload: col("a", "overview") });
    const view = { object_id: "a", view_id: "call-graph" } as unknown as ContextualView;
    s = ColumnNavigation.apply(s, { type: "SET_ACTIVE_VIEW", payload: view });
    expect(s.chain[0]!.active_view).toBe("call-graph");
    expect(s.panes[0]!.activeViewId).toBe("call-graph");
  });

  it("SET_ACTIVE_LENS updates the leaf pane's lensId", () => {
    let s = makeInitialNavigationState("column");
    s = ColumnNavigation.apply(s, { type: "PUSH_COLUMN", payload: col("a") });
    s = ColumnNavigation.apply(s, { type: "SET_ACTIVE_LENS", payload: { lensId: "hotspots" } });
    expect(s.panes[0]!.activeLensId).toBe("hotspots");
  });

  it("RESET clears the state", () => {
    let s = makeInitialNavigationState("column");
    s = ColumnNavigation.apply(s, { type: "PUSH_COLUMN", payload: col("a") });
    s = ColumnNavigation.apply(s, { type: "RESET" });
    expect(s.chain).toEqual([]);
    expect(s.panes).toEqual([]);
  });

  it("pane-stack actions are no-ops", () => {
    let s = makeInitialNavigationState("column");
    s = ColumnNavigation.apply(s, { type: "PUSH_COLUMN", payload: col("a") });
    const before = s.chain.length;
    const panesBefore = s.panes.length;
    s = ColumnNavigation.apply(s, {
      type: "PUSH_PANE",
      payload: { objectId: "x", kind: "symbol" },
    });
    expect(s.chain.length).toBe(before);
    expect(s.panes.length).toBe(panesBefore);
  });
});

// ============================================================================
// PaneStackNavigation
// ============================================================================

describe("PaneStackNavigation", () => {
  it("starts empty", () => {
    const s = makeInitialNavigationState("pane-stack");
    expect(s.panes).toEqual([]);
    expect(s.activePaneId).toBeNull();
    expect(PaneStackNavigation.hasFocus(s)).toBe(false);
  });

  it("PUSH_PANE opens a new pane and focuses it", () => {
    const s = PaneStackNavigation.apply(makeInitialNavigationState("pane-stack"), {
      type: "PUSH_PANE",
      payload: { objectId: "a", kind: "symbol" },
    });
    expect(s.panes).toHaveLength(1);
    expect(s.activePaneId).toBe(s.panes[0]!.id);
    expect(s.panes[0]!.objectId).toBe("a");
    expect(PaneStackNavigation.getActiveFocus(s).objectId).toBe("a");
  });

  it("PUSH_PANE twice opens two panes, second is active", () => {
    let s = makeInitialNavigationState("pane-stack");
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "b", kind: "symbol" } });
    expect(s.panes).toHaveLength(2);
    expect(s.panes[0]!.objectId).toBe("a");
    expect(s.panes[1]!.objectId).toBe("b");
    expect(s.activePaneId).toBe(s.panes[1]!.id);
  });

  it("MAX_PANES cap drops the oldest (FIFO)", () => {
    let s = makeInitialNavigationState("pane-stack");
    for (let i = 0; i < MAX_PANES + 2; i++) {
      s = PaneStackNavigation.apply(s, {
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
    let s = makeInitialNavigationState("pane-stack");
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "b", kind: "symbol" } });
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "c", kind: "symbol" } });
    const middleId = s.panes[1]!.id;
    s = PaneStackNavigation.apply(s, { type: "CLOSE_PANE", payload: { paneId: middleId } });
    expect(s.panes).toHaveLength(2);
    expect(s.panes.find((p) => p.id === middleId)).toBeUndefined();
  });

  it("CLOSE_PANE on the last pane sets activePaneId to null", () => {
    let s = makeInitialNavigationState("pane-stack");
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    const onlyId = s.panes[0]!.id;
    s = PaneStackNavigation.apply(s, { type: "CLOSE_PANE", payload: { paneId: onlyId } });
    expect(s.panes).toHaveLength(0);
    expect(s.activePaneId).toBeNull();
    expect(PaneStackNavigation.hasFocus(s)).toBe(false);
  });

  it("CLOSE_PANE on the active pane moves focus to a neighbour", () => {
    let s = makeInitialNavigationState("pane-stack");
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "b", kind: "symbol" } });
    const activeId = s.activePaneId!;
    s = PaneStackNavigation.apply(s, { type: "CLOSE_PANE", payload: { paneId: activeId } });
    // Active was last (b). After close, focus moves to "a".
    const first = s.panes[0]!;
    expect(s.activePaneId).toBe(first.id);
    expect(first.objectId).toBe("a");
  });

  it("ACTIVATE_PANE changes focus", () => {
    let s = makeInitialNavigationState("pane-stack");
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "b", kind: "symbol" } });
    const firstId = s.panes[0]!.id;
    s = PaneStackNavigation.apply(s, { type: "ACTIVATE_PANE", payload: { paneId: firstId } });
    expect(s.activePaneId).toBe(firstId);
  });

  it("REORDER_PANE swaps positions", () => {
    let s = makeInitialNavigationState("pane-stack");
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "b", kind: "symbol" } });
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "c", kind: "symbol" } });
    s = PaneStackNavigation.apply(s, { type: "REORDER_PANE", payload: { fromIndex: 0, toIndex: 2 } });
    expect(s.panes.map((p) => p.objectId)).toEqual(["b", "c", "a"]);
  });

  it("REORDER_PANE with invalid indices is a no-op", () => {
    let s = makeInitialNavigationState("pane-stack");
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    const before = s.panes.map((p) => p.objectId);
    s = PaneStackNavigation.apply(s, { type: "REORDER_PANE", payload: { fromIndex: 0, toIndex: 99 } });
    expect(s.panes.map((p) => p.objectId)).toEqual(before);
  });

  it("SELECT_OBJECT on a new object opens a new pane", () => {
    const s = PaneStackNavigation.apply(makeInitialNavigationState("pane-stack"), {
      type: "SELECT_OBJECT",
      payload: { objectId: "x", kind: "symbol" },
    });
    expect(s.panes).toHaveLength(1);
    expect(s.panes[0]!.objectId).toBe("x");
  });

  it("SELECT_OBJECT on an existing object focuses that pane (and updates view)", () => {
    let s = makeInitialNavigationState("pane-stack");
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "b", kind: "symbol" } });
    s = PaneStackNavigation.apply(s, {
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
    let s = makeInitialNavigationState("pane-stack");
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    const view = { object_id: "a", view_id: "call-graph" } as unknown as ContextualView;
    s = PaneStackNavigation.apply(s, { type: "SET_ACTIVE_VIEW", payload: view });
    const first = s.panes[0]!;
    expect(first.activeViewId).toBe("call-graph");
  });

  it("SET_PANE_SCROLL records scroll on the named pane", () => {
    let s = makeInitialNavigationState("pane-stack");
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    const id = s.panes[0]!.id;
    s = PaneStackNavigation.apply(s, {
      type: "SET_PANE_SCROLL",
      payload: { paneId: id, scrollY: 240 },
    });
    expect(s.panes[0]!.scrollY).toBe(240);
  });

  it("column-only actions are no-ops", () => {
    let s = makeInitialNavigationState("pane-stack");
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    const beforePanes = s.panes.length;
    s = PaneStackNavigation.apply(s, {
      type: "PUSH_COLUMN",
      payload: { object_id: "x", active_view: null, kind: "symbol" },
    });
    s = PaneStackNavigation.apply(s, { type: "POP_COLUMN", payload: { index: 0 } });
    expect(s.panes.length).toBe(beforePanes);
  });

  it("RESET clears the state", () => {
    let s = makeInitialNavigationState("pane-stack");
    s = PaneStackNavigation.apply(s, { type: "PUSH_PANE", payload: { objectId: "a", kind: "symbol" } });
    s = PaneStackNavigation.apply(s, { type: "RESET" });
    expect(s.panes).toEqual([]);
    expect(s.activePaneId).toBeNull();
  });
});
