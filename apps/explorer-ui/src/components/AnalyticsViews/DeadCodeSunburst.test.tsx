/**
 * `DeadCodeSunburst` tests — partition + colour rules.
 *
 * 1. Renders one segment per child, with dead/alive counts shown.
 * 2. Dead segments are coloured from the warm palette (orange/red
 *    hexes); alive segments use the cool/neutral palette (slate
 *    greys). We assert on the actual hex strings to catch
 *    palette regressions.
 * 3. Empty data renders the empty-state testid and no SVG.
 * 4. Each segment carries a tooltip with the module path and
 *    alive/dead status.
 */
import { describe, it, expect } from "vitest";
import { render, screen, within } from "@testing-library/react";

import { DeadCodeSunburst } from "./DeadCodeSunburst";
import type { SunburstData } from "./types";

const DEAD_HEXES = new Set(["#f97316", "#ef4444", "#dc2626", "#b91c1c"]);
const ALIVE_HEXES = new Set(["#64748b", "#94a3b8", "#cbd5e1", "#475569"]);

const SAMPLE: SunburstData = {
  name: "crates/cognicode-explorer/src",
  children: [
    { name: "api", size: 80, alive: true },
    { name: "db", size: 60, alive: true },
    { name: "lib", size: 50, alive: true },
    { name: "orphan_a", size: 30, alive: false },
    { name: "orphan_b", size: 25, alive: false },
    { name: "legacy_mod", size: 20, alive: false },
  ],
};

describe("DeadCodeSunburst", () => {
  it("renders the root label, counts, and one segment per child", () => {
    render(<DeadCodeSunburst data={SAMPLE} />);
    const root = screen.getByTestId("dead-code-sunburst");
    expect(within(root).getByText("crates/cognicode-explorer/src")).toBeInTheDocument();
    expect(within(root).getByTestId("dead-code-sunburst-counts")).toHaveTextContent(
      /3 dead \/ 3 alive/,
    );
    expect(root).toHaveAttribute("data-segment-count", "6");
  });

  it("colours dead segments from the warm palette and alive from the cool palette", () => {
    render(<DeadCodeSunburst data={SAMPLE} />);
    for (const c of SAMPLE.children) {
      const seg = screen.getByTestId(`dead-code-sunburst-segment-${c.name}`);
      const path = seg.querySelector("path");
      const fill = path?.getAttribute("fill") ?? "";
      if (c.alive) {
        expect(ALIVE_HEXES.has(fill)).toBe(true);
      } else {
        expect(DEAD_HEXES.has(fill)).toBe(true);
      }
      // The data-alive attribute on the wrapping <g> reflects the
      // source data so screen readers can pivot on it.
      expect(seg).toHaveAttribute("data-alive", c.alive ? "true" : "false");
    }
  });

  it("renders the empty state when there are no children", () => {
    render(<DeadCodeSunburst data={{ name: "Empty", children: [] }} />);
    expect(screen.getByTestId("dead-code-sunburst-empty")).toBeInTheDocument();
    expect(screen.queryByTestId("dead-code-sunburst-svg")).not.toBeInTheDocument();
  });

  it("embeds a tooltip with the module path and alive/dead status", () => {
    render(<DeadCodeSunburst data={SAMPLE} />);
    const dead = screen.getByTestId("dead-code-sunburst-segment-orphan_a");
    const alive = screen.getByTestId("dead-code-sunburst-segment-api");
    expect(dead.querySelector("title")?.textContent ?? "").toMatch(/orphan_a — dead/);
    expect(alive.querySelector("title")?.textContent ?? "").toMatch(/api — alive/);
  });
});
