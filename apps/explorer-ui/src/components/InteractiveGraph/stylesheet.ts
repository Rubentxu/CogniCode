/**
 * `stylesheet.ts` — cytoscape styles for the InteractiveGraph.
 *
 * Buckets follow the wire `style_class` taxonomy from the backend:
 * - Nodes: `function` (default), `module`, `external`
 * - Edges: `edge.calls` (default), `edge.implements`, `edge.uses`
 * - Selection state: `selected`, `highlighted`, `dimmed`
 *
 * Unknown node `style_class` values fall back to the `function`
 * visual + a `console.warn` (per spec R2 of `interactive-graph`).
 */
import type { StylesheetCSS, StylesheetStyle } from "cytoscape";

export const KNOWN_NODE_CLASSES = new Set(["function", "module", "external"]);
export const KNOWN_EDGE_CLASSES = new Set([
  "edge.calls",
  "edge.implements",
  "edge.uses",
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
 */
const warnedBuckets = new Set<string>();
export function resolveNodeStyleClass(raw: string | undefined): "function" | "module" | "external" {
  if (raw && KNOWN_NODE_CLASSES.has(raw)) {
    return raw as "function" | "module" | "external";
  }
  if (raw && !warnedBuckets.has(raw)) {
    warnedBuckets.add(raw);
    console.warn(
      `InteractiveGraph: unknown node style_class "${raw}" — falling back to "function"`,
    );
  }
  return "function";
}
