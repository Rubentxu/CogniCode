/**
 * `RationaleView` — interactive graph for rationale visualization.
 *
 * Wraps `InteractiveGraph` with a data-fetching layer that calls the
 * rationale endpoint (`GET /api/graph/:id/rationale`) via
 * `useRationaleGraph`. When corroboration scores are present in the
 * response, they are dynamically applied to the cytoscape instance
 * via the `onCyReady` callback.
 *
 * The component is a controlled drop-in for the 4th-column graph
 * area — it accepts the same `onSelectObject` / `selectedId` props
 * that `InteractiveGraph` does, plus `focusId` / `maxDepth` / `maxNodes`
 * for the rationale endpoint.
 */
import { useCallback } from "react";
import type { Core } from "cytoscape";

import { InteractiveGraph } from "../InteractiveGraph";
import { useRationaleGraph } from "../../hooks/useRationaleGraph";
import { applyCorroborationStyles } from "../InteractiveGraph/stylesheet";

export interface RationaleViewProps {
  /** Focus node id for the rationale query. */
  focusId: string;
  /** Max depth for the backend crawl (default 3). */
  maxDepth?: number;
  /** Max nodes for the backend crawl (default 50). */
  maxNodes?: number;
  /** Called when the user clicks a node. */
  onSelectObject: (id: string) => void;
  /** Optional currently-selected node id. */
  selectedId?: string | null;
  /** Optional CSS class passthrough. */
  className?: string;
}

export function RationaleView({
  focusId,
  maxDepth = 3,
  maxNodes = 50,
  onSelectObject,
  selectedId,
  className,
}: RationaleViewProps) {
  const { data, error, isLoading } = useRationaleGraph(focusId, {
    maxDepth,
    maxNodes,
  });

  // Apply corroboration styles whenever a new cytoscape instance is
  // mounted (i.e. on root or data change). We memoise the callback
  // so it does not cause unnecessary re-renders.
  const handleCyReady = useCallback(
    (cy: Core) => {
      if (!data?.corroboration_scores) return;
      const scores = data.corroboration_scores;
      if (Object.keys(scores).length === 0) return;
      applyCorroborationStyles(cy, scores);
    },
    [data],
  );

  if (isLoading) {
    return (
      <div
        data-testid="rationale-loading"
        className={className}
        style={{
          height: "100%",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--color-text-muted)",
          fontSize: 12,
        }}
      >
        Loading rationale…
      </div>
    );
  }

  if (error) {
    return (
      <div
        role="alert"
        data-testid="rationale-error"
        className={className}
        style={{
          height: "100%",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--color-error)",
          fontSize: 12,
          gap: 4,
        }}
      >
        <span className="font-semibold">Error loading rationale</span>
        <span style={{ color: "var(--color-text-muted)" }}>{error.message}</span>
      </div>
    );
  }

  if (!data) return null;

  const scoredCount = data.corroboration_scores
    ? Object.keys(data.corroboration_scores).length
    : 0;

  return (
    <div className={className} data-testid="rationale-view">
      <div
        data-testid="rationale-header"
        className="text-sm px-2 py-1"
        style={{
          color: "var(--color-text-muted)",
          borderBottom: "1px solid var(--color-border)",
        }}
      >
        Rationale for {focusId} · {data.nodes.length} nodes · {data.edges.length} edges
        {scoredCount > 0 && ` · ${scoredCount} scored edges`}
      </div>
      <div style={{ flex: "1 1 auto", minHeight: 0 }} data-testid="rationale-graph">
        <InteractiveGraph
          root={focusId}
          data={data}
          selectedId={selectedId}
          onSelectObject={onSelectObject}
          onCyReady={handleCyReady}
        />
      </div>
    </div>
  );
}
