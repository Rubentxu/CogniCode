/**
 * Tests for the `NeighborMinigraph` component.
 *
 * Mocks cytoscape with a minimal in-memory implementation so the
 * tests run under jsdom (cytoscape needs Canvas). The cytoscape
 * mock factory is defined inline here because vitest hoists
 * `vi.mock` to the top of the file — it must be a sibling of the
 * imports it needs to mock.
 */
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { render } from "@testing-library/react";

vi.mock("cytoscape", () => {
  type NodeData = { id: string; style_class?: string; label?: string };
  type EdgeData = { id: string; source: string; target: string };

  class CyNode {
    id: string;
    data: NodeData;
    classes: Set<string> = new Set();
    listeners = new Map<string, Array<(e: unknown) => void>>();
    private edgeListeners = new Set<CyEdge>();
    constructor(element: { data?: NodeData; id?: string }) {
      const nodeData = element.data ?? (element as unknown as NodeData);
      this.id = String(nodeData.id);
      this.data = nodeData;
    }
    addClass(c: string) { this.classes.add(c); }
    removeClass(c: string) { this.classes.delete(c); }
    hasClass(c: string) { return this.classes.has(c); }
    on(evt: string, fn: (e: unknown) => void) {
      const list = this.listeners.get(evt) ?? [];
      list.push(fn);
      this.listeners.set(evt, list);
    }
    off(evt: string, fn: (e: unknown) => void) {
      const list = this.listeners.get(evt) ?? [];
      this.listeners.set(evt, list.filter((f) => f !== fn));
    }
    connectedEdges(): CyEdge[] {
      return [...this.edgeListeners];
    }
    private emit() {
      for (const fn of this.listeners.get("tap") ?? []) fn({ target: this });
    }
    static fireTap(n: CyNode) { n.emit(); }
  }
  class CyEdge {
    id: string;
    data: EdgeData;
    classes: Set<string> = new Set();
    constructor(element: { data?: EdgeData; id?: string }) {
      const edgeData = element.data ?? (element as unknown as EdgeData);
      this.id = String(edgeData.id);
      this.data = edgeData;
    }
    addClass(c: string) { this.classes.add(c); }
    removeClass(c: string) { this.classes.delete(c); }
    hasClass(c: string) { return this.classes.has(c); }
  }
  class Cy {
    nodes: CyNode[] = [];
    edges: CyEdge[] = [];
    destroyed = false;
    constructor(opts: { elements?: { nodes?: unknown[]; edges?: unknown[] } }) {
      this.nodes = (opts.elements?.nodes ?? []).map(
        (d) => new CyNode(d as { data: NodeData }),
      );
      this.edges = (opts.elements?.edges ?? []).map(
        (d) => new CyEdge(d as { data: EdgeData }),
      );
      for (const e of this.edges) {
        const src = this.nodes.find((n) => n.id === String(e.data.source));
        const tgt = this.nodes.find((n) => n.id === String(e.data.target));
        if (src) (src as unknown as { edgeListeners: Set<CyEdge> }).edgeListeners.add(e);
        if (tgt) (tgt as unknown as { edgeListeners: Set<CyEdge> }).edgeListeners.add(e);
      }
      const inst = {
        nodes: this.nodes.map((n) => ({ id: n.id, data: n.data })),
        edges: this.edges.map((e) => ({ id: e.id, data: e.data })),
        destroyed: false,
        clickNode: (id: string) => {
          const n = this.nodes.find((x) => x.id === id);
          if (n) CyNode.fireTap(n);
        },
      };
      (globalThis as unknown as { __cyInstances?: unknown[] }).__cyInstances?.push(inst);
      const realDestroy = () => {
        this.destroyed = true;
        inst.destroyed = true;
      };
      this.destroy = realDestroy;
    }
    on(_evt: string, selector: string | ((e: unknown) => void), fn?: (e: unknown) => void) {
      if (typeof selector === "function") return;
      if (fn && selector === "node") {
        for (const n of this.nodes) n.on("tap", fn);
      }
    }
    off(_evt: string, fn: (e: unknown) => void) {
      for (const n of this.nodes) n.off("tap", fn);
    }
    destroy() { this.destroyed = true; }
  }
  return {
    default: ((opts: { elements?: { nodes?: NodeData[]; edges?: EdgeData[] } }) =>
      new Cy(opts)) as unknown as { (opts: unknown): unknown },
  };
});

import { NeighborMinigraph } from "./NeighborMinigraph";
import { getCyInstances, resetCyMock } from "./NeighborMinigraph.test-helpers";
import type { GraphEdge, GraphNode } from "../../api/types";

const focus: GraphNode = {
  id: "sym:foo::alpha",
  label: "alpha",
  kind: "function",
  file: "src/foo.rs",
  line: 1,
  style_class: "function",
};

const neighbors: GraphNode[] = [
  {
    id: "sym:foo::beta",
    label: "beta",
    kind: "function",
    file: "src/foo.rs",
    line: 10,
    style_class: "function",
  },
];

const edges: GraphEdge[] = [
  {
    source: "sym:foo::alpha",
    target: "sym:foo::beta",
    relation: "calls",
    style_class: "edge.calls",
  },
];

beforeEach(() => {
  resetCyMock();
});

afterEach(() => {
  vi.clearAllMocks();
});

describe("NeighborMinigraph", () => {
  it("initializes cytoscape with nodes and edges", () => {
    render(
      <NeighborMinigraph
        focus={focus}
        nodes={neighbors}
        edges={edges}
        onFocus={() => {}}
      />,
    );
    const insts = getCyInstances();
    expect(insts.length).toBeGreaterThan(0);
    const inst = insts[insts.length - 1]!;
    // focus + 1 neighbor = 2 nodes, 1 edge.
    expect(inst.nodes.length).toBe(2);
    expect(inst.edges.length).toBe(1);
  });

  it("destroys cytoscape on unmount", () => {
    const { unmount } = render(
      <NeighborMinigraph
        focus={focus}
        nodes={neighbors}
        edges={edges}
        onFocus={() => {}}
      />,
    );
    const insts = getCyInstances();
    const inst = insts[insts.length - 1]!;
    unmount();
    expect(inst.destroyed).toBe(true);
  });

  it("calls onFocus on node tap", async () => {
    const onFocus = vi.fn();
    render(
      <NeighborMinigraph
        focus={focus}
        nodes={neighbors}
        edges={edges}
        onFocus={onFocus}
      />,
    );
    await new Promise((r) => setTimeout(r, 10));
    const insts = getCyInstances();
    const inst = insts[insts.length - 1]!;
    inst.clickNode("sym:foo::beta");
    expect(onFocus).toHaveBeenCalledWith("sym:foo::beta");
  });
});
