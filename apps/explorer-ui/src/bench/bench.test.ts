import { describe, it, expect } from "vitest";
import {
  assertFixture,
  FixtureValidationError,
  type Fixture,
} from "./fixture-schema";
import {
  METRICS_SCHEMA_VERSION,
  makeMetricsRecord,
  isBehaviorValid,
} from "./metrics";

const smallFixture: Fixture = {
  fixture_id: "call-graph-small",
  kind: "call_graph",
  size_band: "small",
  node_count: 3,
  edge_count: 2,
  nodes: [
    { id: "n1", label: "alpha", kind: "function", style_class: "node-function" },
    { id: "n2", label: "beta", kind: "function", style_class: "node-function" },
    { id: "n3", label: "gamma", kind: "function", style_class: "node-function" },
  ],
  edges: [
    {
      id: "e1",
      source: "n1",
      target: "n2",
      relation: "calls",
      style_class: "edge-calls",
    },
    {
      id: "e2",
      source: "n2",
      target: "n3",
      relation: "calls",
      style_class: "edge-calls",
    },
  ],
};

describe("assertFixture", () => {
  it("accepts a well-formed fixture", () => {
    expect(() => assertFixture(smallFixture)).not.toThrow();
  });

  it("rejects a missing fixture_id", () => {
    const broken = { ...smallFixture, fixture_id: "" };
    expect(() => assertFixture(broken)).toThrow(FixtureValidationError);
  });

  it("rejects an unknown kind", () => {
    const broken = { ...smallFixture, kind: "bogus" };
    expect(() => assertFixture(broken)).toThrow(/fixture\.kind/);
  });

  it("rejects a size_band mismatch", () => {
    const broken = { ...smallFixture, size_band: "huge" };
    expect(() => assertFixture(broken)).toThrow(/fixture\.size_band/);
  });

  it("rejects when node_count does not match nodes.length", () => {
    const broken = { ...smallFixture, node_count: 99 };
    expect(() => assertFixture(broken)).toThrow(/node_count/);
  });

  it("rejects when an edge references a non-existent node", () => {
    const broken: Fixture = {
      ...smallFixture,
      edge_count: 1,
      edges: [
        {
          id: "e1",
          source: "n1",
          target: "missing",
          relation: "calls",
          style_class: null,
        },
      ],
    };
    expect(() => assertFixture(broken)).toThrow(/must reference/);
  });

  it("rejects self-referencing edges", () => {
    const broken: Fixture = {
      ...smallFixture,
      edge_count: 1,
      edges: [
        {
          id: "e1",
          source: "n1",
          target: "n1",
          relation: "calls",
          style_class: null,
        },
      ],
    };
    expect(() => assertFixture(broken)).toThrow(/source and target/);
  });
});

describe("makeMetricsRecord", () => {
  const base = {
    runner: {
      browser: "chromium",
      browser_version: "120",
      os: "linux",
      machine_profile: "ci",
    },
    fixture: {
      fixture_id: "call-graph-small",
      kind: "call_graph",
      size_band: "small",
      node_count: 3,
      edge_count: 2,
    },
    renderer: {
      id: "cytoscape-canvas" as const,
      version: "3.34",
      config: {},
    },
    run: { mode: "cold" as const, index: 0 },
  };

  it("fills defaults for timings and behavior", () => {
    const record = makeMetricsRecord(base);
    expect(record.schema_version).toBe(METRICS_SCHEMA_VERSION);
    expect(record.timings_ms.load).toBe(0);
    expect(record.behavior.selection_works).toBe(false);
    expect(record.notes).toBe("");
  });

  it("merges partial timings and behavior overrides", () => {
    const record = makeMetricsRecord({
      ...base,
      timings_ms: { first_render: 42 },
      behavior: { selection_works: true, edge_highlight_works: true },
    });
    expect(record.timings_ms.first_render).toBe(42);
    expect(record.timings_ms.fit).toBe(0);
    expect(record.behavior.selection_works).toBe(true);
    expect(record.behavior.layout_completed).toBe(false);
  });

  it("marks records with all-green behavior as valid", () => {
    const record = makeMetricsRecord({
      ...base,
      behavior: {
        selection_works: true,
        edge_highlight_works: true,
        layout_completed: true,
      },
    });
    expect(isBehaviorValid(record)).toBe(true);
  });

  it("flags records with any failing behavior check", () => {
    const record = makeMetricsRecord({
      ...base,
      behavior: {
        selection_works: true,
        edge_highlight_works: false,
        layout_completed: true,
      },
    });
    expect(isBehaviorValid(record)).toBe(false);
  });
});