/**
 * Tests for the cytoscape adapter — `toCytoscapeElements`.
 *
 * The adapter is the boundary between the wire `SubgraphResponse`
 * (REST → zod) and the cytoscape `ElementsDefinition`. It must:
 * 1. Pass through node ids and labels faithfully.
 * 2. Mirror `style_class` into cytoscape `data.style_class` so the
 *    cytoscape stylesheet can style each bucket.
 * 3. Map REST `source`/`target` onto cytoscape edge `data.source` /
 *    `data.target` (cytoscape edges reference node ids).
 * 4. Tolerate empty inputs without throwing.
 */
import { describe, expect, it } from "vitest";

import { toCytoscapeElements } from "./adapter";
import { smallSubgraphFixture } from "../../mocks/subgraphFixtures";

describe("toCytoscapeElements", () => {
  it("returns a cytoscape ElementsDefinition shape", () => {
    const out = toCytoscapeElements(
      smallSubgraphFixture.nodes,
      smallSubgraphFixture.edges,
    );
    expect(out).toHaveProperty("nodes");
    expect(out).toHaveProperty("edges");
    expect(Array.isArray(out.nodes)).toBe(true);
    expect(Array.isArray(out.edges)).toBe(true);
  });

  it("preserves the style_class on every cytoscape node", () => {
    const out = toCytoscapeElements(
      smallSubgraphFixture.nodes,
      smallSubgraphFixture.edges,
    );
    for (const n of out.nodes) {
      expect(n.data).toHaveProperty("style_class");
      expect(n.data.style_class).toBeTruthy();
    }
  });

  it("maps REST edge source/target onto cytoscape edge data", () => {
    const out = toCytoscapeElements(
      smallSubgraphFixture.nodes,
      smallSubgraphFixture.edges,
    );
    for (const e of out.edges) {
      expect(typeof e.data.source).toBe("string");
      expect(typeof e.data.target).toBe("string");
      // Source + target must reference known node ids.
      const known = new Set(out.nodes.map((n) => String(n.data.id)));
      expect(known.has(String(e.data.source))).toBe(true);
      expect(known.has(String(e.data.target))).toBe(true);
    }
  });

  it("tolerates empty inputs", () => {
    const out = toCytoscapeElements([], []);
    expect(out.nodes).toEqual([]);
    expect(out.edges).toEqual([]);
  });
});
