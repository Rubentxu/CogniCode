/**
 * `bench` — barrel for the E7 benchmark harness.
 *
 * The barrel exposes the public surface of `apps/explorer-ui/src/bench/`.
 * Renderer adapters, the runner, and the report writer are added in
 * later tasks (T4–T8). This task ships only the schemas.
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