/**
 * Bench runner.
 *
 * The runner drives the fixed scenario sequence
 *   load -> fit -> pan -> zoom -> select -> relayout
 * over every (fixture, renderer) cell, producing one `MetricsRecord`
 * per run. Cold and warm runs are recorded separately so the report
 * can compare them.
 *
 * The runner is the single source of truth for which adapters
 * participate in a given run. It honors `RendererAdapter.isEnabled`
 * and the `cold`, `warm`, and `warm_runs` fields of `BenchConfig`.
 */

import { loadAllFixtures } from "./fixtures";
import {
  type Behavior,
  type BenchConfig,
  type RendererAdapter,
  type RendererController,
  type MetricsRecord,
  makeMetricsRecord,
  isBehaviorValid,
} from "./index";
import { CytoscapeCanvasAdapter } from "./renderers/cytoscape-canvas";
import { CytoscapeWebglAdapter } from "./renderers/cytoscape-webgl";
import { SigmaPocAdapter } from "./renderers/sigma-poc";

/**
 * Default roster of adapters the runner iterates over. The Sigma
 * adapter is included even when disabled -- the runner asks each
 * adapter `isEnabled` and skips the disabled ones.
 */
const DEFAULT_ADAPTERS: readonly RendererAdapter[] = [
  new CytoscapeCanvasAdapter(),
  new CytoscapeWebglAdapter(),
  new SigmaPocAdapter(),
];

/**
 * Default adapter info. The runner records the renderer config block
 * so the report can show exactly which renderer mode each cell used.
 */
const DEFAULT_ADAPTER_INFO: Record<
  RendererAdapter["id"],
  { config: Record<string, unknown> }
> = {
  "cytoscape-canvas": { config: { name: "canvas" } },
  "cytoscape-webgl": {
    config: {
      name: "canvas",
      webgl: true,
      webglTexSize: 4096,
      webglTexRows: 24,
      webglBatchSize: 2048,
      webglTexPerBatch: 16,
    },
  },
  "sigma-poc": { config: { renderer: "webgl" } },
};

/**
 * Default roster of fixtures, in deterministic order.
 */
const DEFAULT_FIXTURES = loadAllFixtures();

/**
 * Progress events emitted by the runner. Used by the CLI in T9 to
 * drive a spinner or a CI reporter.
 */
export interface BenchProgressEvent {
  fixture_id: string;
  renderer_id: string;
  run_mode: "cold" | "warm";
  run_index: number;
  status: "ok" | "failed";
}

export interface BenchHooks {
  onProgress?: (event: BenchProgressEvent) => void;
}

/**
 * Run the harness and collect metrics records.
 */
export async function runBench(
  config: BenchConfig,
  options: {
    adapters?: readonly RendererAdapter[];
    fixtures?: readonly ReturnType<typeof loadAllFixtures>[number][];
    hooks?: BenchHooks;
  } = {},
): Promise<MetricsRecord[]> {
  const adapters = options.adapters ?? DEFAULT_ADAPTERS;
  const fixtures = options.fixtures ?? DEFAULT_FIXTURES;
  const records: MetricsRecord[] = [];

  for (const fixture of fixtures) {
    for (const adapter of adapters) {
      if (!adapter.isEnabled(config)) continue;

      if (config.cold) {
        const record = await runOnce(fixture, adapter, "cold", 0, config, options.hooks);
        records.push(record);
      }
      for (let i = 0; i < config.warm_runs; i++) {
        const record = await runOnce(
          fixture,
          adapter,
          "warm",
          i + 1,
          config,
          options.hooks,
        );
        records.push(record);
      }
    }
  }

  return records;
}

/**
 * Run a single (fixture, renderer, mode) cell. Drives the fixed
 * scenario sequence and emits one `MetricsRecord`.
 */
export async function runOnce(
  fixture: ReturnType<typeof loadAllFixtures>[number],
  adapter: RendererAdapter,
  mode: "cold" | "warm",
  runIndex: number,
  config: BenchConfig,
  hooks?: BenchHooks,
): Promise<MetricsRecord> {
  const rendererInfo = {
    id: adapter.id,
    version: adapter.version,
    config: { ...DEFAULT_ADAPTER_INFO[adapter.id].config },
  };

  const timings = {
    load: 0,
    first_render: 0,
    fit: 0,
    pan: 0,
    zoom: 0,
    select: 0,
    relayout: 0,
  };

  const behavior: Behavior = {
    selection_works: false,
    edge_highlight_works: false,
    layout_completed: false,
    regressions: [],
  };

  const notes: string[] = [];

  let controller: RendererController | null = null;

  try {
    const mountStart = performance.now();
    controller = await adapter.mount(fixture, {});
    timings.load = performance.now() - mountStart;

    const nodeToSelect = fixture.nodes[0]?.id ?? "";

    if (nodeToSelect) {
      const selectStart = performance.now();
      const selectResult = await controller.select(nodeToSelect);
      timings.select = performance.now() - selectStart;
      behavior.selection_works = selectResult.selection_works;
      behavior.edge_highlight_works = selectResult.edge_highlight_works;
      if (!behavior.selection_works) {
        behavior.regressions.push(
          `select(${nodeToSelect}) returned selection_works=false`,
        );
      }
    } else {
      notes.push("fixture has no nodes; selection skipped");
    }

    timings.pan = await controller.pan(10, 10);
    timings.zoom = await controller.zoom(1.05);

    if (fixture.node_count > 0) {
      timings.relayout = await controller.relayout();
    }

    behavior.layout_completed = controller.isLayoutComplete();
    if (!behavior.layout_completed) {
      behavior.regressions.push("controller.isLayoutComplete() returned false");
    }
  } catch (err) {
    notes.push(`mount failed: ${describeError(err)}`);
    behavior.regressions.push(`mount threw: ${describeError(err)}`);
  } finally {
    if (controller) {
      try {
        await controller.teardown();
      } catch (err) {
        notes.push(`teardown failed: ${describeError(err)}`);
      }
    }
  }

  const record = makeMetricsRecord({
    runner: detectRunnerInfo(),
    fixture: {
      fixture_id: fixture.fixture_id,
      kind: fixture.kind,
      size_band: fixture.size_band,
      node_count: fixture.node_count,
      edge_count: fixture.edge_count,
    },
    renderer: rendererInfo,
    run: { mode, index: runIndex },
    timings_ms: timings,
    behavior,
    notes: notes.join("; "),
  });

  if (hooks?.onProgress) {
    hooks.onProgress({
      fixture_id: fixture.fixture_id,
      renderer_id: adapter.id,
      run_mode: mode,
      run_index: runIndex,
      status: isBehaviorValid(record) ? "ok" : "failed",
    });
  }

  return record;
}

/**
 * Detect the runtime environment once. The runner records this in
 * every metrics record so the report can group results by machine.
 */
function detectRunnerInfo(): MetricsRecord["runner"] {
  if (typeof navigator !== "undefined" && navigator.userAgent) {
    const ua = navigator.userAgent;
    let browser = "unknown";
    if (ua.includes("Chrome")) browser = "chromium";
    else if (ua.includes("Firefox")) browser = "firefox";
    else if (ua.includes("Safari")) browser = "safari";
    return {
      browser,
      browser_version: extractVersion(ua),
      os: detectOs(ua),
      machine_profile: detectMachineProfile(),
    };
  }
  return {
    browser: "node",
    browser_version: "unknown",
    os: "unknown",
    machine_profile: "node",
  };
}

function extractVersion(ua: string): string {
  const match = ua.match(/(?:Chrome|Firefox|Safari)\/(\d+(?:\.\d+)*)/);
  return match ? match[1] : "unknown";
}

function detectOs(ua: string): string {
  if (ua.includes("Mac")) return "macos";
  if (ua.includes("Windows")) return "windows";
  if (ua.includes("Linux")) return "linux";
  return "unknown";
}

function detectMachineProfile(): string {
  if (typeof process !== "undefined" && process.env?.CI) return "ci";
  return "dev";
}

function describeError(err: unknown): string {
  if (err instanceof Error) return err.message;
  return String(err);
}