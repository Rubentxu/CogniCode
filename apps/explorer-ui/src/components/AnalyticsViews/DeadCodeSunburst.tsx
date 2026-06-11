/**
 * `DeadCodeSunburst` — D3 partition/arc for code-coverage analysis.
 *
 * Each ring segment represents a module or symbol. `size` controls
 * the angular extent; `alive === false` paints the segment in a
 * warm (red/orange) palette so dead code is immediately obvious.
 *
 * The component animates between data sets via d3-transition:
 * when the parent passes a new `data` reference we run a 450ms
 * path-`d` interpolation. The transition relies on `d3-transition`
 * augmenting the selection prototype at import time.
 *
 * The component is purely presentational.
 */
import { useEffect, useMemo, useRef } from "react";

import { hierarchy, partition, type HierarchyRectangularNode } from "d3-hierarchy";
import { arc as d3Arc } from "d3-shape";
import { scaleOrdinal } from "d3-scale";
import { select } from "d3-selection";
import "d3-transition";

import type { SunburstData, SunburstLeaf } from "./types";

export interface DeadCodeSunburstProps {
  data: SunburstData;
  /**
   * Outer diameter in pixels. Defaults to 360.
   */
  size?: number;
  /**
   * Inner radius as a fraction of the outer radius. Defaults to
   * 0.32 which gives a comfortable donut look.
   */
  innerRadiusRatio?: number;
  /**
   * Optional className passthrough.
   */
  className?: string;
}

interface ArcRange {
  startAngle: number;
  endAngle: number;
  innerRadius: number;
  outerRadius: number;
}

interface RenderedSegment {
  key: string;
  name: string;
  alive: boolean;
  d: string;
  range: ArcRange;
  midAngle: number;
  midRadius: number;
  fill: string;
  stroke: string;
  tooltip: string;
  showLabel: boolean;
  labelX: number;
  labelY: number;
  labelRotation: number;
}

const DEAD_PALETTE = ["#f97316", "#ef4444", "#dc2626", "#b91c1c"];
const ALIVE_PALETTE = ["#64748b", "#94a3b8", "#cbd5e1", "#475569"];

const deadScale = scaleOrdinal<string, string>().range(DEAD_PALETTE);
const aliveScale = scaleOrdinal<string, string>().range(ALIVE_PALETTE);

const TRANSITION_DURATION_MS = 450;

/**
 * Build an SVG arc generator that consumes `ArcRange` shapes.
 * Captured per-render so the d3-transition call site stays short.
 */
function makeArcGen() {
  return d3Arc<ArcRange>()
    .startAngle((d) => d.startAngle)
    .endAngle((d) => d.endAngle)
    .innerRadius((d) => d.innerRadius)
    .outerRadius((d) => d.outerRadius)
    .padAngle(0.005);
}

/**
 * Layout the sunburst. We use d3-hierarchy + d3-partition for the
 * ring geometry and d3-arc to generate the SVG path strings.
 */
function layoutSunburst(
  data: SunburstData,
  size: number,
  innerRatio: number,
): RenderedSegment[] {
  if (data.children.length === 0 || size <= 0) return [];
  const radius = size / 2;
  const arcGen = makeArcGen();

  // The d3-hierarchy type system can't infer that leaf nodes have
  // `size` and `alive` while the root only has `name` and
  // `children` — we declare a union type that covers both shapes
  // and cast at the call site.
  type AnyNode = SunburstData | SunburstLeaf;
  const root = hierarchy<AnyNode>({ name: data.name, children: data.children })
    .sum((d) => (((d as AnyNode).children ? 0 : (d as SunburstLeaf).size) ?? 0))
    .sort((a, b) => (b.value ?? 0) - (a.value ?? 0));

  const part = partition<AnyNode>().size([2 * Math.PI, radius]);
  part(root);

  return (root.descendants() as HierarchyRectangularNode<AnyNode>[])
    .filter((d) => d.depth > 0 && !d.children)
    .map((d) => {
      const leaf = d.data as SunburstLeaf;
      const alive = Boolean(leaf.alive);
      const fill = (alive ? aliveScale : deadScale)(leaf.name);
      const stroke = alive ? "var(--color-surface)" : "#7f1d1d";
      const midAngle = (d.x0 + d.x1) / 2;
      const midRadius = (d.y0 + d.y1) / 2 + radius * innerRatio;
      const span = d.x1 - d.x0;
      const showLabel = span > 0.18 && midRadius > 0;
      const labelX = Math.sin(midAngle) * midRadius;
      const labelY = -Math.cos(midAngle) * midRadius;
      // Rotate text along the arc, but flip it so it stays readable
      // on the left half of the chart.
      let labelRotation = (midAngle * 180) / Math.PI - 90;
      if (labelRotation > 90) labelRotation -= 180;
      if (labelRotation < -90) labelRotation += 180;
      const range: ArcRange = {
        startAngle: d.x0,
        endAngle: d.x1,
        innerRadius: d.y0 + radius * innerRatio,
        outerRadius: d.y1 + radius * innerRatio - 1,
      };
      return {
        key: leaf.name,
        name: leaf.name,
        alive,
        d: arcGen(range) ?? "",
        range,
        midAngle,
        midRadius,
        fill,
        stroke,
        tooltip: `${leaf.name} — ${alive ? "alive" : "dead"}`,
        showLabel,
        labelX,
        labelY,
        labelRotation,
      };
    });
}

/**
 * Interpolate between two arc ranges. Used by d3-transition's
 * `attrTween` to morph the SVG path between the previous and
 * next data sets.
 */
function interpolateRange(prev: ArcRange, next: ArcRange, t: number): ArcRange {
  return {
    startAngle: prev.startAngle + (next.startAngle - prev.startAngle) * t,
    endAngle: prev.endAngle + (next.endAngle - prev.endAngle) * t,
    innerRadius: prev.innerRadius + (next.innerRadius - prev.innerRadius) * t,
    outerRadius: prev.outerRadius + (next.outerRadius - prev.outerRadius) * t,
  };
}

/**
 * Animate path d-attribute transitions when the data identity
 * changes. We hold the previous layout in a ref and interpolate
 * between matching segments. New segments fade in via opacity.
 *
 * The hook returns the new `segments` as-is on every render. The
 * `useEffect` only drives the d3-transition on the DOM — it never
 * mutates React state, so we avoid the cascading-render warning.
 */
function useAnimatedSegments(
  segments: RenderedSegment[],
  enabled: boolean,
): RenderedSegment[] {
  const previousRef = useRef<Map<string, ArcRange> | null>(null);
  const dataIdentityRef = useRef<string | null>(null);

  useEffect(() => {
    if (!enabled) {
      previousRef.current = null;
      dataIdentityRef.current = null;
      return;
    }

    const dataIdentity = segments.map((s) => s.key).join("|");
    const isFirstRender = previousRef.current === null;
    const isDataChange = dataIdentityRef.current !== dataIdentity;

    // First render — snapshot layout; React already paints.
    if (isFirstRender) {
      previousRef.current = new Map(segments.map((s) => [s.key, s.range]));
      dataIdentityRef.current = dataIdentity;
      return;
    }

    if (!isDataChange) {
      // Same data, same identity — refresh layout snapshot but
      // skip the d3 transition.
      previousRef.current = new Map(segments.map((s) => [s.key, s.range]));
      return;
    }

    const previous = previousRef.current ?? new Map<string, ArcRange>();
    const svg = select("[data-testid='dead-code-sunburst-svg']");
    const arcGen = makeArcGen();

    if (!svg.empty()) {
      svg
        .selectAll<SVGPathElement, RenderedSegment>("path")
        .data(segments, (d) => (d as RenderedSegment).key)
        .transition()
        .duration(TRANSITION_DURATION_MS)
        .attrTween("d", function (next) {
          const prevRange = previous.get(next.key);
          if (!prevRange) {
            return () => next.d;
          }
          const interpolator = (t: number) =>
            arcGen(interpolateRange(prevRange, next.range, t)) ?? next.d;
          return interpolator as (t: number) => string;
        });
    }

    previousRef.current = new Map(segments.map((s) => [s.key, s.range]));
    dataIdentityRef.current = dataIdentity;
  }, [segments, enabled]);

  return segments;
}

export function DeadCodeSunburst({
  data,
  size = 360,
  innerRadiusRatio = 0.32,
  className,
}: DeadCodeSunburstProps) {
  const safeSize = Math.max(160, size);
  const segments = useMemo(
    () => layoutSunburst(data, safeSize, innerRadiusRatio),
    [data, safeSize, innerRadiusRatio],
  );
  const display = useAnimatedSegments(segments, true);

  // Empty state — same testid pattern as the treemap so callers
  // can assert on the empty branch uniformly.
  if (data.children.length === 0) {
    return (
      <div
        data-testid="dead-code-sunburst-empty"
        className={
          "flex w-full items-center justify-center rounded-md border text-xs " +
          (className ?? "")
        }
        style={{
          height: safeSize,
          color: "var(--color-text-muted)",
          borderColor: "var(--color-border)",
          backgroundColor: "var(--color-surface-overlay)",
        }}
      >
        No dead code detected.
      </div>
    );
  }

  const radius = safeSize / 2;
  return (
    <div
      data-testid="dead-code-sunburst"
      data-segment-count={segments.length}
      className={"relative w-full " + (className ?? "")}
    >
      <header
        className="mb-1 flex items-baseline justify-between"
        style={{ color: "var(--color-text-secondary)" }}
      >
        <h3
          className="text-sm font-semibold"
          style={{ color: "var(--color-text-primary)" }}
        >
          {data.name}
        </h3>
        <span
          className="font-mono text-xs"
          data-testid="dead-code-sunburst-counts"
          style={{ color: "var(--color-text-muted)" }}
        >
          {segments.filter((s) => !s.alive).length} dead / {segments.filter((s) => s.alive).length} alive
        </span>
      </header>
      <svg
        width={safeSize}
        height={safeSize}
        viewBox={`-${radius} -${radius} ${safeSize} ${safeSize}`}
        role="img"
        aria-label={`Dead code sunburst: ${data.name}`}
        data-testid="dead-code-sunburst-svg"
        data-alive-count={segments.filter((s) => s.alive).length}
        data-dead-count={segments.filter((s) => !s.alive).length}
      >
        {display.map((seg) => (
          <g
            key={seg.key}
            data-testid={`dead-code-sunburst-segment-${seg.name}`}
            data-alive={seg.alive ? "true" : "false"}
          >
            <path
              d={seg.d}
              fill={seg.fill}
              stroke={seg.stroke}
              strokeWidth={1}
            >
              <title>{seg.tooltip}</title>
            </path>
            {seg.showLabel && (
              <text
                x={seg.labelX}
                y={seg.labelY}
                textAnchor="middle"
                dominantBaseline="middle"
                fontSize={9}
                fill={seg.alive ? "#0f172a" : "#fef2f2"}
                transform={`rotate(${seg.labelRotation} ${seg.labelX} ${seg.labelY})`}
                pointerEvents="none"
              >
                {seg.name}
              </text>
            )}
          </g>
        ))}
      </svg>
    </div>
  );
}
