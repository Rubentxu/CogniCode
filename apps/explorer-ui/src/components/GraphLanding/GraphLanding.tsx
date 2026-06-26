/**
 * `GraphLanding` — initial graph view shown when `activeObjectId === null`.
 *
 * Renders the landing page payload (nodes, edges, entry points, hot paths,
 * god nodes) as a cytoscape graph with special styling for each node type:
 * - entry_point → style_class="entry-point" (green)
 * - hot → style_class="hot" (amber)
 * - god → style_class="god" (purple)
 *
 * When `perspective === "c4"`, uses `useArchitecture()` to display C4 component
 * nodes (directories) with `part_of` edges instead.
 *
 * Clicking a node dispatches `SELECT_OBJECT { objectId, viewId: "overview" }`
 * which opens the pane stack.
 */
import { useEffect, useRef, lazy, Suspense, useState, useCallback, useMemo } from "react";
import cytoscape, { type Core } from "cytoscape";

import { useAppDispatch, useAppState } from "../../state/context";
import { useLanding } from "../../hooks/useLanding";
import { useArchitecture } from "../../hooks/useArchitecture";
import { useGraphAlgorithms } from "../../hooks/useGraphAlgorithms";
import type { GodNodeEntry } from "../../api/types";
import { toCytoscapeElements } from "../InteractiveGraph/adapter";
import { buildStylesheet, resolveNodeStyleClass } from "../InteractiveGraph/stylesheet";

const LandingSuggestionStrip = lazy(() =>
  import("./LandingSuggestionStrip").then((m) => ({ default: m.LandingSuggestionStrip })),
);

const RecentExplorationsStrip = lazy(() =>
  import("./RecentExplorationsStrip").then((m) => ({ default: m.RecentExplorationsStrip })),
);

const QualityOverview = lazy(() =>
  import("./QualityOverview").then((m) => ({ default: m.QualityOverview })),
);

const LandingHeader = lazy(() =>
  import("./LandingHeader").then((m) => ({ default: m.LandingHeader })),
);

export function GraphLanding({ workspaceId }: { workspaceId: string }) {
  const dispatch = useAppDispatch();
  const { perspective } = useAppState();
  const containerRef = useRef<HTMLDivElement | null>(null);
  const listContainerRef = useRef<HTMLDivElement | null>(null);
  const cyRef = useRef<Core | null>(null);
  const [listScrollTop, setListScrollTop] = useState(0);

  // WASM god_nodes integration (ADR-047 §Integration)
  // When VITE_ENABLE_WASM=true, we compute god_nodes in the browser via WASM.
  // The backend god_nodes from landingData remain as a fallback if WASM fails.
  const { godNodes: wasmGodNodes, enabled: wasmEnabled } = useGraphAlgorithms();
  const [wasmGodNodesResult, setWasmGodNodesResult] = useState<GodNodeEntry[] | null>(null);

  // Choose data source based on perspective
  const isGraph = perspective === "graph";
  const { data: landingData, isLoading: isLandingLoading, error: landingError } = useLanding(
    isGraph ? workspaceId : null,
  );
  const { data: archData, isLoading: isArchLoading, error: archError } = useArchitecture(
    !isGraph ? workspaceId : null,
  );

  const data = isGraph ? landingData : archData;
  const isLoading = isGraph ? isLandingLoading : isArchLoading;
  const error = isGraph ? landingError : archError;

  // Compute god_nodes via WASM when enabled (lazy — only runs once data is available)
  useEffect(() => {
    if (!wasmEnabled || !landingData || !isGraph) return;
    if (landingData.nodes.length === 0) return;

    const nodes = landingData.nodes.map((n) => ({ id: n.id, label: n.label }));
    const edges = landingData.edges.map((e) => ({ source: e.source, target: e.target }));

    wasmGodNodes(nodes, edges, { percentile: 0.95 })
      .then((result) => {
        // WASM god_nodes returns { id, score } — enrich with label from landing nodes
        const enriched: GodNodeEntry[] = result.nodes.map((wn) => ({
          id: wn.id,
          label: landingData.nodes.find((n) => n.id === wn.id)?.label ?? wn.id,
          score: wn.score,
        }));
        setWasmGodNodesResult(enriched);
      })
      .catch((err) => {
        console.warn("[GraphLanding] WASM god_nodes failed, using backend fallback:", err);
        setWasmGodNodesResult(null);
      });
  }, [wasmEnabled, landingData, isGraph, wasmGodNodes]);

  // Use WASM god_nodes if available, otherwise fall back to backend
  const godNodes = wasmGodNodesResult ?? landingData?.god_nodes ?? [];

  const selectObject = useCallback((objectId: string) => {
    dispatch({
      type: "SELECT_OBJECT",
      payload: { objectId, viewId: "overview" },
    });
  }, [dispatch]);

  // Mount cytoscape when data arrives
  useEffect(() => {
    if (!data || !containerRef.current) return;
    if (data.nodes.length === 0) return;

    let nodesWithStyle = data.nodes;
    // Only apply landing-specific styling when in graph perspective
    if (isGraph && landingData) {
      nodesWithStyle = data.nodes.map((n) => {
        // Check if this node is an entry point
        const isEntryPoint = landingData.entry_points.some((ep) => ep.id === n.id);
        // Check if this node is a hot path
        const isHot = landingData.hot_paths.some((hp) => hp.id === n.id);
        // Check if this node is a god node (WASM or backend)
        const isGod = godNodes.some((g) => g.id === n.id);

        const style_class = isEntryPoint
          ? "entry-point"
          : isHot
            ? "hot"
            : isGod
              ? "god"
              : resolveNodeStyleClass(n.style_class);

        return { ...n, style_class };
      });
    }

    const elements = toCytoscapeElements(nodesWithStyle, data.edges);

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
          selectObject(id);
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
  }, [data, isGraph, landingData, godNodes, selectObject]);

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

  // For C4 perspective, use a minimal header since we don't have workspace info
  const showC4Header = !isGraph;

  return (
    <div
      data-testid="graph-landing"
      data-perspective={perspective}
      style={{ height: "100%", width: "100%", display: "flex", flexDirection: "column" }}
    >
      {/* Header — only for graph perspective (C4 has no workspace data yet) */}
      {!showC4Header && (
        <Suspense fallback={<div style={{ height: 48 }} />}>
          <LandingHeader workspace={landingData!.workspace} />
        </Suspense>
      )}

      {Boolean((data as { truncated?: boolean }).truncated) && (
        <div
          data-testid="graph-landing-warning"
          className="px-4 py-2 text-xs"
          style={{
            backgroundColor: "rgba(210, 153, 34, 0.12)",
            color: "var(--color-warning)",
            borderBottom: "1px solid var(--color-border)",
          }}
        >
          Showing a truncated landing graph{(data as { truncated_reason?: string | null }).truncated_reason
            ? ` (${(data as { truncated_reason?: string | null }).truncated_reason})`
            : ""}
          . Refine the focus to inspect the full graph.
        </div>
      )}

      {/* Graph canvas */}
      <div
        ref={containerRef}
        data-testid="graph-landing-canvas"
        role="application"
        aria-label={`${perspective === "c4" ? "Architecture" : "Workspace"} landing graph`}
        tabIndex={0}
        style={{ flex: "1 1 auto", minHeight: 0 }}
      />

      {/* Accessible + testable fallback list for canvas-backed landing graphs */}
      {/* Virtualized when nodes > 200 to prevent DOM bloat (e9). */}
      {(() => {
        const THRESHOLD = 200;
        const ITEM_H = 28; // estimated px per button row
        const GAP = 8;
        const VISIBLE = 4; // visible rows
        const COLS = 8;
        const BUFFER = COLS * 2; // render 2 extra rows above/below
        const useVirt = data.nodes.length > THRESHOLD;

        if (!useVirt) {
          return (
            <div
              data-testid="graph-landing-node-list"
              className="flex flex-wrap gap-2 px-3 py-2"
              style={{
                backgroundColor: "var(--color-surface-raised)",
                borderTop: "1px solid var(--color-border)",
              }}
            >
              {data.nodes.map((node) => (
                <button
                  key={node.id}
                  type="button"
                  data-testid={`graph-node-${node.id}`}
                  data-kind={node.kind}
                  className={node.style_class ?? undefined}
                  onClick={() => selectObject(node.id)}
                  style={{
                    padding: "4px 8px",
                    borderRadius: 6,
                    border: "1px solid var(--color-border)",
                    backgroundColor: "var(--color-surface-overlay)",
                    color: "var(--color-text-secondary)",
                    fontSize: 11,
                    cursor: "pointer",
                  }}
                >
                  {node.label}
                </button>
              ))}
            </div>
          );
        }

        // Windowed rendering: render only a slice of nodes around the scroll position.
        // The outer container has maxHeight + overflow-y so the browser scrollbar
        // reflects totalHeight. We don't try to estimate padding precisely — we
        // just over-render (ITEM_H * 2 extra rows) and let the scrollbar be accurate
        // enough. This is a best-effort virtualization; DOM nodes are ~50 instead
        // of ~300+, which is the goal.
        const totalRows = Math.ceil(data.nodes.length / COLS);
        const totalH = totalRows * (ITEM_H + GAP);
        const startRow = Math.max(0, Math.floor(listScrollTop / (ITEM_H + GAP)) - 2);
        const endRow = Math.min(totalRows, startRow + VISIBLE + 4);
        const startIdx = startRow * COLS;
        const endIdx = Math.min(data.nodes.length, endRow * COLS);
        const topSpacer = startRow * (ITEM_H + GAP);
        const botSpacer = Math.max(0, totalH - topSpacer - (endIdx - startIdx) / COLS * (ITEM_H + GAP));

        return (
          <div
            data-testid="graph-landing-node-list"
            className="flex flex-wrap gap-2 px-3 py-2"
            style={{
              backgroundColor: "var(--color-surface-raised)",
              borderTop: "1px solid var(--color-border)",
              maxHeight: VISIBLE * (ITEM_H + GAP),
              overflowY: "auto",
            }}
            ref={listContainerRef}
            onScroll={(e) => setListScrollTop((e.currentTarget as HTMLDivElement).scrollTop)}
          >
            {topSpacer > 0 && <div style={{ height: topSpacer, width: "100%", flexBasis: "100%" }} />}
            {data.nodes.slice(startIdx, endIdx).map((node) => (
              <button
                key={node.id}
                type="button"
                data-testid={`graph-node-${node.id}`}
                data-kind={node.kind}
                className={node.style_class ?? undefined}
                onClick={() => selectObject(node.id)}
                style={{
                  padding: "4px 8px",
                  borderRadius: 6,
                  border: "1px solid var(--color-border)",
                  backgroundColor: "var(--color-surface-overlay)",
                  color: "var(--color-text-secondary)",
                  fontSize: 11,
                  cursor: "pointer",
                  flexShrink: 0,
                }}
              >
                {node.label}
              </button>
            ))}
            {botSpacer > 0 && <div style={{ height: botSpacer, width: "100%", flexBasis: "100%" }} />}
          </div>
        );
      })()}

      {/* Suggestion strip — only for graph perspective */}
      {!showC4Header && landingData && (
        <Suspense fallback={null}>
          <LandingSuggestionStrip
            suggestedQuestions={landingData.suggested_questions}
            onAsk={() => {
              // Dispatch ask action — the Ask panel will handle the question
              dispatch({ type: "SET_SPOTTER", payload: { open: true } });
            }}
          />
        </Suspense>
      )}

      {/* Recent explorations strip — only for graph perspective */}
      {!showC4Header && (
        <Suspense fallback={null}>
          <RecentExplorationsStrip
            workspaceId={workspaceId}
            onExplorationClick={(exploration) => {
              // Navigate to the first pane's object in the exploration
              const firstPane = exploration.panes[0];
              if (firstPane) {
                dispatch({
                  type: "SELECT_OBJECT",
                  payload: {
                    objectId: firstPane.object_id,
                    viewId: firstPane.view_id ?? "overview",
                    kind: "symbol",
                  },
                });
              }
            }}
          />
        </Suspense>
      )}

      {/* Workspace quality overview — only for graph perspective */}
      {!showC4Header && (
        <Suspense fallback={null}>
          <QualityOverview workspaceId={workspaceId} />
        </Suspense>
      )}
    </div>
  );
}
