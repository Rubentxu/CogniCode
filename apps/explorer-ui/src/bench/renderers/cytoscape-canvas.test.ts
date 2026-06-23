import {
  describe,
  it,
  expect,
  beforeEach,
  vi,
} from "vitest";

vi.mock("cytoscape", () => {
  // Capture the registry via closure so it survives the mock factory.
  const registry = new Map<string, { id: string; incident: number }>();
  const selections = new Set<string>();

  (globalThis as Record<string, unknown>).__cytoscapeMockRegistry =
    registry;
  (globalThis as Record<string, unknown>).__cytoscapeMockSelections =
    selections;

  return {
    default: () => {
      const cyId = `cy-${Date.now()}-${Math.random()}`;
      return {
        id: cyId,
        nodes: () => ({ length: registry.size }),
        edges: () => ({ length: registry.size }),
        getElementById: (id: string) => {
          const node = registry.get(id);
          const length = node ? 1 : 0;
          return {
            length,
            select: () => {
              if (node) selections.add(id);
            },
            selected: () => selections.has(id),
            connectedEdges: () => ({ length: node?.incident ?? 0 }),
          };
        },
        panBy: () => undefined,
        zoom: () => undefined,
        center: () => undefined,
        fit: () => undefined,
        layout: () => ({
          on: (_evt: string, cb: () => void) => {
            cb();
            return { run: () => undefined };
          },
          run: () => undefined,
        }),
        destroy: () => undefined,
      };
    },
  };
});

import {
  CytoscapeCanvasAdapter,
} from "./cytoscape-canvas";
import { loadFixture } from "../fixtures";
import { DEFAULT_BENCH_CONFIG } from "./types";
import type { RendererController } from "./types";

interface MockNode {
  id: string;
  incident: number;
}

function resetMock(): void {
  const reg = (globalThis as Record<string, unknown>).__cytoscapeMockRegistry as
    | Map<string, MockNode>
    | undefined;
  const sel = (globalThis as Record<string, unknown>).__cytoscapeMockSelections as
    | Set<string>
    | undefined;
  reg?.clear();
  sel?.clear();
}

function seedMock(
  nodes: ReadonlyArray<{ id: string }>,
  edges: ReadonlyArray<{ source: string; target: string }>,
): void {
  const reg = (globalThis as Record<string, unknown>).__cytoscapeMockRegistry as
    | Map<string, MockNode>
    | undefined;
  if (!reg) return;
  reg.clear();
  for (const node of nodes) {
    const incident = edges.filter(
      (e) => e.source === node.id || e.target === node.id,
    ).length;
    reg.set(node.id, { id: node.id, incident });
  }
}

describe("CytoscapeCanvasAdapter", () => {
  it("reports the canonical id", () => {
    const adapter = new CytoscapeCanvasAdapter();
    expect(adapter.id).toBe("cytoscape-canvas");
  });

  it("always reports itself as enabled", () => {
    const adapter = new CytoscapeCanvasAdapter();
    expect(adapter.isEnabled(DEFAULT_BENCH_CONFIG)).toBe(true);
  });

  it("pins the cytoscape version", () => {
    const adapter = new CytoscapeCanvasAdapter();
    expect(adapter.version).toMatch(/^\d+\.\d+/);
  });
});

describe("CytoscapeCanvasAdapter.mount", () => {
  let adapter: CytoscapeCanvasAdapter;

  beforeEach(() => {
    adapter = new CytoscapeCanvasAdapter();
    resetMock();
  });

  it("mounts the call-graph-small fixture with working selection", async () => {
    const fixture = loadFixture("call-graph-small");
    seedMock(fixture.nodes, fixture.edges);

    const controller: RendererController = await adapter.mount(fixture, {});

    const result = await controller.select(fixture.nodes[0]!.id);
    expect(result.selection_works).toBe(true);
    expect(result.edge_highlight_works).toBe(true);

    await controller.teardown();
  });

  it("runs the scenario sequence end-to-end", async () => {
    const fixture = loadFixture("call-graph-small");
    seedMock(fixture.nodes, fixture.edges);

    const controller = await adapter.mount(fixture, {});

    expect(await controller.relayout()).toBeGreaterThanOrEqual(0);
    expect(await controller.pan(10, 10)).toBeGreaterThanOrEqual(0);
    expect(await controller.zoom(1.1)).toBeGreaterThanOrEqual(0);
    expect(controller.isLayoutComplete()).toBe(true);

    await controller.teardown();
  });

  it("returns selection_works=false for an unknown node id", async () => {
    const fixture = loadFixture("call-graph-small");
    seedMock(fixture.nodes, fixture.edges);

    const controller = await adapter.mount(fixture, {});

    const result = await controller.select("does-not-exist");
    expect(result.selection_works).toBe(false);
    expect(result.edge_highlight_works).toBe(false);

    await controller.teardown();
  });
});