/**
 * Tests for `RationaleView` — corroboration-rationale-views.
 *
 * The component renders an InteractiveGraph with a rationale header
 * that shows node/edge/scored-edge counts. Uses MSW to intercept the
 * rationale endpoint (handlers.ts serves the rationaleSubgraphFixture).
 *
 * Loading and error states are surfaced via testids.
 */
import { describe, expect, it, vi, afterEach } from "vitest";
import { render, screen, cleanup, waitFor } from "@testing-library/react";

// Mock cytoscape with a minimal shim so InteractiveGraph can mount
// without a real DOM canvas.
vi.mock("cytoscape", () => {
  type NodeData = { id: string; style_class?: string; label?: string };
  type EdgeData = { id: string; source: string; target: string };

  class CyNode {
    id: string;
    data: NodeData;
    classes: Set<string> = new Set();
    private listeners = new Map<string, Array<(e: unknown) => void>>();
    private edgeListeners: Set<CyEdge> = new Set();
    constructor(d: NodeData) {
      this.id = String(d.id);
      this.data = d;
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
    static fireTap(n: CyNode) {
      for (const fn of n.listeners.get("tap") ?? []) fn({ target: n });
    }
    style() { return this; }
  }
  class CyEdge {
    id: string;
    data: EdgeData;
    classes: Set<string> = new Set();
    constructor(d: EdgeData) {
      this.id = String(d.id);
      this.data = d;
    }
    addClass(c: string) { this.classes.add(c); }
    removeClass(c: string) { this.classes.delete(c); }
    hasClass(c: string) { return this.classes.has(c); }
    isNode() { return false; }
    isEdge() { return true; }
    style() { return this; }
  }
  class CyCollection {
    items: Array<CyNode | CyEdge>;
    constructor(items: Array<CyNode | CyEdge>) {
      this.items = items;
    }
    addClass(c: string) { for (const i of this.items) i.addClass(c); }
    removeClass(c: string) { for (const i of this.items) i.removeClass(c); }
    subtract(other: CyCollection): CyCollection {
      const ids = new Set(other.items.map((i) => i.id));
      return new CyCollection(this.items.filter((i) => !ids.has(i.id)));
    }
    length = 0;
  }
  class Cy {
    private _nodes: CyNode[] = [];
    private _edgeElements: CyEdge[] = [];
    private allListeners: Array<(e: unknown) => void> = [];
    constructor(opts: { elements?: { nodes?: NodeData[]; edges?: EdgeData[] } }) {
      this._nodes = (opts.elements?.nodes ?? []).map((d) => new CyNode(d));
      this._edgeElements = (opts.elements?.edges ?? []).map((d) => new CyEdge(d as EdgeData));
      for (const e of this._edgeElements) {
        const src = this._nodes.find((n) => n.id === String(e.data.source));
        const tgt = this._nodes.find((n) => n.id === String(e.data.target));
        if (src) (src as unknown as { edgeListeners: Set<CyEdge> }).edgeListeners.add(e);
        if (tgt) (tgt as unknown as { edgeListeners: Set<CyEdge> }).edgeListeners.add(e);
      }
    }
    on(_evt: string, selector: string | ((e: unknown) => void), fn?: (e: unknown) => void) {
      if (typeof selector === "function") {
        this.allListeners.push(selector);
      } else if (fn && selector === "node") {
        for (const n of this._nodes) n.on("tap", fn);
      }
    }
    off(_evt: string, fn: (e: unknown) => void) {
      for (const n of this._nodes) n.off("tap", fn);
    }
    elements(): CyCollection {
      return new CyCollection([...this._nodes, ...this._edgeElements]);
    }
    nodes(): CyCollection {
      return new CyCollection(this._nodes);
    }
    getElementById(id: string): CyCollection {
      const all = [...this._nodes, ...this._edgeElements];
      return new CyCollection(all.filter((i) => i.id === String(id)));
    }
    destroy() { /* no-op */ }
    // eslint-disable-next-line @typescript-eslint/no-unused-vars -- intentional unused param
    edges(_?: string) {
      return new CyCollection(this._edgeElements);
    }
  }
  return {
    default: ((opts: { elements?: { nodes?: NodeData[]; edges?: EdgeData[] } }) =>
      new Cy(opts)) as unknown as { (opts: unknown): unknown },
  };
});

import { RationaleView } from "./RationaleView";

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("RationaleView", () => {
  it("renders loading state", () => {
    render(
      <RationaleView
        focusId="sym:rat::focus"
        onSelectObject={() => {}}
      />,
    );
    expect(screen.getByTestId("rationale-loading")).toBeInTheDocument();
    expect(screen.getByText("Loading rationale…")).toBeInTheDocument();
  });

  it("renders rationale data with node/edge counts", async () => {
    render(
      <RationaleView
        focusId="sym:rat::focus"
        onSelectObject={() => {}}
      />,
    );

    // Wait for loading to resolve
    await waitFor(() => {
      expect(screen.queryByTestId("rationale-loading")).not.toBeInTheDocument();
    });

    expect(screen.getByTestId("rationale-view")).toBeInTheDocument();
    const header = screen.getByTestId("rationale-header");
    expect(header).toHaveTextContent(/3 nodes/);
    expect(header).toHaveTextContent(/2 edges/);
  });

  it("renders corroboration count when scores present", async () => {
    render(
      <RationaleView
        focusId="sym:rat::focus"
        onSelectObject={() => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.queryByTestId("rationale-loading")).not.toBeInTheDocument();
    });

    const header = screen.getByTestId("rationale-header");
    // The rationaleSubgraphFixture has 2 corroboration_scores entries
    expect(header).toHaveTextContent(/2 scored edges/);
  });

  it("passes props to InteractiveGraph (renders graph canvas)", async () => {
    render(
      <RationaleView
        focusId="sym:rat::focus"
        onSelectObject={() => {}}
        selectedId="sym:rat::focus"
      />,
    );

    await waitFor(() => {
      expect(screen.queryByTestId("rationale-loading")).not.toBeInTheDocument();
    });

    // The InteractiveGraph renders inside rationale-graph
    expect(screen.getByTestId("rationale-graph")).toBeInTheDocument();

    // The interactive-graph container should be rendered
    // (the cytoscape mock renders it when data is present)
    await waitFor(() => {
      expect(screen.getByTestId("interactive-graph")).toBeInTheDocument();
    });
  });

  it("renders error state on API failure", async () => {
    render(
      <RationaleView
        focusId="missing-sym"
        onSelectObject={() => {}}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("rationale-error")).toBeInTheDocument();
    });

    expect(screen.getByText("Error loading rationale")).toBeInTheDocument();
  });
});
