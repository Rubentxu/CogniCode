/**
 * `GraphNode` — a single node (rounded rect + label).
 *
 * Visual treatment:
 * - Rounded rect, sized to fit the longest label line.
 * - `kind`-specific fill colour (so a glance tells you symbol vs
 *   file vs scope) — colours come from CSS variables.
 * - Click + Enter / Space dispatch `onSelect(id)` to the parent.
 *
 * Accessibility:
 * - `<g role="button" tabIndex={0}>` so keyboard users can pick
 *   a node.
 * - `aria-label` reads the label + kind — the visual text label
 *   inside the rect is also kept for sighted users.
 */
import { memo, useState } from "react";

import type { KeyboardEvent } from "react";

export interface GraphNodeProps {
  id: string;
  label: string;
  kind: string;
  x: number;
  y: number;
  /** Width of the rect — defaults to a label-sized box. */
  width?: number;
  /** Height of the rect. */
  height?: number;
  /** Click / Enter / Space triggers this. */
  onSelect?: (id: string) => void;
  /** Whether this node is the active / focused one. */
  selected?: boolean;
  /** Whether this node is hovered (visual only). */
  hovered?: boolean;
}

/**
 * Default node size. The container scales the SVG to the layout
 * viewBox, so 96×32 looks consistent at any zoom level.
 */
const NODE_WIDTH_DEFAULT = 96;
const NODE_HEIGHT_DEFAULT = 32;
const NODE_RADIUS = 6;

function colorForKind(kind: string, selected: boolean): string {
  if (selected) return "var(--color-graph-node-selected)";
  switch (kind) {
    case "symbol":
      return "var(--color-graph-node)";
    case "file":
      return "var(--color-info)";
    case "scope":
      return "var(--color-warning)";
    case "workspace":
      return "var(--color-text-primary)";
    case "module":
      return "var(--color-severity-medium)";
    case "evidence":
      return "var(--color-text-secondary)";
    case "decision_artifact":
      return "var(--color-severity-high)";
    case "quality_issue":
      return "var(--color-error)";
    case "rule":
      return "var(--color-text-muted)";
    default:
      return "var(--color-graph-node)";
  }
}

function GraphNodeImpl({
  id,
  label,
  kind,
  x,
  y,
  width = NODE_WIDTH_DEFAULT,
  height = NODE_HEIGHT_DEFAULT,
  onSelect,
  selected = false,
  hovered = false,
}: GraphNodeProps) {
  const [focused, setFocused] = useState(false);
  const fill = colorForKind(kind, selected);
  const stroke =
    selected || focused
      ? "var(--color-graph-node-focus)"
      : hovered
        ? "var(--color-primary)"
        : "var(--color-border)";

  function handleKey(event: KeyboardEvent<SVGGElement>) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onSelect?.(id);
    }
  }

  return (
    <g
      data-testid={`graph-node-${id}`}
      data-kind={kind}
      data-selected={selected ? "true" : "false"}
      role="button"
      tabIndex={0}
      aria-label={`${label} (${kind})`}
      onClick={() => onSelect?.(id)}
      onKeyDown={handleKey}
      onFocus={() => setFocused(true)}
      onBlur={() => setFocused(false)}
      style={{ cursor: onSelect ? "pointer" : "default" }}
    >
      <rect
        x={x - width / 2}
        y={y - height / 2}
        width={width}
        height={height}
        rx={NODE_RADIUS}
        ry={NODE_RADIUS}
        fill={fill}
        stroke={stroke}
        strokeWidth={selected || focused ? 2 : 1}
        opacity={hovered && !selected ? 0.92 : 1}
      />
      <text
        x={x}
        y={y}
        textAnchor="middle"
        dominantBaseline="central"
        fontSize={11}
        fontFamily="var(--font-mono)"
        style={{ fill: "var(--color-surface)" }}
        // Truncate with a clip-path if the label is too long. The
        // parent passes the actual label so screen readers get
        // the full string; we visually cap the text at width-12.
        clipPath={`inset(0 ${width / 2 - 12}px 0 ${width / 2 - 12}px round ${NODE_RADIUS}px)`}
      >
        {truncate(label, 14)}
      </text>
    </g>
  );
}

function truncate(label: string, max: number): string {
  if (label.length <= max) return label;
  return `${label.slice(0, max - 1)}…`;
}

export const GraphNode = memo(GraphNodeImpl);
