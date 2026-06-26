/**
 * Tests for the `ContextualPanel` orchestrator.
 *
 * TDD: RED before the component lands; GREEN after.
 *
 * The panel composes the `useContextualGraph` hook with the
 * subcomponents (FocusCard, ParentBreadcrumb, ChildrenList,
 * TruncationBanner, NeighborMinigraph). Each test exercises one
 * state machine branch.
 */
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent, cleanup } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { createElement, type ReactNode } from "react";
import { SWRConfig } from "swr";

import { server } from "../../mocks/node";
import { ContextualPanel } from "./index";
import { resetCyMock } from "./NeighborMinigraph.test-helpers";

// Inline cytoscape mock — `vi.mock` is hoisted to the top of the
// file by vitest, so it must be a top-level call (not inside a
// function). We duplicate the mock factory here rather than
// import it from a helper, because vi.mock factories cannot be
// defined in a separate module.
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

function withSWR() {
  return function Wrapper({ children }: { children: ReactNode }) {
    return createElement(SWRConfig, {
      value: { provider: () => new Map(), dedupingInterval: 0 },
    }, children);
  };
}

beforeEach(() => {
  resetCyMock();
  server.resetHandlers();
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("ContextualPanel", () => {
  it("shows a loading state initially", async () => {
    render(<ContextualPanel focusId="sym:ctx::alpha" />, { wrapper: withSWR() });
    // On the very first render (before SWR resolves), the panel
    // shows the loading message.
    expect(screen.getByTestId("contextual-panel-loading")).toBeInTheDocument();
  });

  it("renders focus, parent, children, and neighbors on success", async () => {
    render(<ContextualPanel focusId="sym:ctx::alpha" />, { wrapper: withSWR() });
    await waitFor(() => {
      expect(screen.getByTestId("contextual-panel")).toBeInTheDocument();
    });
    // Focus card is present
    expect(screen.getByTestId("focus-card")).toBeInTheDocument();
    // Parent breadcrumb is present (parent is non-null in the fixture)
    expect(screen.getByTestId("parent-breadcrumb")).toBeInTheDocument();
    // Children list is present
    expect(screen.getByTestId("children-list")).toBeInTheDocument();
    // Neighbor minigraph is present (sameLevel has 1 node + 1 edge)
    expect(screen.getByTestId("neighbor-minigraph")).toBeInTheDocument();
  });

  it("hides the parent breadcrumb when parent is null", async () => {
    server.use(
      http.get("/api/graph/:id/contextual", () => {
        return HttpResponse.json({
          focusNode: {
            id: "sym:ctx::orphan",
            label: "orphan",
            kind: "function",
            file: "src/orphan.rs",
            line: 1,
            style_class: "function",
          },
          parent: null,
          children: null,
          sameLevel: { nodes: [], edges: [] },
          level: "file",
          truncated: false,
          truncatedReason: null,
        });
      }),
    );
    render(<ContextualPanel focusId="sym:ctx::orphan" />, { wrapper: withSWR() });
    await waitFor(() => {
      expect(screen.getByTestId("contextual-panel")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("parent-breadcrumb")).not.toBeInTheDocument();
    expect(screen.queryByTestId("children-list")).not.toBeInTheDocument();
  });

  it("shows the truncation banner when truncated=true", async () => {
    server.use(
      http.get("/api/graph/:id/contextual", () => {
        return HttpResponse.json({
          focusNode: {
            id: "sym:ctx::big",
            label: "big",
            kind: "function",
            file: "src/big.rs",
            line: 1,
            style_class: "function",
          },
          parent: null,
          children: null,
          sameLevel: { nodes: [], edges: [] },
          level: "file",
          truncated: true,
          truncatedReason: "max_nodes_exceeded",
        });
      }),
    );
    render(<ContextualPanel focusId="sym:ctx::big" />, { wrapper: withSWR() });
    await waitFor(() => {
      expect(screen.getByTestId("contextual-panel")).toBeInTheDocument();
    });
    expect(screen.getByTestId("truncation-banner")).toBeInTheDocument();
  });

  it("refocuses on parent breadcrumb click", async () => {
    // The orchestrator updates `window.location.hash` on click.
    // We snapshot the hash before, click, and assert the hash now
    // carries the parent's id.
    const initialHash = window.location.hash;
    render(<ContextualPanel focusId="sym:ctx::alpha" />, { wrapper: withSWR() });
    await waitFor(() => {
      expect(screen.getByTestId("parent-breadcrumb")).toBeInTheDocument();
    });
    const parentBtn = screen.getByTestId("parent-breadcrumb-button");
    fireEvent.click(parentBtn);
    // The orchestrator sets the hash to `#file:src/alpha.rs`.
    expect(window.location.hash).toBe(`#file:src/alpha.rs`);
    // Restore.
    window.location.hash = initialHash;
  });

  it("handles 404 with an error message", async () => {
    server.use(
      http.get("/api/graph/:id/contextual", () => {
        return HttpResponse.json(
          { error: "symbol_not_found" },
          { status: 404 },
        );
      }),
    );
    render(<ContextualPanel focusId="missing-sym" />, { wrapper: withSWR() });
    await waitFor(() => {
      expect(screen.getByTestId("contextual-panel-error")).toBeInTheDocument();
    });
    expect(screen.getByTestId("contextual-panel-error").textContent).toMatch(/not found/i);
  });
});
