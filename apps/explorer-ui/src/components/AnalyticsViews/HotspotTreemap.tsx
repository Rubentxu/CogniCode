/**
 * `HotspotTreemap` — D3 treemap for code-complexity hotspots.
 *
 * Each cell represents a symbol. Cell area is driven by `value`
 * (typically call count or fan-in) and cell colour by `complexity`
 * (a 0..1 score). The colour scale goes green → yellow → red so
 * hot cells visually pop.
 *
 * The component is responsive: it measures its container on mount
 * and re-measures on window resize. SVG `viewBox` lets the cells
 * scale smoothly when the panel is resized.
 *
 * The component is purely presentational — it takes `data` and
 * emits nothing. The parent owns the data fetch (see LensPanel).
 */
import { useEffect, useMemo, useRef, useState, type RefObject } from "react";

import { hierarchy, treemap, type HierarchyRectangularNode } from "d3-hierarchy";
import { scaleLinear, scaleSequential } from "d3-scale";
import { interpolateRgb } from "d3-interpolate";
import { max as d3Max } from "d3-array";

import type { TreemapData, TreemapLeaf } from "./types";

export interface HotspotTreemapProps {
  data: TreemapData;
  /**
   * Optional container height. Defaults to 360. The width is
   * measured from the parent — the treemap fills it.
   */
  height?: number;
  /**
   * Optional className passthrough.
   */
  className?: string;
}

interface RenderedCell {
  key: string;
  name: string;
  x0: number;
  y0: number;
  x1: number;
  y1: number;
  complexity: number;
  fill: string;
  textX: number;
  textY: number;
  showLabel: boolean;
  showComplexity: boolean;
  tooltip: string;
}

/**
 * Green → yellow → red colour ramp. `interpolateRgb` is part of
 * d3-interpolate; we use it to build a three-stop scale so the
 * gradient is readable rather than muted.
 */
const colourRamp = scaleSequential(
  interpolateRgb("#10b981", "#ef4444"),
).domain([0, 1]);

/**
 * Pick text colour based on the fill brightness. Cells near the
 * red end of the spectrum get white text, cells near the green
 * end get dark text. Keeps the symbol name legible in all cases.
 */
const textOnFill = scaleLinear<string>()
  .domain([0, 0.55, 0.85, 1])
  .range(["#0f172a", "#0f172a", "#ffffff", "#ffffff"])
  .clamp(true);

/**
 * Render the treemap. We compute the layout with d3-hierarchy,
 * map each cell to a drawable shape, and emit them as `<rect>`s
 * inside a single `<svg>`.
 */
function layoutTreemap(
  data: TreemapData,
  width: number,
  height: number,
): RenderedCell[] {
  if (data.children.length === 0 || width <= 0 || height <= 0) return [];

  // The d3-hierarchy type system can't infer that leaf nodes have
  // `value` and `complexity` while the root only has `name` and
  // `children` — we declare a union type that covers both shapes
  // and cast at the call site.
  type AnyNode = TreemapData | TreemapLeaf;
  const root = hierarchy<AnyNode>({ name: data.name, children: data.children })
    .sum((d) => (((d as AnyNode).children ? 0 : (d as TreemapLeaf).value) ?? 0))
    .sort((a, b) => (b.value ?? 0) - (a.value ?? 0));

  const tree = treemap<AnyNode>().size([width, height]).paddingInner(2).round(true);
  tree(root);

  return (root.descendants() as HierarchyRectangularNode<AnyNode>[])
    .filter((d) => d.depth > 0 && !d.children)
    .map((d) => {
      const leaf = d.data as TreemapLeaf;
      const complexity = Math.max(0, Math.min(1, Number(leaf.complexity ?? 0)));
      const fill = colourRamp(complexity);
      const cellW = d.x1 - d.x0;
      const cellH = d.y1 - d.y0;
      const showLabel = cellW > 56 && cellH > 24;
      const showComplexity = cellW > 64 && cellH > 36;
      return {
        key: leaf.name,
        name: leaf.name,
        x0: d.x0,
        y0: d.y0,
        x1: d.x1,
        y1: d.y1,
        complexity,
        fill,
        textX: d.x0 + cellW / 2,
        textY: d.y0 + cellH / 2,
        showLabel,
        showComplexity,
        tooltip: `${leaf.name} — complexity ${complexity.toFixed(2)}`,
      };
    });
}

/**
 * Container-width measurement. We track the parent width so the
 * treemap fills whatever space the parent gives it. Falls back to
 * a sensible default during SSR/test bootstrap before mount.
 *
 * Returns a tuple `[width, ref]` — same shape as `useState` pairs
 * — so consumers can `const [width, ref] = useContainerWidth()`.
 */
function useContainerWidth(
  defaultWidth = 480,
): readonly [number, RefObject<HTMLDivElement | null>] {
  const ref = useRef<HTMLDivElement | null>(null);
  const [width, setWidth] = useState<number>(defaultWidth);

  useEffect(() => {
    const node = ref.current;
    if (!node) return;
    const measure = () => {
      const rect = node.getBoundingClientRect();
      if (rect.width > 0) setWidth(rect.width);
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  return [width, ref] as const;
}

export function HotspotTreemap({
  data,
  height = 360,
  className,
}: HotspotTreemapProps) {
  const [width, containerRef] = useContainerWidth();
  const cells = useMemo(() => layoutTreemap(data, width, height), [data, width, height]);
  const maxComplexity = useMemo(
    () => d3Max(data.children, (c) => c.complexity) ?? 1,
    [data.children],
  );

  // Empty state — no cells means no hotspots to draw. Keep the
  // same testid so callers can assert on the empty branch.
  if (data.children.length === 0) {
    return (
      <div
        ref={containerRef}
        data-testid="hotspot-treemap-empty"
        className={
          "flex w-full items-center justify-center rounded-md border text-xs " +
          (className ?? "")
        }
        style={{
          height,
          color: "var(--color-text-muted)",
          borderColor: "var(--color-border)",
          backgroundColor: "var(--color-surface-overlay)",
        }}
      >
        No hotspots detected.
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      data-testid="hotspot-treemap"
      data-cell-count={cells.length}
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
          data-testid="hotspot-treemap-max"
          style={{ color: "var(--color-text-muted)" }}
        >
          max {maxComplexity.toFixed(2)}
        </span>
      </header>
      <svg
        width={width}
        height={height}
        viewBox={`0 0 ${width} ${height}`}
        role="img"
        aria-label={`Hotspot treemap: ${data.name}`}
        data-testid="hotspot-treemap-svg"
      >
        {cells.map((cell) => (
          <g
            key={cell.key}
            data-testid={`hotspot-treemap-cell-${cell.name}`}
            data-name={cell.name}
            data-complexity={cell.complexity}
          >
            <rect
              x={cell.x0}
              y={cell.y0}
              width={cell.x1 - cell.x0}
              height={cell.y1 - cell.y0}
              fill={cell.fill}
              stroke="var(--color-surface)"
              strokeWidth={1}
              rx={2}
            >
              <title>{cell.tooltip}</title>
            </rect>
            {cell.showLabel && (
              <text
                x={cell.textX}
                y={cell.textY}
                textAnchor="middle"
                dominantBaseline="middle"
                fontSize={11}
                fontWeight={600}
                fill={textOnFill(cell.complexity)}
                pointerEvents="none"
              >
                {cell.name}
              </text>
            )}
            {cell.showComplexity && (
              <text
                x={cell.textX}
                y={cell.textY + 14}
                textAnchor="middle"
                dominantBaseline="middle"
                fontSize={10}
                fill={textOnFill(cell.complexity)}
                pointerEvents="none"
                opacity={0.85}
              >
                {cell.complexity.toFixed(2)}
              </text>
            )}
          </g>
        ))}
      </svg>
    </div>
  );
}
