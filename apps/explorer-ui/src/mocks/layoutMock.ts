/**
 * `layoutMock` — deterministic positions for graph nodes.
 *
 * The backend endpoint `POST /api/diagrams/layout` does not exist
 * yet (deferred to a later cycle). To keep the SVG Graph usable
 * end-to-end, the mock places nodes on a circle in a stable order
 * keyed by node id — same id, same x/y, every render.
 *
 * The component consuming the mock is layout-agnostic: it just
 * reads `{x, y}` from each node. When the backend layout endpoint
 * lands, swap the mock for an SWR hook that calls it.
 *
 * Why a circle?
 * - Trivial to verify in tests (positions are a function of id
 *   only — no randomness).
 * - It works for small N (the call graph view) without ugly
 *   overlap. For larger N, replace with a force-directed mock.
 */
import type { SpotterResult } from "../api/types";

export interface LayoutNode {
  /** Stable id (matches the spotter/object id). */
  id: string;
  /** Display label. */
  label: string;
  /** Object kind — drives the visual style. */
  kind: string;
  /** Computed x position. */
  x: number;
  /** Computed y position. */
  y: number;
}

export interface LayoutEdge {
  /** Source node id. */
  from: string;
  /** Target node id. */
  to: string;
  /** Optional label (relation kind, etc). */
  label?: string;
}

export interface LayoutResult {
  nodes: LayoutNode[];
  edges: LayoutEdge[];
  /**
   * Bounding box of the layout in SVG units. The component
   * uses this to set the `viewBox` so the graph always fits.
   */
  viewBox: { x: number; y: number; width: number; height: number };
}

/**
 * Hash a string into a 32-bit unsigned int. FNV-1a — simple and
 * deterministic. The hash drives the per-node phase so that
 * even identical labels land at different positions.
 */
function hash32(s: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}

/**
 * Lay out the given node ids in a circle of `radius` around the
 * origin. The starting angle is offset by the hash of the first
 * node id so different inputs rotate the layout deterministically.
 */
function circularLayout(
  ids: string[],
  labels: Map<string, string>,
  kinds: Map<string, string>,
  radius: number,
): LayoutNode[] {
  const n = ids.length;
  if (n === 0) return [];
  const startAngle =
    n > 0 ? (hash32(ids[0] ?? "") / 0xffffffff) * Math.PI * 2 : 0;
  return ids.map((id, i) => {
    const angle = startAngle + (i * 2 * Math.PI) / Math.max(n, 1);
    return {
      id,
      label: labels.get(id) ?? id,
      kind: kinds.get(id) ?? "symbol",
      x: Math.round(radius + radius * Math.cos(angle)),
      y: Math.round(radius + radius * Math.sin(angle)),
    };
  });
}

/**
 * Synthesize a small call graph: connect node 0 → every other
 * node, with an additional ring between consecutive nodes. This
 * mirrors what the real call-graph block emits (a root symbol
 * plus its callers / callees).
 */
function defaultEdges(nodeIds: string[]): LayoutEdge[] {
  const edges: LayoutEdge[] = [];
  for (let i = 1; i < nodeIds.length; i++) {
    const from = nodeIds[0];
    const to = nodeIds[i];
    if (from !== undefined && to !== undefined) {
      edges.push({ from, to });
    }
  }
  return edges;
}

/**
 * Build a `LayoutResult` from a list of spotter results. The mock
 * places each result on a circle and draws edges from the top-
 * scored hit to every other hit.
 */
export function layoutFromSpotter(
  results: ReadonlyArray<SpotterResult>,
  options?: { radius?: number },
): LayoutResult {
  const radius = options?.radius ?? 200;
  const ids = results.map((r) => r.object.id);
  const labels = new Map(results.map((r) => [r.object.id, r.object.label]));
  const kinds = new Map(
    results.map((r) => [r.object.id, r.object.object_type]),
  );
  const nodes = circularLayout(ids, labels, kinds, radius);
  const edges = defaultEdges(ids);
  return {
    nodes,
    edges,
    viewBox: {
      x: 0,
      y: 0,
      width: radius * 2,
      height: radius * 2,
    },
  };
}

/**
 * Build a `LayoutResult` from an explicit list of node ids. Use
 * this when the graph is not derived from the spotter (e.g., a
 * fixed fixture or a saved exploration).
 */
export function layoutFromIds(
  ids: string[],
  options?: {
    radius?: number;
    labelOf?: (id: string) => string;
    kindOf?: (id: string) => string;
    edges?: LayoutEdge[];
  },
): LayoutResult {
  const radius = options?.radius ?? 200;
  const labels = new Map(
    ids.map((id) => [id, options?.labelOf?.(id) ?? id]),
  );
  const kinds = new Map(
    ids.map((id) => [id, options?.kindOf?.(id) ?? "symbol"]),
  );
  const nodes = circularLayout(ids, labels, kinds, radius);
  const edges = options?.edges ?? defaultEdges(ids);
  return {
    nodes,
    edges,
    viewBox: { x: 0, y: 0, width: radius * 2, height: radius * 2 },
  };
}

/**
 * Build a LayoutResult from a `ContextualView`. We mine the
 * `callers` and `callees` blocks for the edges, plus the parent
 * `object_id` as the centre node. This is what the LensPanel
 * uses to render the "call graph" lens.
 */
export function layoutFromContextualView(
  view: { object_id: string; blocks: ReadonlyArray<{ id: string; body: unknown }> },
  options?: { radius?: number },
): LayoutResult {
  const radius = options?.radius ?? 220;
  const ids: string[] = [view.object_id];
  const labels = new Map<string, string>([[view.object_id, view.object_id]]);
  const kinds = new Map<string, string>([[view.object_id, "symbol"]]);
  const edges: LayoutEdge[] = [];

  for (const block of view.blocks) {
    if (block.id !== "callers" && block.id !== "callees") continue;
    const body = block.body as {
      items?: Array<{
        object_id: string;
        name: string;
        kind: string;
      }>;
    };
    for (const it of body.items ?? []) {
      ids.push(it.object_id);
      labels.set(it.object_id, it.name);
      kinds.set(it.object_id, it.kind);
      edges.push({
        from: block.id === "callers" ? it.object_id : view.object_id,
        to: block.id === "callers" ? view.object_id : it.object_id,
        label: block.id,
      });
    }
  }

  // De-dupe ids while preserving order.
  const seen = new Set<string>();
  const uniqueIds = ids.filter((id) => {
    if (seen.has(id)) return false;
    seen.add(id);
    return true;
  });

  const nodes = circularLayout(uniqueIds, labels, kinds, radius);
  return {
    nodes,
    edges,
    viewBox: { x: 0, y: 0, width: radius * 2, height: radius * 2 },
  };
}
