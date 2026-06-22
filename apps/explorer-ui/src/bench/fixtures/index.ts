/**
 * Fixture loader for the E7 benchmark harness.
 *
 * Hand-tuned fixtures are imported statically. Medium and large
 * fixtures are generated on demand by `generator.ts` and validated
 * against the `Fixture` schema before being returned to callers.
 *
 * The set returned by `loadAllFixtures` is the canonical roster
 * referenced by REQ-3 and SCN-1 in the spec.
 */

import { assertFixture } from "../fixture-schema";
import type { Fixture } from "../fixture-schema";

import callGraphSmall from "./call-graph-small.json";
import dependencyGraphSmall from "./dependency-graph-small.json";
import architectureC4MediumJson from "./architecture-c4-medium.json";
import landingOverviewMediumJson from "./landing-overview-medium.json";

import { generateFixture } from "./generator";

/**
 * Static fixtures bundled with the harness. Validated once at module
 * load -- if any of them is malformed, the harness fails fast on
 * startup.
 */
const STATIC_FIXTURES: readonly Fixture[] = [
  callGraphSmall as Fixture,
  dependencyGraphSmall as Fixture,
  architectureC4MediumJson as Fixture,
  landingOverviewMediumJson as Fixture,
];

for (const fixture of STATIC_FIXTURES) {
  assertFixture(fixture);
}

/**
 * Generated fixtures. The generator runs at load time so the schema
 * validation covers the programmatic fixtures too. The generator
 * output is deterministic; running the loader twice yields identical
 * fixtures.
 */
const GENERATED_FIXTURES: readonly Fixture[] = [
  generateFixture({
    fixture_id: "call-graph-medium",
    kind: "call_graph",
    size_band: "medium",
    node_count: 1000,
  }),
  generateFixture({
    fixture_id: "dependency-graph-medium",
    kind: "dependency_graph",
    size_band: "medium",
    node_count: 1000,
  }),
  generateFixture({
    fixture_id: "call-graph-large",
    kind: "call_graph",
    size_band: "large",
    node_count: 5000,
  }),
];

for (const fixture of GENERATED_FIXTURES) {
  assertFixture(fixture);
}

const ALL_FIXTURES: readonly Fixture[] = [
  ...STATIC_FIXTURES,
  ...GENERATED_FIXTURES,
];

const FIXTURE_INDEX: ReadonlyMap<string, Fixture> = new Map(
  ALL_FIXTURES.map((f) => [f.fixture_id, f]),
);

/**
 * Load a single fixture by id. Throws if the id is unknown.
 */
export function loadFixture(id: string): Fixture {
  const fixture = FIXTURE_INDEX.get(id);
  if (!fixture) {
    throw new Error(
      `loadFixture: unknown fixture id "${id}". Known: ${[...FIXTURE_INDEX.keys()].join(", ")}`,
    );
  }
  return fixture;
}

/**
 * Load the canonical fixture roster. The order is deterministic.
 */
export function loadAllFixtures(): readonly Fixture[] {
  return ALL_FIXTURES;
}

/** Public list of fixture ids for documentation and CLI help. */
export const FIXTURE_IDS: readonly string[] = ALL_FIXTURES.map(
  (f) => f.fixture_id,
);