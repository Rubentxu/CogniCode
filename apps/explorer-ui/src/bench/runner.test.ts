import { describe, it, expect } from "vitest";

import { runBench, runOnce } from "./runner";
import {
  DEFAULT_BENCH_CONFIG,
  type BenchConfig,
  type RendererAdapter,
  type RendererController,
  RendererMountError,
  type Fixture,
} from "./index";
import { loadFixture } from "./fixtures";

/**
 * Spy adapter that captures how the runner drives it. The runner
 * does not need real cytoscape / sigma under vitest; this fake
 * preserves the contract and records the call sequence.
 */
function makeSpyAdapter(
  id: "cytoscape-canvas" | "cytoscape-webgl" | "sigma-poc",
  opts: {
    enabled?: boolean;
    failMount?: boolean;
    failSelect?: boolean;
    layoutComplete?: boolean;
  } = {},
): RendererAdapter & { mounts: Fixture[] } {
  const mounts: Fixture[] = [];
  const adapter: RendererAdapter & { mounts: Fixture[] } = {
    id,
    version: "spy-1.0",
    mounts,
    isEnabled: (cfg: BenchConfig) => {
      if (id === "sigma-poc") return cfg.enable_sigma === true;
      return opts.enabled ?? true;
    },
    async mount(fixture: Fixture): Promise<RendererController> {
      mounts.push(fixture);
      if (opts.failMount) {
        throw new RendererMountError(id, "spy mount failure");
      }
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
        async select(nodeId: string) {
          if (opts.failSelect) {
            return { duration_ms: 0, selection_works: false, edge_highlight_works: false };
          }
          const exists = fixture.nodes.some((n) => n.id === nodeId);
          return {
            duration_ms: 4,
            selection_works: exists,
            edge_highlight_works: exists && fixture.edges.length > 0,
          };
        },
        isLayoutComplete() {
          return opts.layoutComplete ?? true;
        },
        async teardown() {
          return;
        },
      };
    },
  };
  return adapter;
}

describe("runBench gating", () => {
  it("skips disabled adapters entirely", async () => {
    const spy = makeSpyAdapter("sigma-poc");
    const config: BenchConfig = {
      ...DEFAULT_BENCH_CONFIG,
      cold: true,
      warm: false,
      warm_runs: 0,
      enable_sigma: false,
    };
    await runBench(config, { adapters: [spy] });
    expect(spy.mounts.length).toBe(0);
  });

  it("includes the Sigma adapter when opted in", async () => {
    const spy = makeSpyAdapter("sigma-poc");
    const config: BenchConfig = {
      ...DEFAULT_BENCH_CONFIG,
      cold: true,
      warm: false,
      warm_runs: 0,
      enable_sigma: true,
    };
    const records = await runBench(config, {
      adapters: [spy],
      fixtures: [loadFixture("call-graph-small")],
    });
    expect(spy.mounts.length).toBe(1);
    expect(records.length).toBe(1);
    expect(records[0].renderer.id).toBe("sigma-poc");
  });

  it("always includes cytoscape adapters when configured", async () => {
    const canvas = makeSpyAdapter("cytoscape-canvas");
    const webgl = makeSpyAdapter("cytoscape-webgl");
    const config: BenchConfig = {
      ...DEFAULT_BENCH_CONFIG,
      cold: true,
      warm: false,
      warm_runs: 0,
      enable_sigma: false,
    };
    const records = await runBench(config, {
      adapters: [canvas, webgl],
      fixtures: [loadFixture("call-graph-small")],
    });
    expect(canvas.mounts.length).toBe(1);
    expect(webgl.mounts.length).toBe(1);
    expect(records.length).toBe(2);
  });
});

describe("runBench cold/warm runs", () => {
  it("records one cold run per cell when cold is enabled", async () => {
    const canvas = makeSpyAdapter("cytoscape-canvas");
    const config: BenchConfig = {
      ...DEFAULT_BENCH_CONFIG,
      cold: true,
      warm: false,
      warm_runs: 0,
    };
    const records = await runBench(config, {
      adapters: [canvas],
      fixtures: [loadFixture("call-graph-small")],
    });
    expect(records.length).toBe(1);
    expect(records[0].run.mode).toBe("cold");
    expect(records[0].run.index).toBe(0);
  });

  it("records N warm runs per cell when warm is enabled", async () => {
    const canvas = makeSpyAdapter("cytoscape-canvas");
    const config: BenchConfig = {
      ...DEFAULT_BENCH_CONFIG,
      cold: false,
      warm: true,
      warm_runs: 3,
    };
    const records = await runBench(config, {
      adapters: [canvas],
      fixtures: [loadFixture("call-graph-small")],
    });
    expect(records.length).toBe(3);
    for (let i = 0; i < 3; i++) {
      expect(records[i].run.mode).toBe("warm");
      expect(records[i].run.index).toBe(i + 1);
    }
  });

  it("records cold plus warm when both are enabled", async () => {
    const canvas = makeSpyAdapter("cytoscape-canvas");
    const config: BenchConfig = {
      ...DEFAULT_BENCH_CONFIG,
      cold: true,
      warm: true,
      warm_runs: 2,
    };
    const records = await runBench(config, {
      adapters: [canvas],
      fixtures: [loadFixture("call-graph-small")],
    });
    expect(records.length).toBe(3);
    expect(records[0].run.mode).toBe("cold");
    expect(records[1].run.mode).toBe("warm");
    expect(records[2].run.mode).toBe("warm");
  });
});

describe("runBench behavior checks", () => {
  it("flags runs with failed selection as invalid", async () => {
    const canvas = makeSpyAdapter("cytoscape-canvas", { failSelect: true });
    const config: BenchConfig = {
      ...DEFAULT_BENCH_CONFIG,
      cold: true,
      warm: false,
      warm_runs: 0,
    };
    const records = await runBench(config, {
      adapters: [canvas],
      fixtures: [loadFixture("call-graph-small")],
    });
    expect(records[0].behavior.selection_works).toBe(false);
    expect(records[0].behavior.regressions.length).toBeGreaterThan(0);
  });

  it("flags runs with failing layout as invalid", async () => {
    const canvas = makeSpyAdapter("cytoscape-canvas", { layoutComplete: false });
    const config: BenchConfig = {
      ...DEFAULT_BENCH_CONFIG,
      cold: true,
      warm: false,
      warm_runs: 0,
    };
    const records = await runBench(config, {
      adapters: [canvas],
      fixtures: [loadFixture("call-graph-small")],
    });
    expect(records[0].behavior.layout_completed).toBe(false);
    expect(records[0].behavior.regressions.length).toBeGreaterThan(0);
  });

  it("captures mount failures as regressions and notes", async () => {
    const canvas = makeSpyAdapter("cytoscape-canvas", { failMount: true });
    const config: BenchConfig = {
      ...DEFAULT_BENCH_CONFIG,
      cold: true,
      warm: false,
      warm_runs: 0,
    };
    const records = await runBench(config, {
      adapters: [canvas],
      fixtures: [loadFixture("call-graph-small")],
    });
    expect(records[0].behavior.regressions.length).toBeGreaterThan(0);
    expect(records[0].notes).toMatch(/mount failed/);
  });
});

describe("runBench timing capture", () => {
  it("records non-negative timings for every step", async () => {
    const canvas = makeSpyAdapter("cytoscape-canvas");
    const config: BenchConfig = {
      ...DEFAULT_BENCH_CONFIG,
      cold: true,
      warm: false,
      warm_runs: 0,
    };
    const records = await runBench(config, {
      adapters: [canvas],
      fixtures: [loadFixture("call-graph-small")],
    });
    const t = records[0].timings_ms;
    expect(t.load).toBeGreaterThanOrEqual(0);
    expect(t.pan).toBeGreaterThanOrEqual(0);
    expect(t.zoom).toBeGreaterThanOrEqual(0);
    expect(t.select).toBeGreaterThanOrEqual(0);
    expect(t.relayout).toBeGreaterThanOrEqual(0);
  });

  it("skips relayout when the fixture has zero nodes", async () => {
    const empty: Fixture = {
      fixture_id: "empty-test",
      kind: "call_graph",
      size_band: "small",
      node_count: 0,
      edge_count: 0,
      nodes: [],
      edges: [],
    };
    const canvas = makeSpyAdapter("cytoscape-canvas");
    const config: BenchConfig = {
      ...DEFAULT_BENCH_CONFIG,
      cold: true,
      warm: false,
      warm_runs: 0,
    };
    const records = await runBench(config, {
      adapters: [canvas],
      fixtures: [empty],
    });
    expect(records[0].timings_ms.relayout).toBe(0);
  });
});

describe("runBench progress hooks", () => {
  it("emits a progress event for every run", async () => {
    const canvas = makeSpyAdapter("cytoscape-canvas");
    const config: BenchConfig = {
      ...DEFAULT_BENCH_CONFIG,
      cold: true,
      warm: true,
      warm_runs: 1,
    };
    const events: string[] = [];
    await runBench(config, {
      adapters: [canvas],
      fixtures: [loadFixture("call-graph-small")],
      hooks: {
        onProgress: (event) => {
          events.push(`${event.renderer_id}:${event.run_mode}`);
        },
      },
    });
    expect(events).toContain("cytoscape-canvas:cold");
    expect(events).toContain("cytoscape-canvas:warm");
  });
});

describe("runOnce direct usage", () => {
  it("returns a MetricsRecord for a single run", async () => {
    const fixture = loadFixture("call-graph-small");
    const canvas = makeSpyAdapter("cytoscape-canvas");
    const record = await runOnce(
      fixture,
      canvas,
      "cold",
      0,
      { ...DEFAULT_BENCH_CONFIG, cold: true, warm: false, warm_runs: 0 },
    );
    expect(record.fixture.fixture_id).toBe("call-graph-small");
    expect(record.renderer.id).toBe("cytoscape-canvas");
    expect(record.run.mode).toBe("cold");
  });

  it("reports renderer config block in the metrics record", async () => {
    const fixture = loadFixture("call-graph-small");
    const webgl = makeSpyAdapter("cytoscape-webgl");
    const record = await runOnce(
      fixture,
      webgl,
      "cold",
      0,
      { ...DEFAULT_BENCH_CONFIG, cold: true, warm: false, warm_runs: 0 },
    );
    const cfg = record.renderer.config as Record<string, unknown>;
    expect(cfg.name).toBe("canvas");
    expect(cfg.webgl).toBe(true);
  });
});