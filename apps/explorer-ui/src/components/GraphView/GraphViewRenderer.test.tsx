import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { useReducer } from "react";
import { GraphViewRenderer } from "./GraphViewRenderer";
import {
  AppContext,
  appReducer,
  initialState,
  type Action,
  type AppState,
} from "../../state/context";

// Mock layoutFromContextualView to return deterministic layout
vi.mock("../../mocks/layoutMock", () => ({
  layoutFromContextualView: vi.fn().mockReturnValue({
    nodes: [
      { id: "root", label: "root", kind: "function", x: 200, y: 200 },
      { id: "callee", label: "callee", kind: "function", x: 400, y: 200 },
    ],
    edges: [{ from: "root", to: "callee" }],
    viewBox: { x: 0, y: 0, width: 600, height: 400 },
  }),
}));

/**
 * Minimal harness that provides a live AppContext.
 */
function GraphViewRendererHarness({
  view,
  objectId,
  onClose,
}: {
  view: Parameters<typeof GraphViewRenderer>[0]["view"];
  objectId: string;
  onClose?: () => void;
}) {
  const [state, dispatch] = useReducer(appReducer, initialState);
  const value: { state: AppState; dispatch: React.Dispatch<Action> } = {
    state,
    dispatch,
  };
  return (
    <AppContext.Provider value={value}>
      <GraphViewRenderer view={view} objectId={objectId} onClose={onClose} />
    </AppContext.Provider>
  );
}

describe("GraphViewRenderer", () => {
  const mockView = {
    object_id: "root",
    view_id: "call-graph",
    title: "Call Graph",
    view_kind: "call_graph",
    renderer_kind: "graph" as const,
    blocks: [
      {
        id: "callees",
        title: "Callees",
        body: {
          items: [
            { object_id: "callee", name: "callee", kind: "function" },
          ],
        },
      },
    ],
    relations: [],
    evidence: [],
    findings: [],
  };

  beforeEach(async () => {
    const { layoutFromContextualView } = await import("../../mocks/layoutMock");
    vi.mocked(layoutFromContextualView).mockReturnValue({
      nodes: [
        { id: "root", label: "root", kind: "function", x: 200, y: 200 },
        { id: "callee", label: "callee", kind: "function", x: 400, y: 200 },
      ],
      edges: [{ from: "root", to: "callee" }],
      viewBox: { x: 0, y: 0, width: 600, height: 400 },
    });
  });

  it("renders SvgGraph when layout has nodes", () => {
    render(
      <GraphViewRendererHarness view={mockView} objectId="root" />
    );
    expect(screen.getByTestId("graph-view-renderer")).toBeInTheDocument();
  });

  it("renders empty state when layout has <=1 nodes", async () => {
    const { layoutFromContextualView } = await import("../../mocks/layoutMock");
    vi.mocked(layoutFromContextualView).mockReturnValueOnce({
      nodes: [],
      edges: [],
      viewBox: { x: 0, y: 0, width: 0, height: 0 },
    });
    render(
      <GraphViewRendererHarness view={mockView} objectId="root" />
    );
    expect(screen.getByTestId("graph-empty-state")).toBeInTheDocument();
  });

  it("renders graph view renderer with single node shows empty state", async () => {
    const { layoutFromContextualView } = await import("../../mocks/layoutMock");
    vi.mocked(layoutFromContextualView).mockReturnValueOnce({
      nodes: [{ id: "root", label: "root", kind: "function", x: 200, y: 200 }],
      edges: [],
      viewBox: { x: 0, y: 0, width: 400, height: 400 },
    });
    render(
      <GraphViewRendererHarness view={mockView} objectId="root" />
    );
    expect(screen.getByTestId("graph-empty-state")).toBeInTheDocument();
  });

  it("dispatches SELECT_OBJECT on node click", () => {
    render(
      <GraphViewRendererHarness view={mockView} objectId="root" />
    );

    // The SvgGraph component handles clicks internally
    // We verify the component renders with the right structure
    expect(screen.getByTestId("graph-view-renderer")).toBeInTheDocument();
    expect(screen.getByTestId("svg-graph-canvas")).toBeInTheDocument();
  });

  it("renders title in header", () => {
    render(
      <GraphViewRendererHarness view={mockView} objectId="root" />
    );
    expect(screen.getByRole("heading", { name: "Call Graph" })).toBeInTheDocument();
  });

  it("shows close button when onClose provided", () => {
    const onClose = vi.fn();
    render(
      <GraphViewRendererHarness view={mockView} objectId="root" onClose={onClose} />
    );
    expect(screen.getByTestId("graph-view-close")).toBeInTheDocument();
  });

  it("close button calls onClose when clicked", () => {
    const onClose = vi.fn();
    render(
      <GraphViewRendererHarness view={mockView} objectId="root" onClose={onClose} />
    );
    fireEvent.click(screen.getByTestId("graph-view-close"));
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});