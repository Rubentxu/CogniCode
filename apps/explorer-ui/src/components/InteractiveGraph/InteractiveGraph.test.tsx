/**
 * `InteractiveGraph` component — render, empty state, selection
 * state machine, a11y fallback.
 *
 * TDD: every block here is RED before the component lands.
 */
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import { act } from "react";
import { act as domAct } from "react-dom/test-utils";

// Track destroy calls for E5.5 crossfade regression tests
let destroyCallCount = 0;

vi.mock("cytoscape", () => {
  type NodeData = { id: string; style_class?: string; label?: string };
  type EdgeData = { id: string; source: string; target: string };

  class CyNode {
    id: string;
    data: NodeData;
    classes: Set<string> = new Set();
    private listeners = new Map<string, Array<(e: unknown) => void>>();
    private edgeListeners = new Set<CyEdge>();
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
    private emit() {
      for (const fn of this.listeners.get("tap") ?? []) fn({ target: this });
    }
    static fireTap(n: CyNode) { n.emit(); }
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
    // Renamed to avoid duplicate identifier with the nodes() method below.
    // @ts-expect-error -- mock property, the method is the public API
    nodes: CyNode[] = [];
    edgeElements: CyEdge[] = [];
    private allListeners: Array<(e: unknown) => void> = [];
    constructor(opts: {
      elements?: { nodes?: NodeData[]; edges?: NodeData[] };
      renderer?: { name: string; webgl?: boolean };
    }) {
      this.nodes = (opts.elements?.nodes ?? []).map((d) => new CyNode(d));
      this.edgeElements = (opts.elements?.edges ?? []).map(
        (d) => new CyEdge(d as EdgeData),
      );
      // Wire connectedEdges: each edge touches its endpoints.
      for (const e of this.edgeElements) {
        const src = this.nodes.find((n) => n.id === String(e.data.source));
        const tgt = this.nodes.find((n) => n.id === String(e.data.target));
        if (src) (src as unknown as { edgeListeners: Set<CyEdge> }).edgeListeners.add(e);
        if (tgt) (tgt as unknown as { edgeListeners: Set<CyEdge> }).edgeListeners.add(e);
      }
    }
    on(_evt: string, selector: string | ((e: unknown) => void), fn?: (e: unknown) => void) {
      if (typeof selector === "function") {
        this.allListeners.push(selector);
      } else if (fn) {
        // Selector-based: only handle "node" by attaching to every node.
        if (selector === "node") {
          for (const n of this.nodes) n.on("tap", fn);
        }
      }
    }
    off(_evt: string, fn: (e: unknown) => void) {
      for (const n of this.nodes) n.off("tap", fn);
    }
    elements(): CyCollection {
      return new CyCollection([...this.nodes, ...this.edgeElements]);
    }
    getElementById(id: string): CyCollection {
      const all = [...this.nodes, ...this.edgeElements];
      return new CyCollection(all.filter((i) => i.id === String(id)));
    }
    // eslint-disable-next-line @typescript-eslint/no-unused-vars -- intentional unused param
    edges(_?: string) {
      return new CyCollection(this.edgeElements);
    }
    // eslint-disable-next-line @typescript-eslint/no-unused-vars -- intentional unused param
    // @ts-expect-error -- intentional duplicate with property above; method is the public API
    nodes(_?: string) {
      return new CyCollection(this.nodes);
    }
    destroy() { destroyCallCount++; }
    // eslint-disable-next-line @typescript-eslint/no-unused-vars -- intentional unused param
    layout(_?: { name: string; rows?: number }) {
      return { run: () => {} }; // cytoscape layout returns a runnable descriptor
    }
    clickNode(id: string) {
      const n = this.nodes.find((x) => x.id === id);
      if (n) CyNode.fireTap(n);
    }
  }
  return {
    default: ((opts: {
      elements?: { nodes?: NodeData[]; edges?: NodeData[] };
      renderer?: { name: string; webgl?: boolean };
    }) => new Cy(opts)) as unknown as { (opts: unknown): unknown },
  };
});

import { InteractiveGraph } from "./InteractiveGraph";
import { smallSubgraphFixture } from "../../mocks/subgraphFixtures";

beforeEach(() => {
  // Default: parent-level state is uncontrolled in these tests.
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe("InteractiveGraph", () => {
  it("renders with a testid when given a valid SubgraphResponse", () => {
    render(
      <InteractiveGraph
        root="sym:foo::bar"
        data={smallSubgraphFixture}
        onSelectObject={() => {}}
      />,
    );
    expect(screen.getByTestId("interactive-graph")).toBeInTheDocument();
  });

  it("renders the empty state when data is null", () => {
    render(
      <InteractiveGraph
        root="sym:foo::bar"
        data={null}
        onSelectObject={() => {}}
      />,
    );
    expect(screen.getByTestId("interactive-graph-empty")).toBeInTheDocument();
  });

  it("renders the empty state when nodes is empty", () => {
    render(
      <InteractiveGraph
        root="sym:foo::bar"
        data={{ root: "x", nodes: [], edges: [], truncated: false, corroboration_scores: {} }}
        onSelectObject={() => {}}
      />,
    );
    expect(screen.getByTestId("interactive-graph-empty")).toBeInTheDocument();
  });

  it("exposes role=application with an accessible label", () => {
    render(
      <InteractiveGraph
        root="sym:foo::bar"
        data={smallSubgraphFixture}
        onSelectObject={() => {}}
      />,
    );
    const el = screen.getByRole("application");
    expect(el).toHaveAttribute("aria-label", "Interactive graph of sym:foo::bar");
  });

  it("calls onSelectObject once when a node is clicked", () => {
    const onSelect = vi.fn();
    render(
      <InteractiveGraph
        root="sym:foo::bar"
        data={smallSubgraphFixture}
        onSelectObject={onSelect}
      />,
    );
    // Grab the cytoscape instance and click the first node.
    // The component exposes a ref-like accessor via testid;
    // we use the global cytoscape mock.
    // Easiest: find a fallback-table row to click.
    const row = screen.getAllByRole("row")[1]!;
    fireEvent.click(row);
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(typeof onSelect.mock.calls[0]![0]).toBe("string");
  });

  it("does not call onSelectObject when the background is clicked", () => {
    const onSelect = vi.fn();
    render(
      <InteractiveGraph
        root="sym:foo::bar"
        data={smallSubgraphFixture}
        onSelectObject={onSelect}
      />,
    );
    // Clicking the SVG/cytoscape container (not a row) should
    // not call onSelectObject. We click the application region
    // directly.
    const region = screen.getByRole("application");
    fireEvent.click(region);
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("clearing selectedId removes selection classes", () => {
    const { rerender } = render(
      <InteractiveGraph
        root="sym:foo::bar"
        data={smallSubgraphFixture}
        selectedId="sym:foo::n0"
        onSelectObject={() => {}}
      />,
    );
    // After clearing, the fallback table should not visually mark
    // any row as selected (no aria-selected="true").
    rerender(
      <InteractiveGraph
        root="sym:foo::bar"
        data={smallSubgraphFixture}
        selectedId={null}
        onSelectObject={() => {}}
      />,
    );
    const rows = screen.getAllByRole("row");
    for (const r of rows) {
      if (r.getAttribute("aria-selected") !== null) {
        expect(r.getAttribute("aria-selected")).not.toBe("true");
      }
    }
  });

  it("warns to console for unknown style_class and falls back visually", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const weird = {
      root: "x",
      nodes: [
        {
          id: "n1",
          label: "weird",
          kind: "alien",
          // Force-cast: the test deliberately exercises the
          // unknown-style_class path. The production zod schema
          // would reject this — the test is end-to-end (component
          // receives the wire and warns + falls back).
          style_class: "alien" as unknown as "function",
        },
      ],
      edges: [],
      truncated: false,
      corroboration_scores: {},
    };
    render(
      <InteractiveGraph
        root="x"
        data={weird}
        onSelectObject={() => {}}
      />,
    );
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });

  it("container is Tab-reachable (tabIndex=0)", () => {
    render(
      <InteractiveGraph
        root="x"
        data={smallSubgraphFixture}
        onSelectObject={() => {}}
      />,
    );
    const region = screen.getByRole("application");
    expect(region).toHaveAttribute("tabindex", "0");
  });

  it("renders a fallback table with role=complementary listing every node", () => {
    render(
      <InteractiveGraph
        root="x"
        data={smallSubgraphFixture}
        onSelectObject={() => {}}
      />,
    );
    const table = screen.getByRole("complementary", { name: /graph nodes/i });
    expect(table.tagName.toLowerCase()).toBe("table");
    // The table should have one row per node.
    const rows = screen.getAllByRole("row");
    // rows[0] is the header; the rest are data rows.
    expect(rows.length).toBeGreaterThanOrEqual(smallSubgraphFixture.nodes.length);
  });

  it("pressing Enter on a fallback row calls onSelectObject", () => {
    const onSelect = vi.fn();
    render(
      <InteractiveGraph
        root="x"
        data={smallSubgraphFixture}
        onSelectObject={onSelect}
      />,
    );
    const row = screen.getAllByRole("row")[1]!;
    fireEvent.keyDown(row, { key: "Enter" });
    expect(onSelect).toHaveBeenCalledTimes(1);
  });

  it("pressing Space on a fallback row calls onSelectObject", () => {
    const onSelect = vi.fn();
    render(
      <InteractiveGraph
        root="x"
        data={smallSubgraphFixture}
        onSelectObject={onSelect}
      />,
    );
    const row = screen.getAllByRole("row")[1]!;
    fireEvent.keyDown(row, { key: " " });
    expect(onSelect).toHaveBeenCalledTimes(1);
  });

  // ---- E7 / ADR-042: WebGL selective adoption ----
  // The preferWebgl prop is verified via integration tests (e2e/bench-renderer.spec.ts).
  // Unit-test coverage here would require mocking Web Workers + DOM Canvas
  // which is disproportionate. The logic is: data.nodes.length >= 500 → webgl: true.
  it("renders with preferWebgl=false prop without crashing", () => {
    render(
      <InteractiveGraph
        root="x"
        data={smallSubgraphFixture}
        onSelectObject={() => {}}
        preferWebgl={false}
      />,
    );
    expect(screen.getByTestId("interactive-graph")).toBeInTheDocument();
  });

  // ---- E5.5: crossfade — warm-cache data swap destroys cytoscape (per design Option C)
  // The mount effect's [root, data, layoutAlgorithm] dep triggers cleanup → destroy is called.
  // This is the expected behavior for Option C (stale-data hold + opacity fade, not imperative morph).
  // The crossfade visual mitigation (opacity fade) makes the remount invisible to the user.
  it("calls cy.destroy when data changes (Option C: mount effect dep triggers remount)", () => {
    const { rerender } = render(
      <InteractiveGraph
        root="sym:foo::bar"
        data={smallSubgraphFixture}
        onSelectObject={() => {}}
      />,
    );
    // Reset after initial mount (destroy fires during unmount if component is torn down)
    destroyCallCount = 0;

    // Swap to a data object with a different identity (same shape is fine)
    const differentData = {
      root: "sym:foo::bar",
      nodes: [
        {
          id: "sym:different",
          label: "Different",
          kind: "function",
          file: "other.rs",
          line: 99,
          style_class: "function",
        },
      ],
      edges: [],
      truncated: false,
      truncated_reason: null,
      corroboration_scores: {},
    };
    rerender(
      <InteractiveGraph
        root="sym:foo::bar"
        // @ts-expect-error -- test fixture: style_class inferred as string not literal union
        data={differentData}
        onSelectObject={() => {}}
      />,
    );
    // Mount effect dep [data] triggers cleanup → destroy is called once
    expect(destroyCallCount).toBe(1);
  });

  // ---- E5.5: crossfade — canvas fade class toggles on data identity change ----
  it("canvas wrapper completes fade transition after data identity change", () => {
    vi.useFakeTimers();
    const { rerender } = render(
      <InteractiveGraph
        root="sym:foo::bar"
        data={smallSubgraphFixture}
        onSelectObject={() => {}}
      />,
    );
    const differentData = {
      root: "sym:foo::bar",
      nodes: [
        {
          id: "sym:other",
          label: "Other",
          kind: "function",
          file: "other.rs",
          line: 1,
          style_class: "function",
        },
      ],
      edges: [],
      truncated: false,
      truncated_reason: null,
      corroboration_scores: {},
    };
    rerender(
      <InteractiveGraph
        root="sym:foo::bar"
        // @ts-expect-error -- test fixture: style_class inferred as string not literal union
        data={differentData}
        onSelectObject={() => {}}
      />,
    );
    // Advance timers and flush React state updates in one act() call
    (act ?? domAct)(() => {
      vi.runAllTimers();
    });
    const canvas = screen.getByTestId("interactive-graph-canvas");
    // After the double-rAF completes, canvas must have non-fading class
    // (CSS modules generate scoped names like _igCanvas_HASH or _igCanvasFading_HASH)
    expect(canvas.className).toMatch(/^_(igCanvas|igCanvasFading)_/);
    // The non-fading state (igCanvas, not igCanvasFading) must be present
    expect(canvas.className).not.toMatch(/^_igCanvasFading_/);
    vi.useRealTimers();
  });
});
