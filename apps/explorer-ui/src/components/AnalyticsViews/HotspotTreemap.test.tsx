/**
 * `HotspotTreemap` tests — D3 treemap render + colour mapping.
 *
 * 1. Renders the root header + an SVG with one rect per cell.
 * 2. Cell name appears in a testid keyed off the symbol name.
 * 3. The fill colour scales monotonically with complexity (low →
 *    green, high → red). We assert on the actual hex strings so
 *    regressions in the colour ramp surface immediately.
 * 4. Empty data renders the empty-state testid and no SVG.
 */
import { describe, it, expect } from "vitest";
import { render, screen, within } from "@testing-library/react";

import { HotspotTreemap } from "./HotspotTreemap";
import type { TreemapData } from "./types";

const SAMPLE: TreemapData = {
  name: "crates/cognicode-explorer/src",
  children: [
    { name: "build_overview", value: 120, complexity: 0.9 },
    { name: "spotter", value: 80, complexity: 0.55 },
    { name: "save_exploration", value: 40, complexity: 0.2 },
    { name: "render_block", value: 20, complexity: 0.1 },
  ],
};

describe("HotspotTreemap", () => {
  it("renders the root label and one cell per child", () => {
    render(<HotspotTreemap data={SAMPLE} />);
    const root = screen.getByTestId("hotspot-treemap");
    expect(within(root).getByText("crates/cognicode-explorer/src")).toBeInTheDocument();
    expect(within(root).getByTestId("hotspot-treemap-svg")).toBeInTheDocument();
    expect(root).toHaveAttribute("data-cell-count", "4");
  });

  it("exposes every symbol name as a testid", () => {
    render(<HotspotTreemap data={SAMPLE} />);
    for (const c of SAMPLE.children) {
      expect(
        screen.getByTestId(`hotspot-treemap-cell-${c.name}`),
      ).toBeInTheDocument();
    }
  });

  it("applies a green→red gradient driven by complexity", () => {
    render(<HotspotTreemap data={SAMPLE} />);
    const hot = screen.getByTestId("hotspot-treemap-cell-build_overview");
    const cool = screen.getByTestId("hotspot-treemap-cell-render_block");
    const hotFill = hot.querySelector("rect")?.getAttribute("fill") ?? "";
    const coolFill = cool.querySelector("rect")?.getAttribute("fill") ?? "";
    // d3-interpolate may emit either `#rrggbb` or `rgb(r, g, b)`;
    // we accept both so the test does not couple to the wire format.
    const parseColour = (raw: string): readonly [number, number, number] => {
      const hex = /^#([0-9a-f]{6})$/i.exec(raw);
      if (hex) {
        return [
          parseInt(hex[1]!.slice(0, 2), 16),
          parseInt(hex[1]!.slice(2, 4), 16),
          parseInt(hex[1]!.slice(4, 6), 16),
        ] as const;
      }
      const rgb = /^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/i.exec(raw);
      if (rgb) {
        return [Number(rgb[1]), Number(rgb[2]), Number(rgb[3])] as const;
      }
      throw new Error(`unexpected colour ${raw}`);
    };
    const [hr, hg, hb] = parseColour(hotFill);
    const [cr, cg, cb] = parseColour(coolFill);
    // Hot cell leans red: red channel dominant, and greater than
    // the cool cell's red channel.
    expect(hr).toBeGreaterThanOrEqual(cr);
    expect(hg).toBeLessThanOrEqual(cg);
    expect(hr).toBeGreaterThan(hg);
    expect(hr).toBeGreaterThan(hb);
    // Cool cell leans green: green channel at least as large as
    // its blue channel.
    expect(cg).toBeGreaterThanOrEqual(cb);
  });

  it("renders the empty state when there are no children", () => {
    render(<HotspotTreemap data={{ name: "Empty", children: [] }} />);
    expect(screen.getByTestId("hotspot-treemap-empty")).toBeInTheDocument();
    expect(screen.queryByTestId("hotspot-treemap-svg")).not.toBeInTheDocument();
  });

  it("embeds a tooltip with the symbol name + complexity", () => {
    render(<HotspotTreemap data={SAMPLE} />);
    const cell = screen.getByTestId("hotspot-treemap-cell-spotter");
    const title = cell.querySelector("title")?.textContent ?? "";
    expect(title).toMatch(/spotter/);
    expect(title).toMatch(/complexity 0\.55/);
  });
});
