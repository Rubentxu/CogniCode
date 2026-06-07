/**
 * `GraphEdge` — a single edge between two nodes.
 *
 * Pure presentational — receives the source and target positions
 * (NOT the node ids) so the layout engine can change without
 * touching this file. Stroke colour comes from a CSS variable so
 * the graph re-themes with the rest of the app.
 */
import { memo } from "react";

export interface GraphEdgeProps {
  /** Source position. */
  from: { x: number; y: number };
  /** Target position. */
  to: { x: number; y: number };
  /** Highlighted (e.g., when an endpoint is focused). */
  highlighted?: boolean;
  /** Optional label (drawn near the midpoint). */
  label?: string;
  /** Stable id for test selectors. */
  testId?: string;
}

/**
 * Render an edge as a single `<line>` between two points. The
 * `marker-end` arrow is omitted (we use a thin line + label to
 * keep the SVG accessible to screen readers; the parent renders
 * a textual fallback table).
 *
 * Memoised on (from, to, highlighted) — re-renders only when the
 * endpoint positions change.
 */
function GraphEdgeImpl({ from, to, highlighted = false, label, testId }: GraphEdgeProps) {
  return (
    <g
      data-testid={testId ?? "graph-edge"}
      data-highlighted={highlighted ? "true" : "false"}
      role="presentation"
    >
      <line
        x1={from.x}
        y1={from.y}
        x2={to.x}
        y2={to.y}
        stroke={
          highlighted
            ? "var(--color-graph-edge-highlight)"
            : "var(--color-graph-edge)"
        }
        strokeWidth={highlighted ? 1.75 : 1}
        strokeLinecap="round"
      />
      {label !== undefined && label.length > 0 && (
        <text
          x={(from.x + to.x) / 2}
          y={(from.y + to.y) / 2}
          textAnchor="middle"
          dominantBaseline="middle"
          fontSize={9}
          style={{
            fill: "var(--color-text-muted)",
            paintOrder: "stroke",
            stroke: "var(--color-surface)",
            strokeWidth: 3,
            strokeLinejoin: "round",
          }}
        >
          {label}
        </text>
      )}
    </g>
  );
}

export const GraphEdge = memo(GraphEdgeImpl);
