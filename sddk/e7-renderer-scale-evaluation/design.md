# Kernel Design: E7 — Explorer Renderer Scale Evaluation

## Purpose

Define the **technical design** of the E7 benchmark harness. The spec defines
behavior; this document defines implementation. Constraints:

- The production `InteractiveGraph` must not be modified.
- The benchmark subtree must be isolated from production code.
- All renderer adapters consume the same `Fixture` and return a
  `MetricsRecord`.
- The Sigma adapter is gated by configuration.

## Module Layout

```
apps/explorer-ui/src/bench/
├── fixture-schema.ts        # Fixture schema (zod or hand-written)
├── metrics.ts               # MetricsRecord schema
├── runner.ts                # Scenario executor
├── report.ts                # JSON + Markdown writer
├── fixtures/                # Stable graph payloads
│   ├── call-graph-small.json
│   ├── call-graph-medium.json
│   ├── call-graph-large.json
│   ├── dependency-graph-small.json
│   ├── dependency-graph-medium.json
│   ├── architecture-c4-medium.json
│   └── landing-overview-medium.json
└── renderers/
    ├── types.ts             # RendererAdapter contract
    ├── cytoscape-canvas.ts  # Production renderer mirrored
    ├── cytoscape-webgl.ts   # WebGL mode adapter
    └── sigma-poc.ts         # Gated Sigma adapter
```

## Contracts

### RendererAdapter

```ts
export interface RendererAdapter {
  /** Stable identifier used in metrics records. */
  readonly id: "cytoscape-canvas" | "cytoscape-webgl" | "sigma-poc";

  /** Renderer version reported alongside metrics. */
  readonly version: string;

  /** Mount a fixture into the renderer and return a controller. */
  mount(fixture: Fixture): Promise<RendererController>;

  /** Return whether the adapter is enabled for a given config. */
  isEnabled(config: BenchConfig): boolean;
}

export interface RendererController {
  /** Tear down the renderer and any DOM it created. */
  teardown(): Promise<void>;
}
```

### Fixture

```ts
export interface Fixture {
  fixture_id: string;
  kind: "call_graph" | "dependency_graph" | "architecture_c4" | "landing_overview";
  size_band: "small" | "medium" | "large";
  node_count: number;
  edge_count: number;
  nodes: Array<{
    id: string;
    label: string;
    kind: string;
    style_class: string | null;
  }>;
  edges: Array<{
    id: string;
    source: string;
    target: string;
    relation: string;
    style_class: string | null;
  }>;
}
```

### MetricsRecord

See `spec.md` for the full shape. The schema is the source of truth for both
adapter output and report input.

### BenchConfig

```ts
export interface BenchConfig {
  /** Whether to run cold executions. Default true. */
  cold: boolean;
  /** Whether to run warm executions. Default true. */
  warm: boolean;
  /** Number of warm runs per (fixture, renderer) cell. Default 2. */
  warm_runs: number;
  /** Whether the Sigma adapter is allowed to run. Default false. */
  enable_sigma: boolean;
  /** Output directory. Default `artifacts/e7-renderer-bench`. */
  output_dir: string;
}
```

## Renderer Adapter Implementations

### cytoscape-canvas.ts

The canvas adapter mirrors the production data path:

- Reuses `toCytoscapeElements(fixture.nodes, fixture.edges)` from
  `apps/explorer-ui/src/components/InteractiveGraph/adapter.ts`.
- Reuses `buildStylesheet()` from
  `apps/explorer-ui/src/components/InteractiveGraph/stylesheet.ts`.
- Mounts a cytoscape instance with `renderer: { name: "canvas" }`.

This adapter is the **baseline**. Any future renderer must beat this baseline
to justify migration.

### cytoscape-webgl.ts

The WebGL adapter mirrors the same data path but enables the preview WebGL
mode:

```ts
cytoscape({
  container,
  elements,
  style: buildStylesheet(),
  layout: { name: "preset" },
  renderer: {
    name: "canvas", // cytoscape 3.31 reuses canvas engine; webgl mode is opt-in
    webgl: true,
  },
});
```

If Cytoscape's WebGL preview is not available in the pinned version, the
adapter fails fast and the runner records `behavior.layout_completed = false`.

### sigma-poc.ts

The Sigma adapter is intentionally minimal:

- It uses `graphology` to construct the graph from the fixture.
- It uses `sigma` to render it.
- It maps fixture `style_class` to a Sigma node color via a small lookup.
- It reports the same `MetricsRecord` shape.

This adapter exists so the team can produce measurements if the documented
gate is met. It is **not** a production path.

## Scenario Runner

### runner.ts

The runner loads fixtures, instantiates adapters, and emits metrics records.

```ts
export async function runBench(config: BenchConfig): Promise<MetricsRecord[]> {
  const fixtures = loadAllFixtures();
  const adapters = [
    new CytoscapeCanvasAdapter(),
    new CytoscapeWebglAdapter(),
    ...(config.enable_sigma ? [new SigmaPocAdapter()] : []),
  ];
  const records: MetricsRecord[] = [];
  for (const fixture of fixtures) {
    for (const adapter of adapters) {
      if (!adapter.isEnabled(config)) continue;
      if (config.cold) {
        records.push(await runOnce(fixture, adapter, "cold", 0));
      }
      for (let i = 0; i < config.warm_runs; i++) {
        records.push(await runOnce(fixture, adapter, "warm", i + 1));
      }
    }
  }
  return records;
}
```

### runOnce()

`runOnce` is the per-cell driver. It:

1. Resets the module registry if mode is `cold` (best-effort, only matters
   under Vitest with `vi.resetModules`).
2. Starts `performance.now()`.
3. Mounts the adapter and measures `first_render` and `fit` via the
   controller's lifecycle hooks.
4. Performs pan, zoom, select, and relayout operations and measures each.
5. Verifies `selection_works` and `edge_highlight_works` by inspecting the
   controller's state.
6. Returns the assembled `MetricsRecord`.

### Failure handling

If a renderer throws during mount or interaction, the run is recorded with
the exception message in `notes` and `behavior.regressions` populated. The run
is still saved; it is marked invalid downstream.

## Report Writer

### report.ts

The report writer accepts a list of `MetricsRecord` and emits two artifacts:

1. `artifacts/e7-renderer-bench/results.json` — the raw records
2. `artifacts/e7-renderer-bench/report.md` — a Markdown summary

The Markdown report includes:

- A header with the date and the config used
- One section per `renderer.id` with one row per fixture
- Per-run timings as a small table
- A regression section listing all runs where behavior checks failed

## Fixture Generation

The fixtures live under `apps/explorer-ui/src/bench/fixtures/`. Two source
strategies are acceptable:

1. **Hand-crafted** fixtures for `architecture-c4-medium` and
   `landing-overview-medium`. These shapes are not yet produced by the
   backend at the needed size.
2. **Captured** fixtures for `call-graph-*` and `dependency-graph-*`. These
   come from real `/api/graph/...` payloads, captured once and committed.

The fixtures must be **deterministic**. No timestamps, no random ids, no
floating-point coordinates.

## Configuration and Gating

The Sigma adapter is gated by `BenchConfig.enable_sigma`, which defaults to
`false`. The CLI entrypoint reads `BENCH_ENABLE_SIGMA=1` from the environment
and passes it into the config.

The bench script lives in `apps/explorer-ui/package.json`:

```jsonc
{
  "scripts": {
    "bench:renderer": "tsx src/bench/cli.ts"
  }
}
```

`tsx` is the existing TypeScript runner used elsewhere in the project.

## Testing Strategy

### Vitest (jsdom)

- A smoke test mounts each adapter with a small fixture and asserts node
  and edge counts.
- A schema test asserts every fixture validates against `Fixture`.
- A metrics test asserts the report writer accepts any list of
  `MetricsRecord` and emits both artifacts.

These tests run in jsdom. They validate structure, not performance.

### Real-browser bench script

The full benchmark runs outside Vitest. It requires a real browser because
Cytoscape WebGL and Sigma rely on canvas/WebGL APIs that jsdom does not
provide. The bench script is the source of truth for timings.

## Tradeoffs

- **Why not just use Playwright inside Vitest?** Playwright tests run in
  Vitest, but they require a separate browser install. Keeping the bench
  script separate keeps Vitest fast and isolated. The trade is that the
  bench script is not part of the unit-test loop.
- **Why a hand-written schema instead of zod?** zod would add a dependency
  and inflate the bundle. The schema is small enough that TypeScript types
  plus a runtime `assertFixture` function are sufficient.
- **Why reuse `toCytoscapeElements`?** This guarantees the benchmark exercises
  the production data path. Any drift between benchmark and production
  surfaces as a measurable regression.

## Risks

- jsdom does not implement canvas/WebGL. Adapters may need a real browser
  for smoke tests as well. Mitigation: keep jsdom tests structural and put
  any visual smoke in a Playwright spec.
- Cytoscape WebGL preview is on a moving target. The pinned version is
  recorded in `MetricsRecord.renderer.version`.
- Sigma adapter is a proof of concept and may diverge from production. The
  design accepts this: it is not a migration path, only a measurement.

## References

- `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.tsx`
- `apps/explorer-ui/src/components/InteractiveGraph/adapter.ts`
- `apps/explorer-ui/src/components/InteractiveGraph/stylesheet.ts`
- `apps/explorer-ui/src/components/InteractiveGraph/layout.worker.ts`
- `docs/adr/ADR-039-explorer-navigation-model.md`
- `docs/adr/ADR-041-explorer-renderer-scale-evaluation.md`
- `sddk/e7-renderer-scale-evaluation/proposal.md`
- `sddk/e7-renderer-scale-evaluation/spec.md`