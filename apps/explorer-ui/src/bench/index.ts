/**
 * `bench` — barrel for the E7 benchmark harness.
 *
 * The barrel exposes the public surface of `apps/explorer-ui/src/bench/`.
 * Renderer adapters, the runner, and the report writer are added in
 * later tasks (T4–T8). This task ships the schemas plus the adapter
 * contract.
 */

export {
  type Fixture,
  type FixtureKind,
  type FixtureNode,
  type FixtureEdge,
  type SizeBand,
  FixtureValidationError,
  assertFixture,
} from "./fixture-schema";

export {
  type MetricsRecord,
  type RunMode,
  type RendererId,
  type RunnerInfo,
  type FixtureInfo,
  type RendererInfo,
  type RunInfo,
  type TimingsMs,
  type Behavior,
  METRICS_SCHEMA_VERSION,
  makeMetricsRecord,
  isBehaviorValid,
} from "./metrics";

export {
  type BenchConfig,
  type MountHooks,
  type RendererController,
  type RendererAdapter,
  type AdapterReportInput,
  RendererMountError,
  DEFAULT_BENCH_CONFIG,
  makeRendererInfo,
} from "./renderers/types";

export {
  loadFixture,
  loadAllFixtures,
  FIXTURE_IDS,
} from "./fixtures/index";

export { generateFixture } from "./fixtures/generator";

export {
  CytoscapeCanvasAdapter,
} from "./renderers/cytoscape-canvas";

export { CytoscapeWebglAdapter } from "./renderers/cytoscape-webgl";

export {
  SigmaPocAdapter,
  seedSigmaMock,
  resetSigmaMock,
} from "./renderers/sigma-poc";

export { CYTOSCAPE_VERSION } from "./renderers/cytoscape-shared";