import { describe, it, expect } from "vitest";
import {
  DEFAULT_BENCH_CONFIG,
  RendererMountError,
  makeRendererInfo,
  type BenchConfig,
  type RendererAdapter,
  type RendererController,
  type MountHooks,
} from "./types";
import type { Fixture } from "../fixture-schema";

const fixture: Fixture = {
  fixture_id: "call-graph-small",
  kind: "call_graph",
  size_band: "small",
  node_count: 2,
  edge_count: 1,
  nodes: [
    { id: "n1", label: "alpha", kind: "function", style_class: "node-function" },
    { id: "n2", label: "beta", kind: "function", style_class: "node-function" },
  ],
  edges: [
    {
      id: "e1",
      source: "n1",
      target: "n2",
      relation: "calls",
      style_class: "edge-calls",
    },
  ],
};

function makeFakeController(): RendererController {
  return {
    async relayout() {
      return 1;
    },
    async pan() {
      return 2;
    },
    async zoom() {
      return 3;
    },
    async select() {
      return { duration_ms: 4, selection_works: true, edge_highlight_works: true };
    },
    isLayoutComplete() {
      return true;
    },
    async teardown() {
      return;
    },
  };
}

function makeFakeAdapter(
  overrides: Partial<RendererAdapter> = {},
): RendererAdapter {
  return {
    id: "cytoscape-canvas",
    version: "test",
    isEnabled: () => true,
    mount: async () => makeFakeController(),
    ...overrides,
  };
}

describe("DEFAULT_BENCH_CONFIG", () => {
  it("disables Sigma by default", () => {
    expect(DEFAULT_BENCH_CONFIG.enable_sigma).toBe(false);
  });

  it("records at least one warm run by default", () => {
    expect(DEFAULT_BENCH_CONFIG.warm).toBe(true);
    expect(DEFAULT_BENCH_CONFIG.warm_runs).toBeGreaterThanOrEqual(1);
  });

  it("points output at the conventional artifacts path", () => {
    expect(DEFAULT_BENCH_CONFIG.output_dir).toBe(
      "artifacts/e7-renderer-bench",
    );
  });
});

describe("RendererAdapter contract", () => {
  it("accepts a fixture and yields a controller", async () => {
    const adapter = makeFakeAdapter();
    const hooks: MountHooks = {};
    const controller = await adapter.mount(fixture, hooks);
    expect(await controller.pan(1, 1)).toBeGreaterThanOrEqual(0);
    await controller.teardown();
  });

  it("lets adapters opt out of a config", () => {
    const sigmaAdapter = makeFakeAdapter({
      id: "sigma-poc",
      isEnabled: (cfg: BenchConfig) => cfg.enable_sigma,
    });
    expect(sigmaAdapter.isEnabled(DEFAULT_BENCH_CONFIG)).toBe(false);
    expect(
      sigmaAdapter.isEnabled({ ...DEFAULT_BENCH_CONFIG, enable_sigma: true }),
    ).toBe(true);
  });

  it("propagates mount failures through RendererMountError", async () => {
    const adapter = makeFakeAdapter({
      mount: async () => {
        throw new RendererMountError("cytoscape-canvas", "boom");
      },
    });
    await expect(adapter.mount(fixture, {})).rejects.toBeInstanceOf(
      RendererMountError,
    );
  });
});

describe("makeRendererInfo", () => {
  it("captures id, version, and renderer config", () => {
    const adapter = makeFakeAdapter({ version: "1.2.3" });
    const info = makeRendererInfo(adapter, { webgl: true });
    expect(info.id).toBe("cytoscape-canvas");
    expect(info.version).toBe("1.2.3");
    expect(info.config).toEqual({ webgl: true });
  });
});