/**
 * `SvgGraph` — interactive SVG graph of nodes + edges.
 *
 * The component is layout-agnostic: it receives a `LayoutResult`
 * (see `mocks/layoutMock.ts`) with positions already computed.
 * When the backend `POST /api/diagrams/layout` endpoint lands,
 * swap the mock for an SWR hook that calls it.
 *
 * Interactions:
 * - Mouse drag pans the view (transform on a wrapper `<g>`).
 * - Mouse wheel zooms (anchored at the cursor).
 * - Click on a node dispatches `onSelectObject(id)` to the parent.
 * - Hovered / focused node gets a thicker stroke (visual only).
 *
 * Accessibility:
 * - The SVG container has `role="complementary"` and an accessible
 *   name describing the graph's contents.
 * - An off-screen `<table>` summarises nodes and edges for screen
 *   readers — the SVG is purely visual.
 * - Keyboard users can Tab to a node (role=button) and press
 *   Enter / Space to select it.
 */
import { useCallback, useMemo, useRef, useState } from "react";
import type { MouseEvent as ReactMouseEvent, WheelEvent } from "react";

import type { LayoutResult } from "../../mocks/layoutMock";
import type { ViewportState } from "../../state/navigation/types";
import { GraphNode } from "./GraphNode";
import { GraphEdge } from "./GraphEdge";

export interface SvgGraphProps {
  layout: LayoutResult;
  /** Optional id of the currently selected object. */
  selectedId?: string | null;
  /** Dispatched when the user picks a node. */
  onSelectObject?: (id: string) => void;
  /** Called when the viewport changes after a pan or zoom gesture ends. */
  onViewportChange?: (viewport: ViewportState) => void;
  /** Accessible label for the graph region. */
  ariaLabel?: string;
  /** Optional className passthrough. */
  className?: string;
}

interface Viewport {
  x: number;
  y: number;
  scale: number;
}

const MIN_SCALE = 0.25;
const MAX_SCALE = 4;
const ZOOM_FACTOR = 1.1;

/**
 * Apply a mouse-wheel delta to the current viewport, zooming
 * around the cursor's SVG-space position. The math is the
 * standard "zoom-to-cursor" formula:
 *
 *   world.x = (cursor.x - view.x) / view.scale
 *   view.x  = cursor.x - world.x * newScale
 */
function zoomAt(
  view: Viewport,
  cursor: { x: number; y: number },
  factor: number,
): Viewport {
  const newScale = clamp(view.scale * factor, MIN_SCALE, MAX_SCALE);
  if (newScale === view.scale) return view;
  const worldX = (cursor.x - view.x) / view.scale;
  const worldY = (cursor.y - view.y) / view.scale;
  return {
    scale: newScale,
    x: cursor.x - worldX * newScale,
    y: cursor.y - worldY * newScale,
  };
}

function clamp(v: number, lo: number, hi: number): number {
  return Math.max(lo, Math.min(hi, v));
}

/**
 * Convert a mouse event's client coordinates into the SVG's
 * viewBox space. Needed to anchor zoom to the cursor regardless
 * of the rendered SVG's size on screen.
 */
function clientToSvg(
  svg: SVGSVGElement,
  clientX: number,
  clientY: number,
  viewBox: { x: number; y: number; width: number; height: number },
): { x: number; y: number } {
  const rect = svg.getBoundingClientRect();
  const xRatio = (clientX - rect.left) / rect.width;
  const yRatio = (clientY - rect.top) / rect.height;
  return {
    x: viewBox.x + xRatio * viewBox.width,
    y: viewBox.y + yRatio * viewBox.height,
  };
}

export function SvgGraph({
  layout,
  selectedId = null,
  onSelectObject,
  onViewportChange,
  ariaLabel,
  className,
}: SvgGraphProps) {
  // The viewport (pan/zoom) is owned by an inner component that
  // gets remounted via `key` whenever the layout identity changes.
  // That makes the "reset zoom on new graph" a render-time
  // concern — no setState in an effect, no linter complaints.
  const layoutKey = `${layout.nodes.length}-${layout.viewBox.width}-${layout.viewBox.height}`;

  return (
    <SvgGraphInner
      key={layoutKey}
      layout={layout}
      selectedId={selectedId}
      onSelectObject={onSelectObject}
      onViewportChange={onViewportChange}
      ariaLabel={ariaLabel}
      className={className}
    />
  );
}

// ============================================================================
// Inner — owns the viewport (pan / zoom / hover) state
// ============================================================================

type SvgGraphInnerProps = SvgGraphProps;

function SvgGraphInner({
  layout,
  selectedId = null,
  onSelectObject,
  onViewportChange,
  ariaLabel,
  className,
}: SvgGraphInnerProps) {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const [view, setView] = useState<Viewport>({ x: 0, y: 0, scale: 1 });
  const [hoveredId, setHoveredId] = useState<string | null>(null);

  // Callback to notify parent of viewport changes (for snapshot persistence)
  const handleViewChange = useCallback(
    (newView: Viewport) => {
      onViewportChange?.({ x: newView.x, y: newView.y, scale: newView.scale });
    },
    [onViewportChange],
  );
  // Pan state. We use refs for the high-frequency values (start
  // positions) and a state flag for the visual cursor change so
  // we don't have to read the ref during render.
  const [isDragging, setIsDragging] = useState(false);
  const dragRef = useRef<{
    startClient: { x: number; y: number };
    startView: Viewport;
  } | null>(null);

  // Index nodes by id for O(1) lookup from edges.
  const nodeById = useMemo(() => {
    const map = new Map(layout.nodes.map((n) => [n.id, n]));
    return map;
  }, [layout]);

  // Edges that survive the projection — both endpoints exist.
  const visibleEdges = useMemo(
    () =>
      layout.edges.filter(
        (e) => nodeById.has(e.from) && nodeById.has(e.to),
      ),
    [layout.edges, nodeById],
  );

  // -----------------------------------------------------------------
  // Pan + zoom handlers
  // -----------------------------------------------------------------
  const onPointerDown = useCallback(
    (event: ReactMouseEvent<SVGSVGElement>) => {
      // Only left-button pans; let clicks on nodes bubble.
      if (event.button !== 0) return;
      if (event.target instanceof Element && event.target.closest("[data-testid^='graph-node-']")) {
        return;
      }
      dragRef.current = {
        startClient: { x: event.clientX, y: event.clientY },
        startView: view,
      };
      setIsDragging(true);
      const pid = (event.nativeEvent as PointerEvent).pointerId;
      if (pid !== undefined) {
        event.currentTarget.setPointerCapture?.(pid);
      }
    },
    [view],
  );

  const onPointerMove = useCallback(
    (event: ReactMouseEvent<SVGSVGElement>) => {
      const drag = dragRef.current;
      if (!drag) return;
      const dx = event.clientX - drag.startClient.x;
      const dy = event.clientY - drag.startClient.y;
      // Convert pixel delta into SVG-space delta using the
      // current rendered ratio.
      const svg = svgRef.current;
      if (!svg) return;
      const rect = svg.getBoundingClientRect();
      const xRatio = layout.viewBox.width / rect.width;
      const yRatio = layout.viewBox.height / rect.height;
      setView({
        ...drag.startView,
        x: drag.startView.x - dx * xRatio,
        y: drag.startView.y - dy * yRatio,
      });
    },
    [layout.viewBox.width, layout.viewBox.height],
  );

  const onPointerUp = useCallback(
    (event: ReactMouseEvent<SVGSVGElement>) => {
      if (dragRef.current) {
        const pid = (event.nativeEvent as PointerEvent).pointerId;
        if (pid !== undefined) {
          event.currentTarget.releasePointerCapture?.(pid);
        }
        dragRef.current = null;
        setIsDragging(false);
        handleViewChange(view);
      }
    },
    [handleViewChange, view],
  );

  const onWheel = useCallback(
    (event: WheelEvent<SVGSVGElement>) => {
      event.preventDefault();
      const svg = svgRef.current;
      if (!svg) return;
      const cursor = clientToSvg(svg, event.clientX, event.clientY, layout.viewBox);
      // Translate wheel deltaY to a zoom factor. WheelEvent.deltaY
      // is positive when scrolling down — convention is to zoom out
      // (or in, depending on trackpad direction). We invert so a
      // scroll-up zooms in, which is the standard "pinch-zoom"
      // feeling.
      const factor = event.deltaY < 0 ? ZOOM_FACTOR : 1 / ZOOM_FACTOR;
      // Apply zoom anchored to the current cursor position in the
      // CURRENT viewport (transformed through the current view).
      const transformed = {
        x: view.x + cursor.x * view.scale,
        y: view.y + cursor.y * view.scale,
      };
      setView((prev) => {
        const next = zoomAt(prev, transformed, factor);
        handleViewChange(next);
        return next;
      });
    },
    [layout.viewBox, view.x, view.y, view.scale, handleViewChange],
  );

  // -----------------------------------------------------------------
  // Build the transform string
  // -----------------------------------------------------------------
  // The `<g>` transform is the only thing that changes on
  // pan/zoom — the viewBox stays fixed so screen readers see
  // a stable coordinate system.
  const transform = `translate(${view.x} ${view.y}) scale(${view.scale})`;

  return (
    <div
      role="complementary"
      aria-label={ariaLabel ?? `Graph with ${layout.nodes.length} nodes`}
      data-testid="svg-graph"
      data-node-count={layout.nodes.length}
      data-edge-count={visibleEdges.length}
      className={
        "relative h-full w-full overflow-hidden " + (className ?? "")
      }
      style={{
        backgroundColor: "var(--color-surface)",
        cursor: isDragging ? "grabbing" : "grab",
      }}
    >
      <svg
        ref={svgRef}
        viewBox={`${layout.viewBox.x} ${layout.viewBox.y} ${layout.viewBox.width} ${layout.viewBox.height}`}
        preserveAspectRatio="xMidYMid meet"
        width="100%"
        height="100%"
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerLeave={onPointerUp}
        onWheel={onWheel}
        data-testid="svg-graph-canvas"
        style={{ display: "block", userSelect: "none" }}
      >
        <g transform={transform}>
          {/* Edges first so they sit underneath the node rects. */}
          {visibleEdges.map((edge, idx) => {
            const from = nodeById.get(edge.from);
            const to = nodeById.get(edge.to);
            if (!from || !to) return null;
            const highlighted =
              selectedId === edge.from ||
              selectedId === edge.to ||
              hoveredId === edge.from ||
              hoveredId === edge.to;
            // Per T12 (Grill Decision 6): labels only on highlighted edges
            const label = highlighted ? edge.label : undefined;
            return (
              <GraphEdge
                key={`e-${edge.from}-${edge.to}-${idx}`}
                from={from}
                to={to}
                highlighted={highlighted}
                {...(label !== undefined ? { label } : {})}
                testId={`graph-edge-${edge.from}-${edge.to}`}
              />
            );
          })}
          {layout.nodes.map((node) => (
            <g
              key={node.id}
              onMouseEnter={() => setHoveredId(node.id)}
              onMouseLeave={() =>
                setHoveredId((cur) => (cur === node.id ? null : cur))
              }
            >
              <GraphNode
                id={node.id}
                label={node.label}
                kind={node.kind}
                x={node.x}
                y={node.y}
                selected={node.id === selectedId}
                hovered={node.id === hoveredId && node.id !== selectedId}
                onSelect={onSelectObject}
              />
            </g>
          ))}
        </g>
      </svg>

      {/* Screen-reader fallback — a tabular list of the graph. */}
      <table
        data-testid="svg-graph-fallback"
        className="sr-only"
        aria-label="Graph contents"
      >
        <caption>{`Graph with ${layout.nodes.length} nodes and ${visibleEdges.length} edges`}</caption>
        <thead>
          <tr>
            <th scope="col">Node</th>
            <th scope="col">Kind</th>
            <th scope="col">Position</th>
          </tr>
        </thead>
        <tbody>
          {layout.nodes.map((n) => (
            <tr key={n.id}>
              <th scope="row">{n.label}</th>
              <td>{n.kind}</td>
              <td>{`x=${n.x}, y=${n.y}`}</td>
            </tr>
          ))}
        </tbody>
      </table>

      {/* Zoom controls — visible-only (aria-hidden). */}
      <div
        aria-hidden="true"
        data-testid="svg-graph-controls"
        className="absolute right-2 top-2 flex flex-col gap-1 rounded-md p-1"
        style={{
          backgroundColor: "var(--color-surface-overlay)",
          border: "1px solid var(--color-border)",
        }}
      >
        <button
          type="button"
          onClick={() => setView((v) => ({ ...v, scale: clamp(v.scale * ZOOM_FACTOR, MIN_SCALE, MAX_SCALE) }))}
          className="h-6 w-6 rounded text-xs"
          style={{
            backgroundColor: "var(--color-surface)",
            color: "var(--color-text-primary)",
            border: "1px solid var(--color-border)",
          }}
        >
          +
        </button>
        <button
          type="button"
          onClick={() => setView((v) => ({ ...v, scale: clamp(v.scale / ZOOM_FACTOR, MIN_SCALE, MAX_SCALE) }))}
          className="h-6 w-6 rounded text-xs"
          style={{
            backgroundColor: "var(--color-surface)",
            color: "var(--color-text-primary)",
            border: "1px solid var(--color-border)",
          }}
        >
          −
        </button>
        <button
          type="button"
          onClick={() => setView({ x: 0, y: 0, scale: 1 })}
          className="h-6 w-6 rounded text-xs"
          style={{
            backgroundColor: "var(--color-surface)",
            color: "var(--color-text-primary)",
            border: "1px solid var(--color-border)",
          }}
        >
          ⟲
        </button>
      </div>
    </div>
  );
}
