/**
 * Unit tests for the pure C4 level filter functions.
 */
import { describe, it, expect } from "vitest";
import {
  isGraphPerspective,
  filterArchitectureByLevel,
} from "./c4Levels";
import { architectureFixture } from "../mocks/architectureFixtures";

describe("isGraphPerspective", () => {
  it("returns true for 'graph'", () => {
    expect(isGraphPerspective("graph")).toBe(true);
  });
  it("returns false for any C4 level", () => {
    expect(isGraphPerspective("c4-context")).toBe(false);
    expect(isGraphPerspective("c4-container")).toBe(false);
    expect(isGraphPerspective("c4-component")).toBe(false);
    expect(isGraphPerspective("c4-code")).toBe(false);
  });
});

describe("filterArchitectureByLevel", () => {
  it("returns payload unchanged for 'graph'", () => {
    const result = filterArchitectureByLevel(architectureFixture, "graph");
    expect(result).toBe(architectureFixture);
  });

  it("c4-context: only system nodes survive", () => {
    const result = filterArchitectureByLevel(architectureFixture, "c4-context");
    expect(result.nodes.map((n) => n.kind)).toEqual(["system"]);
    // Edges to non-system nodes are pruned
    expect(result.edges).toHaveLength(0);
  });

  it("c4-container: system + container + child components survive", () => {
    const result = filterArchitectureByLevel(architectureFixture, "c4-container");
    const kinds = result.nodes.map((n) => n.kind).sort();
    expect(kinds).toEqual(expect.arrayContaining(["system", "container", "component"]));
  });

  it("c4-component: component nodes survive", () => {
    const result = filterArchitectureByLevel(architectureFixture, "c4-component");
    expect(result.nodes.map((n) => n.kind)).toEqual(expect.arrayContaining(["component"]));
  });

  it("c4-code: code nodes survive", () => {
    const result = filterArchitectureByLevel(architectureFixture, "c4-code");
    const kinds = result.nodes.map((n) => n.kind).sort();
    expect(kinds).toContain("code");
  });

  it("prunes dangling edges (edges whose endpoint was filtered out)", () => {
    const result = filterArchitectureByLevel(architectureFixture, "c4-context");
    const survivingNodeIds = new Set(result.nodes.map((n) => n.id));
    for (const edge of result.edges) {
      expect(survivingNodeIds.has(edge.source)).toBe(true);
      expect(survivingNodeIds.has(edge.target)).toBe(true);
    }
  });

  it("returns empty nodes/edges when payload is empty", () => {
    const empty = { ...architectureFixture, nodes: [], edges: [] };
    const result = filterArchitectureByLevel(empty, "c4-component");
    expect(result.nodes).toHaveLength(0);
    expect(result.edges).toHaveLength(0);
  });
});
