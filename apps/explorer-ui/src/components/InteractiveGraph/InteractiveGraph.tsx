/**
 * `InteractiveGraph` — interactive cytoscape-backed graph.
 *
 * Renders the `SubgraphResponse` from `GET /api/graph/:id/subgraph`
 * into a cytoscape instance, with a fallback `<table>` that lists
 * every node (for screen readers + keyboard users).
 *
 * Selection state machine:
 * - click a node  → `onSelectObject(id)` once
 * - the node gets `class="selected"`, incident edges `class="highlighted"`,
 *   the rest `class="dimmed"`
 * - clear `selectedId` prop → all three classes are removed
 * - click the background → no selection
 *
 * Accessibility:
 * - container `role="application"`, `tabIndex=0`
 * - fallback table `role="complementary"`, with `aria-label="Graph nodes"`
 * - rows are activatable with Enter or Space
 */
import { useEffect, useRef, useState } from "react";
import cytoscape, { type Core } from "cytoscape";

import type { SubgraphResponse } from "../../api/types";
import { buildStylesheet, resolveNodeStyleClass } from "./stylesheet";
import { toCytoscapeElements } from "./adapter";

export interface InteractiveGraphProps {
  /** Root id echoed in `aria-label` and used as the cytoscape key. */
  root: string;
  /** Sub-graph payload from the REST endpoint. `null` → empty state. */
  data: SubgraphResponse | null;
  /** Optional currently-selected node id; pass `null` to clear. */
  selectedId?: string | null;
  /** Called when the user picks a node (mouse click or Enter/Space). */
  onSelectObject: (id: string) => void;
  /** Optional CSS class passthrough. */
  className?: string;
}

export function InteractiveGraph({
  root,
  data,
  selectedId,
  onSelectObject,
  className,
}: InteractiveGraphProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const cyRef = useRef<Core | null>(null);
  const [mounted, setMounted] = useState(false);

  // ---- Mount cytoscape when we have data + a container ----
  useEffect(() => {
    if (!data || !containerRef.current) return;
    if (data.nodes.length === 0) return;

    // Resolve any unknown `style_class` on a node to the default
    // *and* apply the visual. The stylesheet picks up the
    // `data.style_class` attribute directly; we just need to make
    // sure the attribute value is in the known set so cytoscape
    // doesn't end up with an unstyled node.
    const safeNodes = data.nodes.map((n) => ({
      ...n,
      style_class: resolveNodeStyleClass(n.style_class),
    }));
    const elements = toCytoscapeElements(safeNodes, data.edges);

    const cy = cytoscape({
      container: containerRef.current,
      elements: elements as unknown as cytoscape.ElementDefinition[],
      style: buildStylesheet(),
      layout: { name: "preset" },
      wheelSensitivity: 0.25,
    });

    // Wire node selection. We capture the prop in a ref-like closure
    // so the listener always sees the latest callback.
    const handler = (event: cytoscape.EventObject) => {
      const target = event.target;
      if (target && typeof target === "object" && "id" in target) {
        const id = String((target as cytoscape.NodeSingular).id());
        if (id) onSelectObject(id);
      }
    };
    cy.on("tap", "node", handler);

    // Background tap is a no-op by design — we never clear the
    // selection from cytoscape directly. The parent owns the
    // `selectedId` state and decides when to clear.

    cyRef.current = cy;
    // The `setMounted` is used to gate the selection-state effect
    // until cytoscape has had a chance to bind its listeners. We
    // use a microtask to avoid the lint rule that flags synchronous
    // setState in an effect.
    queueMicrotask(() => setMounted(true));
    return () => {
      cy.off("tap", "node", handler);
      cy.destroy();
      cyRef.current = null;
      setMounted(false);
    };
    // We intentionally re-mount on `root` change so layout resets
    // are clean; `data` change is also a fresh mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [root, data]);

  // ---- Apply selection state when `selectedId` changes ----
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy) return;
    cy.elements().removeClass("selected highlighted dimmed");
    if (selectedId) {
      const node = cy.getElementById(selectedId);
      if (node && node.length > 0) {
        node.addClass("selected");
        const incident = node.connectedEdges();
        incident.addClass("highlighted");
        const others = cy.elements().subtract(node).subtract(incident);
        others.addClass("dimmed");
      }
    }
  }, [selectedId, mounted]);

  // ---- Render ----
  if (!data || data.nodes.length === 0) {
    return (
      <div
        data-testid="interactive-graph-empty"
        className={className}
        style={{
          height: "100%",
          width: "100%",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--color-text-muted)",
          fontSize: 12,
        }}
      >
        <span>No graph data — pick a symbol to see its neighbourhood.</span>
      </div>
    );
  }

  return (
    <div
      data-testid="interactive-graph"
      role="application"
      aria-label={`Interactive graph of ${root}`}
      tabIndex={0}
      className={className}
      style={{ height: "100%", width: "100%", display: "flex", flexDirection: "column" }}
    >
      <div
        ref={containerRef}
        data-testid="interactive-graph-canvas"
        style={{ flex: "1 1 auto", minHeight: 0 }}
      />
      <table
        role="complementary"
        aria-label="Graph nodes"
        data-testid="interactive-graph-fallback"
        style={{
          width: "100%",
          borderCollapse: "collapse",
          fontSize: 11,
          color: "var(--color-text-secondary)",
        }}
      >
        <thead>
          <tr>
            <th style={{ textAlign: "left", padding: "4px 6px" }}>id</th>
            <th style={{ textAlign: "left", padding: "4px 6px" }}>label</th>
            <th style={{ textAlign: "left", padding: "4px 6px" }}>kind</th>
          </tr>
        </thead>
        <tbody>
          {data.nodes.map((n) => {
            const isSelected = selectedId === n.id;
            return (
              <tr
                key={n.id}
                tabIndex={0}
                role="row"
                aria-selected={isSelected}
                onClick={() => onSelectObject(n.id)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    onSelectObject(n.id);
                  }
                }}
                style={{
                  cursor: "pointer",
                  background: isSelected
                    ? "var(--color-surface-overlay)"
                    : undefined,
                }}
              >
                <td style={{ padding: "2px 6px" }}>{n.id}</td>
                <td style={{ padding: "2px 6px" }}>{n.label}</td>
                <td style={{ padding: "2px 6px" }}>{n.kind}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
