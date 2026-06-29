/**
 * C4 level selector — pure data layer.
 *
 * No React imports. All functions are pure and unit-testable in isolation.
 */
import type { ArchitecturePayload } from "../api/types";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** C4 zoom levels, lowest-to-highest zoom (cumulative inclusion). */
export type C4Level = "c4-context" | "c4-container" | "c4-component" | "c4-code";

/** Full perspective union. */
export type Perspective = "graph" | C4Level;

// ---------------------------------------------------------------------------
// Node class map
// ---------------------------------------------------------------------------

/**
 * Which node.style_class values are visible at each C4 level.
 * Inclusion is cumulative: each level includes all broader levels' node classes.
 */
export const C4_LEVEL_NODE_CLASSES: Readonly<Record<C4Level, ReadonlySet<string>>> = {
  "c4-context":   new Set(["node-system"]),
  "c4-container":  new Set(["node-system", "node-container"]),
  "c4-component": new Set(["node-system", "node-container", "node-component"]),
  "c4-code":      new Set(["node-system", "node-container", "node-component", "node-code"]),
};

// ---------------------------------------------------------------------------
// Perspective options (UI descriptor)
// ---------------------------------------------------------------------------

export interface PerspectiveOption {
  readonly id: Perspective;
  readonly label: string;
  /** "basic" badge shown on uncurated levels; null = no badge */
  readonly badge: string | null;
}

export const PERSPECTIVE_OPTIONS: readonly PerspectiveOption[] = [
  { id: "graph",        label: "Graph",      badge: null } as const,
  { id: "c4-context",  label: "Context",   badge: null } as const,
  { id: "c4-container",label: "Container", badge: null } as const,
  { id: "c4-component",label: "Component", badge: "basic" } as const,
  { id: "c4-code",     label: "Code",      badge: "basic" } as const,
] as const;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** True when the perspective is the graph (symbol neighbourhood) canvas. */
export function isGraphPerspective(p: Perspective): boolean {
  return p === "graph";
}

// ---------------------------------------------------------------------------
// Level filter
// ---------------------------------------------------------------------------

/**
 * Returns the set of node.style_class values visible at the given C4 level.
 * Throws if called with a non-C4 Perspective.
 */
export function nodeClassesForLevel(level: C4Level): ReadonlySet<string> {
  return C4_LEVEL_NODE_CLASSES[level];
}

/**
 * Filter an ArchitecturePayload to only the nodes/edges visible at `perspective`.
 * - For `"graph"`: returns the payload unchanged (no filtering).
 * - For a C4 level: returns only nodes whose style_class is in that level's
 *   set, plus edges whose source and target both survive the filter.
 */
export function filterArchitectureByLevel<T extends ArchitecturePayload>(
  payload: T,
  perspective: Perspective,
): T {
  if (perspective === "graph") return payload;

  const level = perspective as C4Level;
  const allowed = C4_LEVEL_NODE_CLASSES[level];
  const allowedNodeIds = new Set(
    payload.nodes.filter((n) => allowed.has(n.style_class ?? "")).map((n) => n.id),
  );

  const filteredNodes = payload.nodes.filter((n) => allowed.has(n.style_class ?? ""));
  const filteredEdges = payload.edges.filter(
    (e) => allowedNodeIds.has(e.source) && allowedNodeIds.has(e.target),
  );

  return {
    ...payload,
    nodes: filteredNodes,
    edges: filteredEdges,
  };
}
