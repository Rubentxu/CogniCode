/**
 * Renderer adapter contract for the E7 benchmark harness.
 *
 * Every renderer adapter implements `RendererAdapter`. The bench
 * runner instantiates adapters, calls `mount(fixture)` for each cell,
 * and uses the returned `RendererController` to drive the scenario
 * sequence and observe behavior.
 *
 * Adapters live under `apps/explorer-ui/src/bench/renderers/`. The
 * production `InteractiveGraph` is NOT a renderer adapter -- the
 * adapters mirror its data path without mutating it.
 */

import type { Fixture } from "../fixture-schema";
import type {
  MetricsRecord,
  RendererId,
  TimingsMs,
  Behavior,
} from "../metrics";

/**
 * Configuration consumed by every renderer adapter. The CLI in T9
 * builds this from environment variables and CLI flags.
 */
export interface BenchConfig {
  /**
   * Whether to record a cold run per (fixture, renderer) cell.
   * Default: `true`.
   */
  cold: boolean;
  /**
   * Whether to record warm runs. Default: `true`.
   */
  warm: boolean;
  /**
   * Number of warm runs per cell. Default: `2`.
   */
  warm_runs: number;
  /**
   * Whether the Sigma proof-of-concept adapter is allowed to run.
   * Default: `false`. The CLI reads `BENCH_ENABLE_SIGMA=1`.
   */
  enable_sigma: boolean;
  /**
   * Output directory for the report writer. Default:
   * `artifacts/e7-renderer-bench`.
   */
  output_dir: string;
}

/**
 * Default benchmark configuration. Adapters MUST treat `isEnabled`
 * as a pure function of this struct.
 */
export const DEFAULT_BENCH_CONFIG: BenchConfig = {
  cold: true,
  warm: true,
  warm_runs: 2,
  enable_sigma: false,
  output_dir: "artifacts/e7-renderer-bench",
};

/**
 * Lifecycle hooks the adapter reports during `mount`. The runner
 * converts these into a `MetricsRecord.timings_ms` value.
 */
export interface MountHooks {
  onLoadStart?: () => void;
  onLoadEnd?: () => void;
  onFirstRender?: () => void;
  onFit?: () => void;
}

/**
 * Result of mounting a fixture into a renderer. The runner drives the
 * scenario sequence through these hooks.
 */
export interface RendererController {
  /**
   * Trigger a re-layout using the adapter's default algorithm.
   * Returns the time spent waiting for the layout to complete.
   * Adapters that do not support layout return `0`.
   */
  relayout(): Promise<number>;

  /**
   * Pan the camera by `(dx, dy)` in renderer coordinates. Returns
   * the time spent waiting for the operation to settle.
   */
  pan(dx: number, dy: number): Promise<number>;

  /**
   * Zoom the camera by a multiplicative factor. Returns the time
   * spent waiting for the operation to settle.
   */
  zoom(factor: number): Promise<number>;

  /**
   * Select a node by id and verify that its incident edges are
   * highlighted. Returns the time spent waiting for the operation
   * plus the result of the verification.
   */
  select(nodeId: string): Promise<{ duration_ms: number; selection_works: boolean; edge_highlight_works: boolean }>;

  /**
   * Whether the adapter believes its last layout completed. This is
   * the source for `MetricsRecord.behavior.layout_completed`.
   */
  isLayoutComplete(): boolean;

  /**
   * Tear down the renderer and any DOM it created. Must be idempotent.
   */
  teardown(): Promise<void>;
}

/**
 * A renderer adapter mounts fixtures and yields a controller.
 *
 * Adapters are constructed once and reused across many fixtures.
 */
export interface RendererAdapter {
  /** Stable identifier reported in metrics records. */
  readonly id: RendererId;

  /** Renderer version (npm version or upstream tag). */
  readonly version: string;

  /**
   * Whether the adapter is enabled for a given benchmark config.
   * Used by the runner to skip adapters that the user has not
   * explicitly opted into (currently the Sigma adapter).
   */
  isEnabled(config: BenchConfig): boolean;

  /**
   * Mount a fixture into the renderer. The returned controller
   * captures the timings and behavior the runner needs.
   *
   * Adapters MUST throw a `RendererMountError` if mounting fails;
   * the runner records the failure in `MetricsRecord.behavior`.
   */
  mount(
    fixture: Fixture,
    hooks: MountHooks,
  ): Promise<RendererController>;
}

/**
 * Errors raised by adapters during mount. The runner catches these
 * and turns them into metrics records with the message preserved in
 * `behavior.regressions`.
 */
export class RendererMountError extends Error {
  constructor(
    public readonly rendererId: RendererId,
    message: string,
    public readonly cause?: unknown,
  ) {
    super(message);
    this.name = "RendererMountError";
  }
}

/**
 * Helpers for building a `MetricsRecord` from inside an adapter.
 * Re-exported here so adapters depend only on `renderers/types.ts`
 * plus the public `metrics.ts` types.
 */
export interface AdapterReportInput {
  fixture: Fixture;
  timings_ms: TimingsMs;
  behavior: Behavior;
  notes?: string;
  runner: MetricsRecord["runner"];
  run: MetricsRecord["run"];
}

/**
 * Pure helper that builds the renderer info block. Adapters compute
 * this once and reuse it for every metrics record they emit.
 */
export function makeRendererInfo(
  adapter: Pick<RendererAdapter, "id" | "version">,
  config: Record<string, unknown>,
): MetricsRecord["renderer"] {
  return { id: adapter.id, version: adapter.version, config };
}