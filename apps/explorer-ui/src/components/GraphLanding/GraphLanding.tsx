/**
 * `GraphLanding` — initial graph view shown when `activeObjectId === null`.
 *
 * Renders the landing page payload (nodes, edges, entry points, hot paths,
 * god nodes) as a cytoscape graph with special styling for each node type:
 * - entry_point → style_class="entry-point" (green)
 * - hot → style_class="hot" (amber)
 * - god → style_class="god" (purple)
 *
 * Clicking a node dispatches `SELECT_OBJECT { objectId, viewId: "overview" }`
 * which opens the pane stack.
 */
import { useEffect, useRef, lazy, Suspense } from "react";
import cytoscape, { type Core } from "cytoscape";

import { useAppDispatch } from "../../state/context";
import { useLanding } from "../../hooks/useLanding";
import { toCytoscapeElements } from "../InteractiveGraph/adapter";
import { buildStylesheet, resolveNodeStyleClass } from "../InteractiveGraph/stylesheet";

const LandingSuggestionStrip = lazy(() =>
  import("./LandingSuggestionStrip").then((m) => ({ default: m.LandingSuggestionStrip })),
);

const LandingHeader = lazy(() =>
  import("./LandingHeader").then((m) => ({ default: m.LandingHeader })),
);

export function GraphLanding({ workspaceId }: { workspaceId: string }) {
  const dispatch = useAppDispatch();
  const containerRef = useRef<HTMLDivElement | null>(null);
  const cyRef = useRef<Core | null>(null);
  const { data, isLoading, error } = useLanding(workspaceId);

  // Mount cytoscape when data arrives
  useEffect(() => {
    if (!data || !containerRef.current) return;
    if (data.nodes.length === 0) return;

    // Apply style classes for landing page node types
    const nodesWithLandingStyle = data.nodes.map((n) => {
      // Check if this node is an entry point
      const isEntryPoint = data.entry_points.some((ep) => ep.id === n.id);
      // Check if this node is a hot path
      const isHot = data.hot_paths.some((hp) => hp.id === n.id);
      // Check if this node is a god node
      const isGod = data.god_nodes.some((g) => g.id === n.id);

      const style_class = isEntryPoint
        ? "entry-point"
        : isHot
          ? "hot"
          : isGod
            ? "god"
            : resolveNodeStyleClass(n.style_class);

      return { ...n, style_class };
    });

    const elements = toCytoscapeElements(nodesWithLandingStyle, data.edges);

    const cy = cytoscape({
      container: containerRef.current,
      elements: elements as unknown as cytoscape.ElementDefinition[],
      style: buildStylesheet(),
      layout: { name: "preset" },
      wheelSensitivity: 0.25,
    });

    // Wire node selection
    const handler = (event: cytoscape.EventObject) => {
      const target = event.target;
      if (target && typeof target === "object" && "id" in target) {
        const id = String((target as cytoscape.NodeSingular).id());
        if (id) {
          dispatch({
            type: "SELECT_OBJECT",
            payload: { objectId: id, viewId: "overview" },
          });
        }
      }
    };
    cy.on("tap", "node", handler);

    cyRef.current = cy;

    // Run a simple layout
    cy.layout({ name: "circle", animate: false }).run();

    return () => {
      cy.off("tap", "node", handler);
      cy.destroy();
      cyRef.current = null;
    };
  }, [data, dispatch]);

  if (isLoading) {
    return (
      <div
        data-testid="graph-landing-loading"
        style={{
          height: "100%",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--color-text-muted)",
          fontSize: 12,
        }}
      >
        Loading workspace…
      </div>
    );
  }

  if (error || !data) {
    return (
      <div
        data-testid="graph-landing-error"
        style={{
          height: "100%",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--color-text-muted)",
          fontSize: 12,
        }}
      >
        Failed to load workspace data.
      </div>
    );
  }

  return (
    <div
      data-testid="graph-landing"
      style={{ height: "100%", width: "100%", display: "flex", flexDirection: "column" }}
    >
      {/* Header */}
      <Suspense fallback={<div style={{ height: 48 }} />}>
        <LandingHeader workspace={data.workspace} />
      </Suspense>

      {/* Graph canvas */}
      <div
        ref={containerRef}
        data-testid="graph-landing-canvas"
        style={{ flex: "1 1 auto", minHeight: 0 }}
      />

      {/* Suggestion strip */}
      <Suspense fallback={null}>
        <LandingSuggestionStrip
          suggestedQuestions={data.suggested_questions}
          onAsk={() => {
            // Dispatch ask action — the Ask panel will handle the question
            dispatch({ type: "SET_SPOTTER", payload: { open: true } });
          }}
        />
      </Suspense>
    </div>
  );
}
