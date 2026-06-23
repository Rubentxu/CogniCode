/**
 * `viewKind.ts` tests — E1.5 render strategy resolution.
 *
 * Tests resolveRenderStrategy() for:
 * 1. Graph ViewKinds → registry path with "graph"
 * 2. Built-in views with blocks → blocks path (renderer_kind ignored)
 * 3. ViewSpecs with explicit renderer_kind (no blocks) → registry path
 * 4. Edge cases: empty blocks, nullish renderer_kind, unknown kinds
 */
import { describe, it, expect } from "vitest";

import {
  GRAPH_KINDS,
  isGraphViewKind,
  resolveRenderStrategy,
} from "./viewKind";
import type { ContextualView } from "../../api/types";

describe("GRAPH_KINDS", () => {
  it("contains 5 graph-shaped ViewKinds", () => {
    expect(GRAPH_KINDS.size).toBe(5);
  });

  it("contains call_graph, dependency_graph, data_flow, impact_radius, seam_map", () => {
    expect(GRAPH_KINDS.has("call_graph")).toBe(true);
    expect(GRAPH_KINDS.has("dependency_graph")).toBe(true);
    expect(GRAPH_KINDS.has("data_flow")).toBe(true);
    expect(GRAPH_KINDS.has("impact_radius")).toBe(true);
    expect(GRAPH_KINDS.has("seam_map")).toBe(true);
  });
});

describe("isGraphViewKind", () => {
  it("returns true for call_graph", () => {
    expect(isGraphViewKind("call_graph")).toBe(true);
  });

  it("returns true for dependency_graph", () => {
    expect(isGraphViewKind("dependency_graph")).toBe(true);
  });

  it("returns true for data_flow", () => {
    expect(isGraphViewKind("data_flow")).toBe(true);
  });

  it("returns true for impact_radius", () => {
    expect(isGraphViewKind("impact_radius")).toBe(true);
  });

  it("returns true for seam_map", () => {
    expect(isGraphViewKind("seam_map")).toBe(true);
  });

  it("returns false for vertical_slice", () => {
    expect(isGraphViewKind("vertical_slice")).toBe(false);
  });

  it("returns false for undefined", () => {
    expect(isGraphViewKind(undefined)).toBe(false);
  });

  it("returns false for unknown kinds", () => {
    expect(isGraphViewKind("some_unknown_kind")).toBe(false);
  });
});

function makeView(partial: Partial<ContextualView>): ContextualView {
  return {
    object_id: "sym:test",
    view_id: "test-view",
    title: "Test View",
    view_kind: "vertical_slice",
    blocks: [],
    relations: [],
    evidence: [],
    findings: [],
    renderer_kind: "json",
    ...partial,
  };
}

describe("resolveRenderStrategy", () => {
  describe("graph ViewKind → registry (graph)", () => {
    const graphKinds = [
      "call_graph",
      "dependency_graph",
      "data_flow",
      "impact_radius",
      "seam_map",
    ] as const;

    for (const vk of graphKinds) {
      it(`${vk} → registry("graph")`, () => {
        const result = resolveRenderStrategy(makeView({ view_kind: vk }));
        expect(result).toEqual({ kind: "registry", rendererKind: "graph" });
      });
    }
  });

  describe("built-in view with blocks → blocks (renderer_kind ignored)", () => {
    it("vertical_slice with blocks → blocks", () => {
      const result = resolveRenderStrategy(
        makeView({
          view_kind: "vertical_slice",
          renderer_kind: "composite",
          blocks: [{ id: "identity", title: "Identity", body: {} }],
        }),
      );
      expect(result).toEqual({ kind: "blocks" });
    });

    it("overview with blocks → blocks", () => {
      const result = resolveRenderStrategy(
        makeView({
          view_kind: "overview",
          renderer_kind: "json",
          blocks: [{ id: "identity", title: "Identity", body: {} }],
        }),
      );
      expect(result).toEqual({ kind: "blocks" });
    });

    it("empty blocks still → blocks (edge case)", () => {
      const result = resolveRenderStrategy(
        makeView({
          view_kind: "vertical_slice",
          blocks: [],
        }),
      );
      expect(result).toEqual({ kind: "blocks" });
    });
  });

  describe("ViewSpec body (no blocks) with explicit renderer_kind → registry", () => {
    it("ViewSpec with renderer_kind=table → registry(table)", () => {
      const result = resolveRenderStrategy(
        makeView({
          view_kind: undefined,
          renderer_kind: "table",
          blocks: undefined,
        }),
      );
      expect(result).toEqual({ kind: "registry", rendererKind: "table" });
    });

    it("ViewSpec with renderer_kind=code → registry(code)", () => {
      const result = resolveRenderStrategy(
        makeView({
          view_kind: undefined,
          renderer_kind: "code",
          blocks: undefined,
        }),
      );
      expect(result).toEqual({ kind: "registry", rendererKind: "code" });
    });
  });

  describe("no blocks, no renderer_kind → blocks (default)", () => {
    it("no blocks, no renderer_kind → blocks", () => {
      const result = resolveRenderStrategy(
        makeView({
          view_kind: undefined,
          blocks: undefined,
        }),
      );
      expect(result).toEqual({ kind: "blocks" });
    });

    it("no blocks, renderer_kind=json → blocks (json is default)", () => {
      const result = resolveRenderStrategy(
        makeView({
          view_kind: undefined,
          renderer_kind: "json",
          blocks: undefined,
        }),
      );
      expect(result).toEqual({ kind: "blocks" });
    });
  });
});
