/**
 * `InteractiveGraph` — interactive cytoscape-backed graph.
 *
 * Renders the `SubgraphResponse` from `GET /api/graph/:id/subgraph`
 * into a cytoscape instance, with a fallback `<table>` that lists
 * every node (for screen readers + keyboard users).
 *
 * Sprint E2 (ADR-039): layout now computed via ELK.js worker
 * (`layout.worker.ts`) with selectable algorithm (layered/force/radial).
 * The worker runs asynchronously after cytoscape mounts; positions are
 * applied via preset layout. Animation support and cancellation are
 * wired through the worker's progress callbacks.
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
import { useEffect, useRef, useState, useCallback } from "react";
import cytoscape, { type Core, type ElementsDefinition } from "cytoscape";

import type { SubgraphResponse } from "../../api/types";
import { buildStylesheet, resolveNodeStyleClass } from "./stylesheet";
import { toCytoscapeElements } from "./adapter";
import {
  createLayoutWorker,
  type LayoutAlgorithm,
  type LayoutWorker,
  LayoutCancelled,
} from "./layout.worker";

export type { LayoutAlgorithm };

/** Max nodes for animated layout. Beyond this, animation is skipped. */
const MAX_ANIMATED_NODES = 500;

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
  /**
   * Optional callback fired after the cytoscape instance is mounted
   * and initialised. Receives the `Core` instance so callers can
   * apply dynamic styles (e.g. corroboration scores).
   */
  onCyReady?: (cy: Core) => void;
  /**
   * Layout algorithm. Defaults to "layered" (ELK layered, left-to-right).
   * "force" uses ELK's force-directed algorithm.
   * "radial" uses ELK's radial layout.
   */
  layoutAlgorithm?: LayoutAlgorithm;
}

export function InteractiveGraph({
  root,
  data,
  selectedId,
  onSelectObject,
  onCyReady,
  className,
  layoutAlgorithm: layoutAlgorithmProp = "layered",
}: InteractiveGraphProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const cyRef = useRef<Core | null>(null);
  const workerRef = useRef<LayoutWorker | null>(null);
  const [mounted, setMounted] = useState(false);
  const [layoutProgress, setLayoutProgress] = useState<number | null>(null);
  // E2: local layout algorithm state (synced from prop, changeable via selector)
  const [layoutAlgorithm, setLayoutAlgorithm] = useState<LayoutAlgorithm>(layoutAlgorithmProp);

  // ---- Mount cytoscape when we have data + a container ----
  useEffect(() => {
    if (!data || !containerRef.current) return;
    if (data.nodes.length === 0) return;

    // Resolve any unknown `style_class` on a node to the default
    const safeNodes = data.nodes.map((n) => ({
      ...n,
      style_class: resolveNodeStyleClass(n.style_class),
    }));
    const elements = toCytoscapeElements(safeNodes, data.edges);

    const cy = cytoscape({
      container: containerRef.current,
      elements: elements as unknown as cytoscape.ElementDefinition[],
      style: buildStylesheet(),
      // Start with preset (empty positions). The worker will compute
      // positions asynchronously and update them.
      layout: { name: "preset" },
      wheelSensitivity: 0.25,
    });

    // Wire node selection
    const handler = (event: cytoscape.EventObject) => {
      const target = event.target;
      if (target && typeof target === "object" && "id" in target) {
        const id = String((target as cytoscape.NodeSingular).id());
        if (id) onSelectObject(id);
      }
    };
    cy.on("tap", "node", handler);

    cyRef.current = cy;
    if (onCyReady) {
      onCyReady(cy);
    }

    // ── E2: ELK layout worker ──────────────────────────────
    const nodeCount = data.nodes.length;
    const animate = nodeCount <= MAX_ANIMATED_NODES;
    const worker = createLayoutWorker();
    workerRef.current = worker;

    // Progress callback: update state for UI feedback
    const unsub = worker.onProgress((p) => {
      setLayoutProgress(p);
    });

    // Run layout asynchronously
    worker
      .layout(elements, {
        algorithm: layoutAlgorithm,
        animate,
      })
      .then((positioned: ElementsDefinition) => {
        // Apply computed positions to the cytoscape nodes
        cy.batch(() => {
          for (const node of positioned.nodes ?? []) {
            const cyNode = cy.getElementById(String(node.data.id));
            if (cyNode.length > 0 && node.position) {
              cyNode.position({
                x: node.position.x,
                y: node.position.y,
              });
            }
          }
        });
        // Fit the viewport to show the positioned graph
        cy.fit(undefined, 20);
        setLayoutProgress(1);
      })
      .catch((err: unknown) => {
        if (err instanceof LayoutCancelled) {
          return; // Silently ignore — component unmounted
        }
        console.warn("Layout worker failed:", err);
        // Fallback: run cytoscape's built-in layout
        cy.layout({ name: "grid", rows: Math.ceil(Math.sqrt(nodeCount)) }).run();
        setLayoutProgress(null);
      });

    queueMicrotask(() => setMounted(true));
    return () => {
      worker.cancel();
      unsub();
      cy.off("tap", "node", handler);
      cy.destroy();
      cyRef.current = null;
      workerRef.current = null;
      setMounted(false);
      setLayoutProgress(null);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [root, data, layoutAlgorithm]);

  // ── E2: re-layout when algorithm changes (no cytoscape remount) ──
  // Separate effect so we don't destroy/recreate the cy instance.
  useEffect(() => {
    const cy = cyRef.current;
    if (!cy || !mounted) return;
    const nodeCount = cy.nodes().length;
    if (nodeCount === 0) return;

    const animate = nodeCount <= MAX_ANIMATED_NODES;
    // Get current elements from cytoscape
    const elements: ElementsDefinition = {
      nodes: cy.nodes().map((n) => ({
        data: {
          id: n.id(),
          label: String(n.data("label") ?? ""),
          kind: String(n.data("kind") ?? "symbol"),
          file: n.data("file") as string | null ?? null,
          line: n.data("line") as number | null ?? null,
          style_class: n.data("style_class") as string | null ?? null,
        },
      })),
      edges: cy.edges().map((e) => ({
        data: {
          id: e.id(),
          source: e.data("source"),
          target: e.data("target"),
          relation: e.data("relation"),
          style_class: e.data("style_class"),
        },
      })),
    };

    const worker = createLayoutWorker();
    const unsub = worker.onProgress((p) => setLayoutProgress(p));

    worker
      .layout(elements, { algorithm: layoutAlgorithm, animate })
      .then((positioned: ElementsDefinition) => {
        cy.batch(() => {
          for (const node of positioned.nodes ?? []) {
            const cyNode = cy.getElementById(String(node.data.id));
            if (cyNode.length > 0 && node.position) {
              cyNode.position({ x: node.position.x, y: node.position.y });
            }
          }
        });
        cy.fit(undefined, 20);
        setLayoutProgress(1);
      })
      .catch((err: unknown) => {
        if (err instanceof LayoutCancelled) return;
        console.warn("Layout worker re-layout failed:", err);
        setLayoutProgress(null);
      });

    return () => {
      worker.cancel();
      unsub();
    };
  }, [layoutAlgorithm, mounted]);

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
      {/* E2: Layout algorithm selector + progress indicator */}
      <div
        className="flex items-center gap-2 px-2 py-1 text-xs"
        style={{
          backgroundColor: "var(--color-surface-raised)",
          borderBottom: "1px solid var(--color-border)",
        }}
      >
        <span style={{ color: "var(--color-text-muted)" }}>Layout:</span>
        {(["layered", "force", "radial"] as LayoutAlgorithm[]).map((alg) => (
          <button
            key={alg}
            type="button"
            onClick={() => setLayoutAlgorithm(alg)}
            aria-pressed={layoutAlgorithm === alg}
            className={`rounded px-2 py-0.5 transition-colors ${
              layoutAlgorithm === alg
                ? "font-semibold"
                : "opacity-60 hover:opacity-100"
            }`}
            style={{
              backgroundColor:
                layoutAlgorithm === alg
                  ? "var(--color-surface-overlay)"
                  : "transparent",
              color: "var(--color-text-primary)",
              border:
                layoutAlgorithm === alg
                  ? "1px solid var(--color-border)"
                  : "1px solid transparent",
            }}
          >
            {alg}
          </button>
        ))}
        {layoutProgress !== null && layoutProgress < 1 && (
          <span
            className="ml-2"
            style={{ color: "var(--color-text-muted)" }}
            aria-live="polite"
          >
            Layout: {Math.round(layoutProgress * 100)}%
          </span>
        )}
        <span className="ml-auto text-xs" style={{ color: "var(--color-text-muted)" }}>
          {data.nodes.length} nodes
        </span>
      </div>
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
