/**
 * `SvgGraph` tests — Phase 8 acceptance criteria.
 *
 * 1. Layout-agnostic: receives nodes/edges with positions and
 *    renders them. The mock layout is exercised in detail.
 * 2. ARIA: the graph region has `role="complementary"` + an
 *    accessible name; the screen-reader table fallback is in
 *    the DOM.
 * 3. Interactions: clicking a node dispatches `onSelectObject`;
 *    keyboard (Enter) also dispatches.
 * 4. Pan + zoom controls: + / − / reset work, the graph state
 *    is updated through the visual transforms.
 * 5. Unknown shapes: the layout mock returns valid data even
 *    for a single node (no edges).
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { SvgGraph } from "./SvgGraph";
import { layoutFromSpotter, layoutFromIds } from "../../mocks/layoutMock";
import { spotterResultsFixture } from "../../mocks/fixtures";

describe("SvgGraph", () => {
  it("renders one <g role=button> per node", () => {
    const layout = layoutFromSpotter(spotterResultsFixture, { radius: 120 });
    render(<SvgGraph layout={layout} />);
    const canvas = screen.getByTestId("svg-graph-canvas");
    const nodes = within(canvas as HTMLElement).getAllByRole("button");
    expect(nodes.length).toBe(spotterResultsFixture.length);
  });

  it("renders one edge per layoutEdges entry", () => {
    const layout = layoutFromSpotter(spotterResultsFixture, { radius: 120 });
    render(<SvgGraph layout={layout} />);
    const edges = screen.getAllByTestId(/^graph-edge-/);
    // spotter fixture has 2 hits — edges from first to second = 1.
    expect(edges.length).toBeGreaterThanOrEqual(1);
  });

  it("has role=complementary and an accessible name", () => {
    const layout = layoutFromIds(["a", "b", "c"]);
    render(<SvgGraph layout={layout} ariaLabel="Call graph of foo" />);
    const region = screen.getByRole("complementary");
    expect(region).toHaveAttribute("aria-label", "Call graph of foo");
  });

  it("renders a screen-reader <table> fallback with all nodes", () => {
    const layout = layoutFromIds(["a", "b", "c"], {
      labelOf: (id) => `Label-${id}`,
    });
    render(<SvgGraph layout={layout} />);
    const table = screen.getByTestId("svg-graph-fallback");
    expect(table).toBeInTheDocument();
    expect(within(table).getByText("Label-a")).toBeInTheDocument();
    expect(within(table).getByText("Label-b")).toBeInTheDocument();
    expect(within(table).getByText("Label-c")).toBeInTheDocument();
  });

  it("clicking a node dispatches onSelectObject with the node id", async () => {
    const onSelect = vi.fn();
    const user = userEvent.setup();
    const layout = layoutFromIds(["a", "b"], { labelOf: (id) => id });
    render(<SvgGraph layout={layout} onSelectObject={onSelect} />);
    const node = screen.getByTestId("graph-node-a");
    await user.click(node);
    expect(onSelect).toHaveBeenCalledWith("a");
  });

  it("pressing Enter on a focused node dispatches onSelectObject", () => {
    const onSelect = vi.fn();
    const layout = layoutFromIds(["a", "b"], { labelOf: (id) => id });
    render(<SvgGraph layout={layout} onSelectObject={onSelect} />);
    const node = screen.getByTestId("graph-node-a");
    fireEvent.keyDown(node, { key: "Enter" });
    expect(onSelect).toHaveBeenCalledWith("a");
    fireEvent.keyDown(node, { key: " " });
    expect(onSelect).toHaveBeenCalledTimes(2);
  });

  it("highlights the selected node with data-selected=true", () => {
    const layout = layoutFromIds(["a", "b"], { labelOf: (id) => id });
    render(<SvgGraph layout={layout} selectedId="b" />);
    const sel = screen.getByTestId("graph-node-b");
    const other = screen.getByTestId("graph-node-a");
    expect(sel).toHaveAttribute("data-selected", "true");
    expect(other).toHaveAttribute("data-selected", "false");
  });

  it("renders an empty graph gracefully (1 node, 0 edges)", () => {
    const layout = layoutFromIds(["solo"], { labelOf: () => "solo" });
    render(<SvgGraph layout={layout} />);
    const region = screen.getByRole("complementary");
    expect(region).toHaveAttribute("data-node-count", "1");
    expect(region).toHaveAttribute("data-edge-count", "0");
  });

  it("drops edges that reference unknown nodes", () => {
    const layout = layoutFromIds(["a", "b"], {
      labelOf: (id) => id,
      edges: [
        { from: "a", to: "b" },
        { from: "a", to: "missing" },
        { from: "missing", to: "b" },
      ],
    });
    render(<SvgGraph layout={layout} />);
    const region = screen.getByRole("complementary");
    expect(region).toHaveAttribute("data-edge-count", "1");
  });

  it("zoom + reset controls mutate the viewport transform", () => {
    const layout = layoutFromIds(["a", "b"], { labelOf: (id) => id });
    const { container } = render(<SvgGraph layout={layout} />);
    const inner = container.querySelector("svg g[transform]");
    expect(inner).not.toBeNull();
    const before = inner?.getAttribute("transform") ?? "";
    expect(before).toMatch(/scale\(1\)/);
    // The zoom controls are inside an aria-hidden wrapper, so we
    // query by testid + text content rather than accessible name.
    const controls = screen.getByTestId("svg-graph-controls");
    const plus = within(controls).getByText("+");
    fireEvent.click(plus);
    const after = container.querySelector("svg g[transform]")?.getAttribute("transform") ?? "";
    expect(after).not.toEqual(before);
    // Reset
    const reset = within(controls).getByText("⟲");
    fireEvent.click(reset);
    const resetTransform = container.querySelector("svg g[transform]")?.getAttribute("transform") ?? "";
    expect(resetTransform).toMatch(/scale\(1\)/);
  });
});
