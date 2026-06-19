/**
 * `stylesheet.ts` — cytoscape styles for the InteractiveGraph.
 *
 * Buckets follow the wire `style_class` taxonomy from the backend:
 * - Nodes: `function` (default), `module`, `external`,
 *   `node-decision` (multimodal), `node-doc` (multimodal),
 *   `node-issue` (multimodal), `node-evidence` (multimodal),
 *   `node-component` (C4), `node-container` (C4), `node-system` (C4)
 * - Edges: `edge.calls` (default), `edge.implements`, `edge.uses`,
 *   `edge-cites` (multimodal), `edge-justifies` (multimodal),
 *   `edge-resolves` (multimodal), `edge-corroborated` (multimodal),
 *   `edge-part-of` (C4), `edge-deployed-as` (C4), `edge-in-system` (C4)
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
 *
 * T-Phase-1 (C4 architecture) — adds 3 node blocks and 3 edge blocks.
 * Shapes and palette follow the C4-model visual convention:
 *   component  → blue rounded rectangle
 *   container  → purple wider rectangle
 *   system     → gray large rectangle with dashed border
 *   part-of    → solid blue
 *   deployed-as → solid purple
 *   in-system  → solid gray
 */
import type { Core, StylesheetCSS, StylesheetStyle } from "cytoscape";

export const KNOWN_NODE_CLASSES = new Set([
  "function",
  "module",
  "external",
  // ---- multimodal (T18) ----
  "node-decision",
  "node-doc",
  "node-issue",
  "node-evidence",
  // ---- C4 architecture (Phase 1) ----
  "node-component",
  "node-container",
  "node-system",
  "node-code",
  // ---- Landing Page (E4 ADR-039) ----
  "entry-point",
  "hot",
  "god",
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
  // ---- C4 architecture (Phase 1) ----
  "edge-part-of",
  "edge-deployed-as",
  "edge-in-system",
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
    // ---- C4 architecture (Phase 1) — 3 new node blocks ----
    //
    // C4 model: Component < Container < System (zoom-out
    // hierarchy). Visual encoding uses color + size to reflect
    // granularity so a glance at the canvas reveals which
    // architectural layer dominates.
    {
      selector: 'node[style_class = "node-component"]',
      style: {
        "background-color": "#60a5fa",
        "border-color": "#1d4ed8",
        shape: "round-rectangle",
        // Smaller than containers; components are leaf-ish
        // groupings of related symbols.
        width: 96,
        height: 40,
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: 'node[style_class = "node-container"]',
      style: {
        "background-color": "#a78bfa",
        "border-color": "#6d28d9",
        shape: "rectangle",
        // Wider than components; containers are deployable
        // units that hold multiple components.
        width: 140,
        height: 64,
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: 'node[style_class = "node-system"]',
      style: {
        "background-color": "#cbd5e1",
        "border-color": "#475569",
        // C4 convention: dashed border signals a system
        // boundary (the "outermost" zoom level).
        "border-style": "dashed",
        "border-width": 2,
        shape: "rectangle",
        // Largest of the three; systems hold multiple
        // containers.
        width: 200,
        height: 96,
      } as unknown as StylesheetStyle["style"],
    },
    // ---- C4 Code (E6 ADR-039) — leaf symbols inside components ----
    {
      selector: 'node[style_class = "node-code"]',
      style: {
        "background-color": "#d1d5db",
        "border-color": "#6b7280",
        shape: "ellipse",
        // Smallest; code symbols are leaf-level entities
        width: 64,
        height: 28,
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
    // ---- C4 architecture (Phase 1) — 3 new edge blocks ----
    //
    // C4-model relationships are containment / deployment
    // edges, not code-level dependencies. Encoded in solid lines
    // with the colour of the SOURCE node so the hierarchy reads
    // top-down (Component ⊂ Container ⊂ System).
    {
      selector: 'edge[style_class = "edge-part-of"]',
      style: {
        "line-color": "#60a5fa",
        "target-arrow-color": "#60a5fa",
        "line-style": "solid",
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: 'edge[style_class = "edge-deployed-as"]',
      style: {
        "line-color": "#a78bfa",
        "target-arrow-color": "#a78bfa",
        "line-style": "solid",
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: 'edge[style_class = "edge-in-system"]',
      style: {
        "line-color": "#94a3b8",
        "target-arrow-color": "#94a3b8",
        "line-style": "solid",
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
    // ---- Landing Page (E4 ADR-039) — special node types ----
    {
      selector: 'node[style_class = "entry-point"]',
      style: {
        "background-color": "#10b981",
        "border-color": "#047857",
        shape: "round-rectangle",
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: 'node[style_class = "hot"]',
      style: {
        "background-color": "#f59e0b",
        "border-color": "#b45309",
        shape: "diamond",
      } as unknown as StylesheetStyle["style"],
    },
    {
      selector: 'node[style_class = "god"]',
      style: {
        "background-color": "#a855f7",
        "border-color": "#7e22ce",
        shape: "ellipse",
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
 *
 * T-Phase-1 (C4 architecture) — adds the 3 C4 node classes
 * (`node-component`, `node-container`, `node-system`).
 *
 * E6 (ADR-039) — adds `node-code` for C4 code symbols.
 */
export type ResolvedNodeStyleClass =
  | "function"
  | "module"
  | "external"
  | "node-decision"
  | "node-doc"
  | "node-issue"
  | "node-evidence"
  | "node-component"
  | "node-container"
  | "node-system"
  | "node-code"
  // ---- Landing Page (E4 ADR-039) ----
  | "entry-point"
  | "hot"
  | "god";

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
 *
 * T-Phase-1 (C4 architecture) — adds the 3 C4 edge classes
 * (`edge-part-of`, `edge-deployed-as`, `edge-in-system`).
 */
const warnedEdgeBuckets = new Set<string>();
export function resolveEdgeStyleClass(
  raw: string | undefined,
):
  | "edge.calls"
  | "edge.implements"
  | "edge.uses"
  | "edge-cites"
  | "edge-justifies"
  | "edge-resolves"
  | "edge-corroborated"
  | "edge-part-of"
  | "edge-deployed-as"
  | "edge-in-system" {
  if (raw && KNOWN_EDGE_CLASSES.has(raw)) {
    return raw as
      | "edge.calls"
      | "edge.implements"
      | "edge.uses"
      | "edge-cites"
      | "edge-justifies"
      | "edge-resolves"
      | "edge-corroborated"
      | "edge-part-of"
      | "edge-deployed-as"
      | "edge-in-system";
  }
  if (raw && !warnedEdgeBuckets.has(raw)) {
    warnedEdgeBuckets.add(raw);
    console.warn(
      `InteractiveGraph: unknown edge style_class "${raw}" — falling back to "edge.calls"`,
    );
  }
  return "edge.calls";
}

/**
 * Apply corroboration score-based styles to edges in a cytoscape
 * instance. Iterates over `scores` entries where the key is
 * `"source->target"` and the value is a score in [0, 1].
 *
 * - `width`  = 1.5 + score * 3   (range 1.5–4.5)
 * - `opacity` = 0.5 + score * 0.5 (range 0.5–1.0)
 *
 * Edges that do not match any score entry are left unchanged.
 */
export function applyCorroborationStyles(
  cy: Core,
  scores: Record<string, number>,
): void {
  for (const [key, score] of Object.entries(scores)) {
    const parts = key.split("->");
    if (parts.length !== 2) continue;
    const [source, target] = parts as [string, string];
    // Use cytoscape's attribute selector to find matching edges.
    const edges = cy.edges(`[source = "${source}"][target = "${target}"]`);
    if (edges.length === 0) continue;
    edges.style("width", `${1.5 + score * 3}`);
    edges.style("opacity", `${0.5 + score * 0.5}`);
  }
}
