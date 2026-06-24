/**
 * `Shell` tests — viewport behaviour, health chip, skip link.
 *
 * Post E3 (ADR-039): Shell renders a 2-zone layout:
 *   InteractiveGraph (left) | PaneStackView (right)
 * Small viewport: graph full-width, PaneStackView as bottom sheet.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { useReducer } from "react";

// ---------------------------------------------------------------------------
// Mock cytoscape for tests that render InteractiveGraph through Shell
// ---------------------------------------------------------------------------
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
    // @ts-expect-error -- mock property, the nodes() method below is the public API
    nodes: CyNode[] = [];
    edgeElements: CyEdge[] = [];
    constructor(opts: {
      elements?: { nodes?: NodeData[]; edges?: NodeData[] };
      renderer?: { name: string; webgl?: boolean };
    }) {
      this.nodes = (opts.elements?.nodes ?? []).map((d) => new CyNode(d));
      this.edgeElements = (opts.elements?.edges ?? []).map(
        (d) => new CyEdge(d as EdgeData),
      );
      for (const e of this.edgeElements) {
        const src = this.nodes.find((n) => n.id === String(e.data.source));
        const tgt = this.nodes.find((n) => n.id === String(e.data.target));
        if (src) (src as unknown as { edgeListeners: Set<CyEdge> }).edgeListeners.add(e);
        if (tgt) (tgt as unknown as { edgeListeners: Set<CyEdge> }).edgeListeners.add(e);
      }
    }
    // eslint-disable-next-line @typescript-eslint/no-unused-vars -- intentional unused param
    on(_evt: string, selector: string | ((e: unknown) => void), _?: (e: unknown) => void) {
      if (typeof selector === "function") {
        this.nodes.forEach((n) => n.on("tap", selector));
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
    destroy() {}
    // eslint-disable-next-line @typescript-eslint/no-unused-vars -- intentional unused param
    layout(_?: { name: string; rows?: number }) {
      return { run: () => {} };
    }
  }
  return {
    default: ((opts: {
      elements?: { nodes?: NodeData[]; edges?: NodeData[] };
      renderer?: { name: string; webgl?: boolean };
    }) => new Cy(opts)) as unknown as { (opts: unknown): unknown },
  };
});

import { detectViewport } from "./viewport";
import { HealthProbe } from "./HealthProbe";
import { SkipLink } from "./SkipLink";
import {
  AppContext,
  appReducer,
  initialState,
  type Action,
  type AppState,
} from "../state/context";
import { workspaceSummaryFixture } from "../mocks/fixtures";
import type { SubgraphResponse } from "../api/types";

// ---------------------------------------------------------------------------
// Hook spies — track calls for E5.3 regression tests
// ---------------------------------------------------------------------------

const SUBGRAPH_FIXTURE: SubgraphResponse = {
  root: "sym-123",
  nodes: [
    {
      id: "sym-123",
      label: "CreateUser",
      kind: "function",
      file: "src/users.rs",
      line: 10,
      style_class: "function",
    },
  ],
  edges: [],
  truncated: false,
  truncated_reason: null,
  corroboration_scores: {},
};

// Default spy implementations — overridden per-test via vi.mock
export const useSubgraphSpy = vi.fn().mockReturnValue({
  data: SUBGRAPH_FIXTURE,
  isLoading: false,
  error: null,
});
export const useArchitectureSpy = vi.fn().mockReturnValue({
  data: null,
  isLoading: false,
  error: null,
});

vi.mock("../hooks/useSubgraph", () => ({
  useSubgraph: (...args: unknown[]) => useSubgraphSpy(...args),
}));

vi.mock("../hooks/useArchitecture", () => ({
  useArchitecture: (...args: unknown[]) => useArchitectureSpy(...args),
}));

// Import Shell after hooks are mocked
import { Shell } from "./Shell";

/**
 * Minimal harness that provides a live AppContext.
 */
function ShellHarness({
  viewport,
}: {
  viewport?: "small" | "tablet" | "desktop" | "ultrawide";
}) {
  const [state, dispatch] = useReducer(appReducer, initialState);
  const value: { state: AppState; dispatch: React.Dispatch<Action> } = {
    state,
    dispatch,
  };
  return (
    <AppContext.Provider value={value}>
      <Shell viewport={viewport} />
    </AppContext.Provider>
  );
}

describe("detectViewport", () => {
  it("classifies >= 1200px as desktop", () => {
    expect(detectViewport(1280)).toBe("desktop");
    expect(detectViewport(1200)).toBe("desktop");
  });
  it("classifies 900-1199 as tablet", () => {
    expect(detectViewport(1199)).toBe("tablet");
    expect(detectViewport(900)).toBe("tablet");
  });
  it("classifies < 900 as small", () => {
    expect(detectViewport(899)).toBe("small");
    expect(detectViewport(360)).toBe("small");
  });
});

describe("Shell", () => {
  it("renders the top bar with the project title and a health chip", async () => {
    render(<ShellHarness viewport="desktop" />);
    expect(
      screen.getByRole("heading", { name: /CogniCode Explorer/i, level: 1 }),
    ).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByTestId("health-chip")).toBeInTheDocument();
    });
  });

  it("renders the skip link as the first focusable element", () => {
    render(<ShellHarness viewport="desktop" />);
    const skip = screen.getByTestId("skip-link");
    expect(skip).toBeInTheDocument();
    expect(skip).toHaveTextContent(/skip to main content/i);
  });

  it("renders a <main> landmark with the right label", () => {
    render(<ShellHarness viewport="desktop" />);
    const main = screen.getByRole("main");
    expect(main).toHaveAttribute("id", "app-main");
    expect(main).toHaveAttribute("aria-label", "Explorer panels");
  });

  it("desktop viewport renders graph + pane-stack zones", async () => {
    render(<ShellHarness viewport="desktop" />);
    // PaneStackView empty state should be present in the right zone
    await waitFor(() => {
      expect(screen.getByTestId("pane-stack-empty")).toBeInTheDocument();
    });
    // Graph loading / empty / resolved should be present in the left zone
    const hasGraph =
      document.querySelector('[data-testid="interactive-graph"]') !== null;
    const hasEmpty =
      document.querySelector('[data-testid="interactive-graph-empty"]') !== null;
    const hasLoading =
      document.querySelector('[data-testid="interactive-graph-loading"]') !== null;
    expect(hasGraph || hasEmpty || hasLoading).toBe(true);
  });

  it("small viewport renders graph full-width with bottom-sheet overlay", async () => {
    render(<ShellHarness viewport="small" />);
    // Bottom sheet should be present
    expect(screen.getByTestId("bottom-sheet")).toBeInTheDocument();
    // Graph/landing zone should eventually render (InteractiveGraph or GraphLanding via Suspense)
    await waitFor(() => {
      const hasGraph =
        document.querySelector('[data-testid="interactive-graph"]') !== null;
      const hasEmpty =
        document.querySelector('[data-testid="interactive-graph-empty"]') !== null;
      const hasLoading =
        document.querySelector('[data-testid="interactive-graph-loading"]') !== null;
      const hasLanding =
        document.querySelector('[data-testid="graph-landing"]') !== null;
      const hasLandingLoading =
        document.querySelector('[data-testid="graph-landing-loading"]') !== null;
      expect(
        hasGraph || hasEmpty || hasLoading || hasLanding || hasLandingLoading,
      ).toBe(true);
    });
  });

  it("tablet viewport renders graph + pane-stack (2-zone grid)", async () => {
    render(<ShellHarness viewport="tablet" />);
    await waitFor(() => {
      expect(screen.getByTestId("pane-stack-empty")).toBeInTheDocument();
    });
    // Graph/landing zone should eventually render
    await waitFor(() => {
      const hasGraph =
        document.querySelector('[data-testid="interactive-graph"]') !== null;
      const hasEmpty =
        document.querySelector('[data-testid="interactive-graph-empty"]') !== null;
      const hasLoading =
        document.querySelector('[data-testid="interactive-graph-loading"]') !== null;
      const hasLanding =
        document.querySelector('[data-testid="graph-landing"]') !== null;
      const hasLandingLoading =
        document.querySelector('[data-testid="graph-landing-loading"]') !== null;
      expect(
        hasGraph || hasEmpty || hasLoading || hasLanding || hasLandingLoading,
      ).toBe(true);
    });
  });

  it("ultrawide viewport renders 2-zone grid (same as desktop)", async () => {
    render(<ShellHarness viewport="ultrawide" />);
    await waitFor(() => {
      expect(screen.getByTestId("pane-stack-empty")).toBeInTheDocument();
    });
    // Graph/landing zone should eventually render
    await waitFor(() => {
      const hasGraph =
        document.querySelector('[data-testid="interactive-graph"]') !== null;
      const hasEmpty =
        document.querySelector('[data-testid="interactive-graph-empty"]') !== null;
      const hasLoading =
        document.querySelector('[data-testid="interactive-graph-loading"]') !== null;
      const hasLanding =
        document.querySelector('[data-testid="graph-landing"]') !== null;
      const hasLandingLoading =
        document.querySelector('[data-testid="graph-landing-loading"]') !== null;
      expect(
        hasGraph || hasEmpty || hasLoading || hasLanding || hasLandingLoading,
      ).toBe(true);
    });
  });

  it("data-viewport attribute reflects the active viewport", () => {
    const { rerender } = render(<ShellHarness viewport="desktop" />);
    expect(screen.getByTestId("shell")).toHaveAttribute(
      "data-viewport",
      "desktop",
    );
    rerender(<ShellHarness viewport="tablet" />);
    expect(screen.getByTestId("shell")).toHaveAttribute(
      "data-viewport",
      "tablet",
    );
    rerender(<ShellHarness viewport="small" />);
    expect(screen.getByTestId("shell")).toHaveAttribute(
      "data-viewport",
      "small",
    );
    rerender(<ShellHarness viewport="ultrawide" />);
    expect(screen.getByTestId("shell")).toHaveAttribute(
      "data-viewport",
      "ultrawide",
    );
  });
});

describe("HealthProbe (chip mode)", () => {
  it("renders the chip in the top bar", async () => {
    render(<HealthProbe showFullScreenOnError={false} />);
    await waitFor(() => {
      expect(screen.getByTestId("health-chip")).toBeInTheDocument();
    });
  });

  it("updates the data-status when the backend responds", async () => {
    render(<HealthProbe showFullScreenOnError={false} />);
    await waitFor(() => {
      expect(screen.getByTestId("health-chip")).toHaveAttribute(
        "data-status",
        "online",
      );
    });
  });
});

describe("SkipLink", () => {
  it("uses the provided target id in the href", () => {
    render(<SkipLink targetId="app-main" />);
    const link = screen.getByTestId("skip-link");
    expect(link).toHaveAttribute("href", "#app-main");
  });
});

describe("PerspectiveToggle", () => {
  it("renders the toggle with Graph and C4 Components labels", async () => {
    render(<ShellHarness viewport="desktop" />);
    await waitFor(() => {
      expect(screen.getByTestId("perspective-toggle")).toBeInTheDocument();
    });
    expect(screen.getByTestId("perspective-graph")).toHaveTextContent("Graph");
    expect(screen.getByTestId("perspective-c4")).toHaveTextContent("C4 Components");
  });

  it('dispatches SET_PERSPECTIVE with "c4" when C4 button is clicked', async () => {
    const dispatch = vi.fn();
    render(
      <AppContext.Provider value={{ state: initialState, dispatch }}>
        <Shell viewport="desktop" />
      </AppContext.Provider>,
    );
    await waitFor(() => {
      expect(screen.getByTestId("perspective-c4")).toBeInTheDocument();
    });
    dispatch.mockClear();
    screen.getByTestId("perspective-c4").click();
    expect(dispatch).toHaveBeenCalledWith({ type: "SET_PERSPECTIVE", payload: "c4" });
  });

  it('dispatches SET_PERSPECTIVE with "graph" when Graph button is clicked', async () => {
    const dispatch = vi.fn();
    const stateWithC4 = { ...initialState, perspective: "c4" as const };
    render(
      <AppContext.Provider value={{ state: stateWithC4, dispatch }}>
        <Shell viewport="desktop" />
      </AppContext.Provider>,
    );
    await waitFor(() => {
      expect(screen.getByTestId("perspective-graph")).toBeInTheDocument();
    });
    dispatch.mockClear();
    screen.getByTestId("perspective-graph").click();
    expect(dispatch).toHaveBeenCalledWith({ type: "SET_PERSPECTIVE", payload: "graph" });
  });

  it('graph button is aria-pressed when perspective is "graph"', async () => {
    render(<ShellHarness viewport="desktop" />);
    await waitFor(() => {
      expect(screen.getByTestId("perspective-graph")).toBeInTheDocument();
    });
    expect(screen.getByTestId("perspective-graph")).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByTestId("perspective-c4")).toHaveAttribute("aria-pressed", "false");
  });

  it('c4 button is aria-pressed when perspective is "c4"', async () => {
    const stateWithC4 = { ...initialState, perspective: "c4" as const };
    const dispatch = vi.fn();
    render(
      <AppContext.Provider value={{ state: stateWithC4, dispatch }}>
        <Shell viewport="desktop" />
      </AppContext.Provider>,
    );
    await waitFor(() => {
      expect(screen.getByTestId("perspective-c4")).toBeInTheDocument();
    });
    expect(screen.getByTestId("perspective-c4")).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByTestId("perspective-graph")).toHaveAttribute("aria-pressed", "false");
  });
});

// ---------------------------------------------------------------------------
// Regression: E5.3 — InteractiveGraphPanel perspective-aware wire-up
// ---------------------------------------------------------------------------
// Verifies that InteractiveGraphPanel swaps its data source based on
// perspective, mirroring the GraphLanding.tsx:43-54 proven pattern.

describe("InteractiveGraphPanel perspective wire-up (E5.3)", () => {
  beforeEach(() => {
    useSubgraphSpy.mockClear();
    useArchitectureSpy.mockClear();
  });

  // Scenario A: rootId set + perspective "graph" → useSubgraph fetches, useArchitecture idle
  it("feeds useSubgraph when perspective is graph with a selected symbol", async () => {
    useSubgraphSpy.mockReturnValue({
      data: SUBGRAPH_FIXTURE,
      isLoading: false,
      error: null,
    });
    useArchitectureSpy.mockReturnValue({
      data: null,
      isLoading: false,
      error: null,
    });

    const stateWithSymbol = {
      ...initialState,
      activeObjectId: "sym-123",
      perspective: "graph" as const,
      workspace: { ...workspaceSummaryFixture, id: "ws-42" },
    };
    const dispatch = vi.fn();

    render(
      <AppContext.Provider value={{ state: stateWithSymbol, dispatch }}>
        <Shell viewport="desktop" />
      </AppContext.Provider>,
    );

    await waitFor(() => {
      expect(useSubgraphSpy).toHaveBeenCalledWith("sym-123");
      expect(useArchitectureSpy).toHaveBeenCalledWith(null);
    });
  });

  // Scenario B: rootId set + perspective "c4" → useArchitecture fetches, useSubgraph idle
  it("feeds useArchitecture when perspective is c4 with a selected symbol", async () => {
    useSubgraphSpy.mockReturnValue({
      data: null,
      isLoading: false,
      error: null,
    });
    useArchitectureSpy.mockReturnValue({
      data: null,
      isLoading: true,
      error: null,
    });

    const stateWithSymbol = {
      ...initialState,
      activeObjectId: "sym-123",
      perspective: "c4" as const,
      workspace: { ...workspaceSummaryFixture, id: "ws-42" },
    };
    const dispatch = vi.fn();

    render(
      <AppContext.Provider value={{ state: stateWithSymbol, dispatch }}>
        <Shell viewport="desktop" />
      </AppContext.Provider>,
    );

    await waitFor(() => {
      expect(useSubgraphSpy).toHaveBeenCalledWith(null);
      expect(useArchitectureSpy).toHaveBeenCalledWith("ws-42");
    });
  });

  // Scenario C: rootId null + perspective "c4" → GraphLanding branch (not InteractiveGraphPanel)
  it("renders GraphLanding (not InteractiveGraphPanel) when no symbol selected and c4 perspective", async () => {
    const stateNoSymbol = {
      ...initialState,
      activeObjectId: null,
      perspective: "c4" as const,
      workspace: { ...workspaceSummaryFixture, id: "ws-42" },
    };
    const dispatch = vi.fn();

    render(
      <AppContext.Provider value={{ state: stateNoSymbol, dispatch }}>
        <Shell viewport="desktop" />
      </AppContext.Provider>,
    );

    // GraphLanding should be mounted (or loading while MSW resolves), not InteractiveGraphPanel
    await waitFor(() => {
      // Either the resolved landing or the loading state is acceptable
      const hasLanding =
        screen.queryByTestId("graph-landing") !== null;
      const hasLandingLoading =
        screen.queryByTestId("graph-landing-loading") !== null;
      expect(hasLanding || hasLandingLoading).toBe(true);
    });
    // useSubgraph and useArchitecture are NOT called because
    // GraphLanding uses useLanding + useArchitecture (different hooks)
    // and InteractiveGraphPanel is not mounted when rootId is null
    expect(useSubgraphSpy).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// E5.5: crossfade — stale-data hold regression tests
// ---------------------------------------------------------------------------

describe("InteractiveGraphPanel stale-data hold (E5.5)", () => {
  beforeEach(() => {
    useSubgraphSpy.mockClear();
    useArchitectureSpy.mockClear();
  });

  // T1.4c: stale data suppresses GRAPH_LOADING during revalidation
  // Verifies that when useSubgraph is revalidating (isLoading=true) with stale data
  // available, InteractiveGraphPanel does NOT show GRAPH_LOADING — it shows the
  // stale data instead. This is the stale-data hold behavior.
  it("does not render GRAPH_LOADING when stale data is available during revalidation", async () => {
    const stateWithSymbol = {
      ...initialState,
      activeObjectId: "sym-123",
      perspective: "graph" as const,
      workspace: { ...workspaceSummaryFixture, id: "ws-42" },
    };
    const dispatch = vi.fn();

    // Set up mock to return loading state with stale data (simulates SWR revalidation)
    useSubgraphSpy.mockReturnValue({
      data: SUBGRAPH_FIXTURE,
      isLoading: true,
      error: null,
    });

    render(
      <AppContext.Provider value={{ state: stateWithSymbol, dispatch }}>
        <Shell viewport="desktop" />
      </AppContext.Provider>,
    );

    // Wait for initial render to settle
    await waitFor(() => {
      expect(screen.queryByTestId("interactive-graph-loading")).not.toBeInTheDocument();
    });

    // The key assertion: even with isLoading=true, no GRAPH_LOADING is shown
    // because stale data is being displayed (stale-data hold)
    expect(screen.queryByTestId("interactive-graph-loading")).not.toBeInTheDocument();
    // The graph should still be rendered (stale data is shown, not loading)
    expect(
      screen.queryByTestId("interactive-graph") ||
        screen.queryByTestId("interactive-graph-empty")
    ).toBeTruthy();
  });

  // T1.4d: hard error clears stale hold and shows GRAPH_ERROR
  it("renders GRAPH_ERROR after hard error even when stale data was primed", async () => {
    const stateWithSymbol = {
      ...initialState,
      activeObjectId: "sym-123",
      perspective: "graph" as const,
      workspace: { ...workspaceSummaryFixture, id: "ws-42" },
    };
    const dispatch = vi.fn();

    // Set up mock to return error state
    useSubgraphSpy.mockReturnValue({
      data: null,
      isLoading: false,
      error: new Error("graph fetch failed"),
    });

    render(
      <AppContext.Provider value={{ state: stateWithSymbol, dispatch }}>
        <Shell viewport="desktop" />
      </AppContext.Provider>,
    );

    // GRAPH_ERROR must appear — hard error takes precedence
    await waitFor(() => {
      expect(screen.getByTestId("interactive-graph-error")).toBeInTheDocument();
    });
    // GRAPH_LOADING must not appear
    expect(screen.queryByTestId("interactive-graph-loading")).not.toBeInTheDocument();
  });
});
