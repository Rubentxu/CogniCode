/**
 * @deprecated Use `RenderSvgGraph` from `../GraphView/render` instead.
 * This re-export exists for backward compatibility.
 *
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
export { RenderSvgGraph as SvgGraph } from "../GraphView/render";
export type { RenderAdapter as SvgGraphProps } from "../GraphView/render";
