/**
 * Deterministic fixture generator for the E7 benchmark harness.
 *
 * Hand-tuned JSON cannot represent graphs at the thousands-of-nodes
 * scale without bloating the repository. This generator produces
 * fixtures that:
 *
 *   - are deterministic (same args produce the same output),
 *   - validate against the `Fixture` schema,
 *   - carry realistic-looking style classes for the corresponding
 *     ViewKind bucket.
 *
 * The generator is used by `loadFixture` for the medium and large
 * fixtures. Small fixtures stay hand-tuned under `fixtures/`.
 */

import type { Fixture, FixtureKind, SizeBand } from "../fixture-schema";

interface GeneratorArgs {
  fixture_id: string;
  kind: FixtureKind;
  size_band: SizeBand;
  /** Target node count. The generator picks a sensible edge count from it. */
  node_count: number;
}

const NODE_KIND_BY_FIXTURE: Record<FixtureKind, string> = {
  call_graph: "function",
  dependency_graph: "module",
  architecture_c4: "container",
  landing_overview: "hot_path",
};

const EDGE_RELATION_BY_FIXTURE: Record<FixtureKind, string> = {
  call_graph: "calls",
  dependency_graph: "depends_on",
  architecture_c4: "in_system",
  landing_overview: "calls",
};

const NODE_STYLE_BY_FIXTURE: Record<FixtureKind, string> = {
  call_graph: "node-function",
  dependency_graph: "node-module",
  architecture_c4: "node-container",
  landing_overview: "node-function",
};

const EDGE_STYLE_BY_FIXTURE: Record<FixtureKind, string> = {
  call_graph: "edge-calls",
  dependency_graph: "edge-dependency",
  architecture_c4: "edge-in-system",
  landing_overview: "edge-calls",
};

/**
 * Pick a deterministic edge count from a node count. Averages 3 edges
 * per node, bounded so the result is reasonable for the chosen size
 * band.
 */
function pickEdgeCount(nodeCount: number, band: SizeBand): number {
  const baseRatio = 3;
  const cap = band === "small" ? 100 : band === "medium" ? 3000 : 20000;
  return Math.min(cap, nodeCount * baseRatio);
}

/**
 * Generate a fixture with `node_count` nodes and a deterministic edge
 * set. Edges form a layered structure: node `i` connects to nodes in
 * a forward window of size `fan_out`. This is intentionally simple
 * but predictable -- the benchmark cares about counts, not topology
 * quality.
 */
export function generateFixture(args: GeneratorArgs): Fixture {
  const { fixture_id, kind, size_band, node_count } = args;

  const nodeKind = NODE_KIND_BY_FIXTURE[kind];
  const edgeRelation = EDGE_RELATION_BY_FIXTURE[kind];
  const nodeStyle = NODE_STYLE_BY_FIXTURE[kind];
  const edgeStyle = EDGE_STYLE_BY_FIXTURE[kind];

  const nodes = Array.from({ length: node_count }, (_, i) => ({
    id: `n${i}`,
    label: `${kind.replace(/_/g, "-")}-n${i}`,
    kind: nodeKind,
    style_class: nodeStyle,
  }));

  const fanOut = 3;
  const edges: Fixture["edges"] = [];
  let edgeId = 0;
  const edgeCountTarget = pickEdgeCount(node_count, size_band);

  outer: for (let i = 0; i < node_count; i++) {
    for (let k = 1; k <= fanOut; k++) {
      const target = i + k;
      if (target >= node_count) continue;
      edges.push({
        id: `e${edgeId++}`,
        source: `n${i}`,
        target: `n${target}`,
        relation: edgeRelation,
        style_class: edgeStyle,
      });
      if (edges.length >= edgeCountTarget) break outer;
    }
  }

  return {
    fixture_id,
    kind,
    size_band,
    node_count: nodes.length,
    edge_count: edges.length,
    nodes,
    edges,
  };
}