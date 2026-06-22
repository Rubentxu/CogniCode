/**
 * `MetricsRecord` schema and constructor.
 *
 * Every renderer adapter returns a `MetricsRecord` per run. The bench
 * runner aggregates these records; the report writer consumes them
 * directly. This module is the single source of truth for the record
 * shape.
 */

export type RunMode = "cold" | "warm";

export type RendererId =
  | "cytoscape-canvas"
  | "cytoscape-webgl"
  | "sigma-poc";

export interface RunnerInfo {
  browser: string;
  browser_version: string;
  os: string;
  machine_profile: string;
}

export interface FixtureInfo {
  fixture_id: string;
  kind: string;
  size_band: string;
  node_count: number;
  edge_count: number;
}

export interface RendererInfo {
  id: RendererId;
  version: string;
  config: Record<string, unknown>;
}

export interface RunInfo {
  mode: RunMode;
  index: number;
}

export interface TimingsMs {
  load: number;
  first_render: number;
  fit: number;
  pan: number;
  zoom: number;
  select: number;
  relayout: number;
}

export interface Behavior {
  selection_works: boolean;
  edge_highlight_works: boolean;
  layout_completed: boolean;
  regressions: string[];
}

export interface MetricsRecord {
  schema_version: "e7.0";
  runner: RunnerInfo;
  fixture: FixtureInfo;
  renderer: RendererInfo;
  run: RunInfo;
  timings_ms: TimingsMs;
  behavior: Behavior;
  notes: string;
}

export const METRICS_SCHEMA_VERSION = "e7.0" as const;

const EMPTY_TIMINGS: TimingsMs = {
  load: 0,
  first_render: 0,
  fit: 0,
  pan: 0,
  zoom: 0,
  select: 0,
  relayout: 0,
};

const FAILING_BEHAVIOR: Behavior = {
  selection_works: false,
  edge_highlight_works: false,
  layout_completed: false,
  regressions: [],
};

/**
 * Build a `MetricsRecord` with sensible defaults. Adapters fill in
 * only the fields they observe.
 */
export function makeMetricsRecord(input: {
  runner: RunnerInfo;
  fixture: FixtureInfo;
  renderer: RendererInfo;
  run: RunInfo;
  timings_ms?: Partial<TimingsMs>;
  behavior?: Partial<Behavior>;
  notes?: string;
}): MetricsRecord {
  return {
    schema_version: METRICS_SCHEMA_VERSION,
    runner: input.runner,
    fixture: input.fixture,
    renderer: input.renderer,
    run: input.run,
    timings_ms: { ...EMPTY_TIMINGS, ...input.timings_ms },
    behavior: { ...FAILING_BEHAVIOR, ...input.behavior },
    notes: input.notes ?? "",
  };
}

/**
 * Returns `true` if a record satisfies the minimum behavior gates
 * defined by REQ-10. A failing run is still persisted; it is the
 * report writer's job to flag it.
 */
export function isBehaviorValid(record: MetricsRecord): boolean {
  return (
    record.behavior.selection_works &&
    record.behavior.edge_highlight_works &&
    record.behavior.layout_completed
  );
}