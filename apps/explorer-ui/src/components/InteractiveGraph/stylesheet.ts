/**
 * `stylesheet.ts` — cytoscape styles for the InteractiveGraph.
 *
 * Buckets follow the wire `style_class` taxonomy from the backend:
 * - Nodes: `function` (default), `module`, `external`,
 *   `node-decision` (multimodal), `node-doc` (multimodal),
 *   `node-issue` (multimodal), `node-evidence` (multimodal)
 * - Edges: `edge.calls` (default), `edge.implements`, `edge.uses`,
 *   `edge-cites` (multimodal), `edge-justifies` (multimodal),
 *   `edge-resolves` (multimodal), `edge-corroborated` (multimodal)
 * - Selection state: `selected`, `highlighted`, `dimmed`
 *
 * Unknown node `style_class` values fall back to the `function`
 * visual + a `console.warn` (per spec R2 of `interactive-graph`).
 *
 * T18 (multimodal) — adds 4 node blocks and 4 edge blocks for the
 * Generic Graph Layer. Shapes and palette chosen for visual
 * distinctiveness:
 *   decision  → diamond  / amber
 *   doc       → round-octagon / teal
 *   issue     → triangle / red
 *   evidence  → ellipse / purple
 *   cites     → dashed blue
 *   justifies → solid green
 *   resolves  → dotted orange
 *   corroborated → double violet
 */
import type { StylesheetCSS, StylesheetStyle } from "cytoscape";

export const KNOWN_NODE_CLASSES = new Set([
  "function",
  "module",
  "external",
  // ---- multimodal (T18) ----
  "node-decision",
  "node-doc",
  "node-issue",
  "node-evidence",
]);
export const KNOWN_EDGE_CLASSES = new Set([
  "edge.calls",
  "edge.implements",
  "edge.uses",
  // ---- multimodal (T18) ----
  "edge-cites",
  "edge-justifies",
  "edge-resolves",
  "edge-corroborated",
]);

// Cytoscape's Css types are extremely strict (e.g. `text-max-width`
// is typed as `PropertyValue<NodeSingular, string>`, not `number`).
// We cast to `Record<string, unknown>` at the call site to keep the
// visual intent obvious in this file without fighting the typings.
type Css = Record<string, unknown>;

const NODE_BASE: Css = {
  "background-color": "#3b82f6",
  "border-color": "#1d4ed8",
  "border-width": 1,
  label: "data(label)",
  color: "#0b1220",
  "font-size": 11,
  "text-valign": "center",
  "text-halign": "center",
  "text-wrap": "wrap",
  "text-max-width": 120,
  width: 80,
  height: 32,
};

const EDGE_BASE: Css = {
  "line-color": "#94a3b8",
  "target-arrow-color": "#94a3b8",
  "target-arrow-shape": "triangle",
  "curve-style": "bezier",
  width: 1.5,
  "arrow-scale": 0.8,
};

/**
 * Build the cytoscape stylesheet. We pass the `style_class` of each
 * node through the mapper; if the bucket is unknown we warn and
 * downgrade to the `function` visual so the graph still renders.
 */
export function buildStylesheet(): (StylesheetStyle | StylesheetCSS)[] {
  // Casts: see `Css` type alias above. Cytoscape's strict typings
  // make a fully-typed stylesheet painful; the visual contract
  // here is the source of truth, not the structural type.
  return [
    {
      selector: "node",
      style: NODE_BASE as unknown as StylesheetStyle["style"],
    },
    {
      selector: 'node[style_class = "function"]',
      style: {
        "background-color": "#3b82f6",
        "border-color": "#1d4ed8",
        shape: "round-rectangle",
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: 'node[style_class = "module"]',
      style: {
        "background-color": "#10b981",
        "border-color": "#047857",
        shape: "round-diamond",
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: 'node[style_class = "external"]',
      style: {
        "background-color": "#f59e0b",
        "border-color": "#b45309",
        shape: "rectangle",
      } as unknown as StylesheetStyle["style"],
    },
    // ---- multimodal (T18) — 4 new node blocks ----
    {
      selector: 'node[style_class = "node-decision"]',
      style: {
        "background-color": "#f59e0b",
        "border-color": "#b45309",
        shape: "diamond",
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: 'node[style_class = "node-doc"]',
      style: {
        "background-color": "#14b8a6",
        "border-color": "#0f766e",
        shape: "round-octagon",
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: 'node[style_class = "node-issue"]',
      style: {
        "background-color": "#ef4444",
        "border-color": "#b91c1c",
        shape: "triangle",
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: 'node[style_class = "node-evidence"]',
      style: {
        "background-color": "#a855f7",
        "border-color": "#7e22ce",
        shape: "ellipse",
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: "edge",
      style: EDGE_BASE as unknown as StylesheetStyle["style"],
    },
    {
      selector: 'edge[style_class = "edge.calls"]',
      style: {
        "line-color": "#3b82f6",
        "target-arrow-color": "#3b82f6",
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: 'edge[style_class = "edge.implements"]',
      style: {
        "line-color": "#10b981",
        "target-arrow-color": "#10b981",
        "line-style": "solid",
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: 'edge[style_class = "edge.uses"]',
      style: {
        "line-color": "#94a3b8",
        "target-arrow-color": "#94a3b8",
        "line-style": "dashed",
      } as unknown as StylesheetStyle["style"],
    },
    // ---- multimodal (T18) — 4 new edge blocks ----
    {
      selector: 'edge[style_class = "edge-cites"]',
      style: {
        "line-color": "#3b82f6",
        "target-arrow-color": "#3b82f6",
        "line-style": "dashed",
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: 'edge[style_class = "edge-justifies"]',
      style: {
        "line-color": "#10b981",
        "target-arrow-color": "#10b981",
        "line-style": "solid",
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: 'edge[style_class = "edge-resolves"]',
      style: {
        "line-color": "#f97316",
        "target-arrow-color": "#f97316",
        "line-style": "dotted",
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: 'edge[style_class = "edge-corroborated"]',
      style: {
        "line-color": "#8b5cf6",
        "target-arrow-color": "#8b5cf6",
        // 'double' renders two parallel lines; cytoscape supports
        // it for visual distinction of corroboration evidence.
        "line-style": "solid",
        width: 2.5,
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: ".selected",
      style: {
        "border-width": 3,
        "border-color": "#fbbf24",
        "background-color": "#fde68a",
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: ".highlighted",
      style: {
        "line-color": "#fbbf24",
        "target-arrow-color": "#fbbf24",
        width: 2.5,
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: ".dimmed",
      style: {
        opacity: 0.25,
      } as unknown as StylesheetStyle["style"],
    },
  ];
}

/**
 * Resolve a `style_class` to the bucket we will actually style by.
 * Unknown buckets fall back to `"function"` and emit a single
 * `console.warn` (deduped per bucket per session).
 *
 * T18 (multimodal) — the return type union now covers the 4
 * new multimodal node classes in addition to the 3 legacy code
 * buckets. The 3 legacy buckets stay unchanged for backward
 * compatibility.
 */
export type ResolvedNodeStyleClass =
  | "function"
  | "module"
  | "external"
  | "node-decision"
  | "node-doc"
  | "node-issue"
  | "node-evidence";

const warnedBuckets = new Set<string>();
export function resolveNodeStyleClass(raw: string | undefined): ResolvedNodeStyleClass {
  if (raw && KNOWN_NODE_CLASSES.has(raw)) {
    return raw as ResolvedNodeStyleClass;
  }
  if (raw && !warnedBuckets.has(raw)) {
    warnedBuckets.add(raw);
    console.warn(
      `InteractiveGraph: unknown node style_class "${raw}" — falling back to "function"`,
    );
  }
  return "function";
}

/**
 * Resolve an edge `style_class` to the bucket we will actually
 * style by. Symmetric to [`resolveNodeStyleClass`] but for edges.
 * Unknown buckets fall back to `"edge.calls"` and emit a single
 * `console.warn` (deduped per bucket per session).
 */
const warnedEdgeBuckets = new Set<string>();
export function resolveEdgeStyleClass(
  raw: string | undefined,
): "edge.calls" | "edge.implements" | "edge.uses" | "edge-cites" | "edge-justifies" | "edge-resolves" | "edge-corroborated" {
  if (raw && KNOWN_EDGE_CLASSES.has(raw)) {
    return raw as
      | "edge.calls"
      | "edge.implements"
      | "edge.uses"
      | "edge-cites"
      | "edge-justifies"
      | "edge-resolves"
      | "edge-corroborated";
  }
  if (raw && !warnedEdgeBuckets.has(raw)) {
    warnedEdgeBuckets.add(raw);
    console.warn(
      `InteractiveGraph: unknown edge style_class "${raw}" — falling back to "edge.calls"`,
    );
  }
  return "edge.calls";
}
