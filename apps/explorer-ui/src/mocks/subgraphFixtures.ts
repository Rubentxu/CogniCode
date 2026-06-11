/**
 * Subgraph fixtures — mirror the Rust `SubgraphResponse` wire shape
 * produced by `GET /api/graph/:id/subgraph`.
 *
 * Used by:
 * - `apps/explorer-ui/src/api/subgraph_schemas.test.ts` (zod parse)
 * - `apps/explorer-ui/src/api/client.subgraph.test.ts` (MSW handler)
 * - `apps/explorer-ui/src/components/InteractiveGraph/adapter.test.ts`
 * - `apps/explorer-ui/src/components/InteractiveGraph/layout.worker.test.ts`
 * - `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.test.tsx`
 */
import type { GraphNode, GraphEdge, SubgraphResponse } from "../api/types";

// ============================================================================
// Small fixture — 4 nodes, 3 edges, fully hand-rolled
// ============================================================================

const smallNodes: GraphNode[] = [
  {
    id: "sym:foo::n0",
    label: "n0",
    kind: "function",
    file: "foo.rs",
    line: 1,
    style_class: "function",
  },
  {
    id: "sym:foo::n1",
    label: "n1",
    kind: "function",
    file: "foo.rs",
    line: 10,
    style_class: "function",
  },
  {
    id: "sym:foo::n2",
    label: "n2",
    kind: "module",
    file: "foo/mod.rs",
    line: 1,
    style_class: "module",
  },
  {
    id: "sym:ext::lib",
    label: "lib",
    kind: "external",
    file: "ext/lib.rs",
    line: 1,
    style_class: "external",
  },
];

const smallEdges: GraphEdge[] = [
  {
    source: "sym:foo::n0",
    target: "sym:foo::n1",
    relation: "calls",
    style_class: "edge.calls",
  },
  {
    source: "sym:foo::n1",
    target: "sym:foo::n2",
    relation: "implements",
    style_class: "edge.implements",
  },
  {
    source: "sym:foo::n1",
    target: "sym:ext::lib",
    relation: "uses",
    style_class: "edge.uses",
  },
];

export const smallSubgraphFixture: SubgraphResponse = {
  root: "sym:foo::n0",
  nodes: smallNodes,
  edges: smallEdges,
  truncated: false,
};

// ============================================================================
// Medium + large (used by Phase-3 mock handlers / smoke fixtures)
// ============================================================================

export const mediumSubgraphFixture: SubgraphResponse = buildSized(50, 75);
export const largeSubgraphFixture: SubgraphResponse = buildSized(200, 280);

function buildSized(nodeCount: number, edgeCount: number): SubgraphResponse {
  const nodes: GraphNode[] = Array.from({ length: nodeCount }, (_, i) => ({
    id: `sym:med::n${i}`,
    label: `n${i}`,
    kind: i % 3 === 0 ? "module" : "function",
    file: "med.rs",
    line: i + 1,
    style_class: i % 3 === 0 ? "module" : "function",
  }));
  const edges: GraphEdge[] = Array.from({ length: edgeCount }, (_, i) => ({
    source: `sym:med::n${i % nodeCount}`,
    target: `sym:med::n${(i + 1) % nodeCount}`,
    relation: "calls",
    style_class: "edge.calls",
  }));
  return {
    root: "sym:med::n0",
    nodes,
    edges,
    truncated: false,
  };
}

// ============================================================================
// Rationale fixture — 3 nodes, 2 edges with corroboration scores
// ============================================================================

const rationaleNodes: GraphNode[] = [
  {
    id: "sym:rat::focus",
    label: "focus",
    kind: "function",
    file: "rat.rs",
    line: 1,
    style_class: "function",
  },
  {
    id: "sym:rat::a",
    label: "supporter_a",
    kind: "function",
    file: "rat.rs",
    line: 5,
    style_class: "function",
  },
  {
    id: "sym:rat::b",
    label: "supporter_b",
    kind: "function",
    file: "rat.rs",
    line: 10,
    style_class: "function",
  },
];

const rationaleEdges: GraphEdge[] = [
  {
    source: "sym:rat::a",
    target: "sym:rat::focus",
    relation: "corroborated_by",
    style_class: "edge-corroborated",
  },
  {
    source: "sym:rat::b",
    target: "sym:rat::focus",
    relation: "corroborated_by",
    style_class: "edge-corroborated",
  },
];

export const rationaleSubgraphFixture: SubgraphResponse = {
  root: "sym:rat::focus",
  nodes: rationaleNodes,
  edges: rationaleEdges,
  truncated: false,
  corroboration_scores: {
    "sym:rat::a->sym:rat::focus": 0.85,
    "sym:rat::b->sym:rat::focus": 0.42,
  },
};
