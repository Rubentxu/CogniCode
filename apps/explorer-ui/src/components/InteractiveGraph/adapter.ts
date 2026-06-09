/**
 * Adapter — REST `SubgraphResponse` → cytoscape `ElementsDefinition`.
 *
 * Cytoscape's element shape is:
 *   { data: { id, label, ... } }
 * Nodes carry `id`; edges carry `source` and `target` referencing
 * node ids. Style classes pass through from the REST response
 * (already derived on the backend) so cytoscape's stylesheet can
 * bucket them.
 */
import type { ElementsDefinition } from "cytoscape";

import type { GraphNode, GraphEdge } from "../../api/types";

export function toCytoscapeElements(
  nodes: GraphNode[],
  edges: GraphEdge[],
): ElementsDefinition {
  const cyNodes: ElementsDefinition["nodes"] = nodes.map((n) => ({
    group: "nodes",
    data: {
      id: n.id,
      label: n.label,
      kind: n.kind,
      file: n.file,
      line: n.line,
      style_class: n.style_class,
    },
  }));
  const cyEdges: ElementsDefinition["edges"] = edges.map((e, i) => ({
    group: "edges",
    data: {
      id: `${e.source}->${e.target}:${i}`,
      source: e.source,
      target: e.target,
      relation: e.relation,
      style_class: e.style_class,
    },
  }));
  return { nodes: cyNodes, edges: cyEdges };
}
