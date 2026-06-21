import { describe, it, expect } from "vitest";
import {
  loadFixture,
  loadAllFixtures,
  FIXTURE_IDS,
} from "./index";
import { generateFixture } from "./generator";
import { assertFixture } from "../fixture-schema";

describe("loadAllFixtures", () => {
  it("returns the canonical roster from REQ-3", () => {
    const ids = loadAllFixtures().map((f) => f.fixture_id);
    expect(ids).toEqual([
      "call-graph-small",
      "dependency-graph-small",
      "architecture-c4-medium",
      "landing-overview-medium",
      "call-graph-medium",
      "dependency-graph-medium",
      "call-graph-large",
    ]);
  });

  it("exports the same ids as FIXTURE_IDS", () => {
    expect(FIXTURE_IDS).toEqual(loadAllFixtures().map((f) => f.fixture_id));
  });

  it("every fixture satisfies the Fixture schema", () => {
    for (const fixture of loadAllFixtures()) {
      expect(() => assertFixture(fixture)).not.toThrow();
    }
  });
});

describe("loadFixture", () => {
  it("returns a known fixture by id", () => {
    const fixture = loadFixture("call-graph-small");
    expect(fixture.kind).toBe("call_graph");
    expect(fixture.size_band).toBe("small");
  });

  it("throws on an unknown id and lists the available ones", () => {
    expect(() => loadFixture("does-not-exist")).toThrow(/call-graph-small/);
  });

  it("returns the same reference twice (deterministic loader)", () => {
    expect(loadFixture("call-graph-medium")).toBe(
      loadFixture("call-graph-medium"),
    );
  });
});

describe("generateFixture", () => {
  it("produces deterministic output for the same args", () => {
    const a = generateFixture({
      fixture_id: "call-graph-medium",
      kind: "call_graph",
      size_band: "medium",
      node_count: 1000,
    });
    const b = generateFixture({
      fixture_id: "call-graph-medium",
      kind: "call_graph",
      size_band: "medium",
      node_count: 1000,
    });
    expect(a).toEqual(b);
  });

  it("respects the requested node_count exactly", () => {
    const fixture = generateFixture({
      fixture_id: "synthetic",
      kind: "call_graph",
      size_band: "small",
      node_count: 50,
    });
    expect(fixture.nodes.length).toBe(50);
    expect(fixture.node_count).toBe(50);
  });

  it("never emits self-referencing edges", () => {
    const fixture = generateFixture({
      fixture_id: "synthetic",
      kind: "call_graph",
      size_band: "medium",
      node_count: 200,
    });
    for (const edge of fixture.edges) {
      expect(edge.source).not.toBe(edge.target);
    }
  });

  it("keeps edge_count within the size band cap", () => {
    const small = generateFixture({
      fixture_id: "synthetic-small",
      kind: "call_graph",
      size_band: "small",
      node_count: 80,
    });
    expect(small.edge_count).toBeLessThanOrEqual(100);

    const large = generateFixture({
      fixture_id: "synthetic-large",
      kind: "call_graph",
      size_band: "large",
      node_count: 5000,
    });
    expect(large.edge_count).toBeLessThanOrEqual(20000);
    expect(large.edge_count).toBeGreaterThan(0);
  });

  it("round-trips through the Fixture schema", () => {
    const fixture = generateFixture({
      fixture_id: "synthetic-medium",
      kind: "dependency_graph",
      size_band: "medium",
      node_count: 1000,
    });
    expect(() => assertFixture(fixture)).not.toThrow();
  });
});