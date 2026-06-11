/**
 * `NeighborMinigraph` — fourth section of the `ContextualPanel`.
 * A small cytoscape canvas rendering the same-level callers +
 * callees of the focus. Clicking a node refocusses the panel.
 *
 * Reuses the cytoscape stylesheet + element adapter from
 * `components/InteractiveGraph` so the visual language is
 * consistent. Cleanup is strict: `useEffect` returns a function
 * that calls `cy.destroy()` to avoid leaks on rapid focus changes.
 */
import { useEffect, useRef } from "react";
import cytoscape, { type Core } from "cytoscape";

import type { GraphEdge, GraphNode } from "../../api/types";
import { buildStylesheet, resolveNodeStyleClass } from "../InteractiveGraph/stylesheet";
import { toCytoscapeElements } from "../InteractiveGraph/adapter";

export interface NeighborMinigraphProps {
  /** The focus node — drawn at the center with a distinct style. */
  focus: GraphNode;
  /** Same-level nodes (callers + callees). */
  nodes: GraphNode[];
  /** Same-level edges. */
  edges: GraphEdge[];
  /** Called when the user clicks a node. */
  onFocus: (id: string) => void;
  /** Optional CSS class passthrough. */
  className?: string;
}

export function NeighborMinigraph({
  focus,
  nodes,
  edges,
  onFocus,
  className,
}: NeighborMinigraphProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const cyRef = useRef<Core | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    // Pre-compute the full set of nodes: focus + neighbors.
    const allNodes: GraphNode[] = [
      focus,
      ...nodes.map((n) => ({ ...n, style_class: resolveNodeStyleClass(n.style_class) })),
    ];
    const elements = toCytoscapeElements(allNodes, edges);
    const cy = cytoscape({
      container: containerRef.current,
      elements: elements as unknown as cytoscape.ElementDefinition[],
      style: buildStylesheet(),
      layout: { name: "preset" },
      wheelSensitivity: 0.25,
    });
    // Wire node taps to the onFocus callback. The cytoscape
    // `NodeSingular` type declares `id()` as a method, but the
    // test mock exposes it as a property — read both for
    // compatibility.
    const handler = (event: cytoscape.EventObject) => {
      const target = event.target;
      if (!target || typeof target !== "object") return;
      const idRaw = (target as { id?: string | (() => string) }).id;
      const id = typeof idRaw === "function" ? idRaw() : idRaw;
      if (typeof id === "string" && id.length > 0) onFocus(id);
    };
    cy.on("tap", "node", handler);
    cyRef.current = cy;
    return () => {
      cy.off("tap", "node", handler);
      cy.destroy();
      cyRef.current = null;
    };
  }, [focus, nodes, edges, onFocus]);

  return (
    <div
      data-testid="neighbor-minigraph"
      className={className}
      style={{
        height: 220,
        width: "100%",
        borderRadius: 4,
        overflow: "hidden",
        border: "1px solid var(--color-border)",
        backgroundColor: "var(--color-surface-raised)",
      }}
    >
      <div
        ref={containerRef}
        data-testid="neighbor-minigraph-canvas"
        style={{ width: "100%", height: "100%" }}
      />
    </div>
  );
}
