/**
 * `viewKind.ts` — ViewKind-level dispatch helpers.
 *
 * Exported for use by PaneInspector and any component that needs to
 * determine how to render a `ContextualView` without importing the
 * full PaneInspector.
 *
 * `isGraphViewKind` is kept as an exported helper (not in JSX render
 * path per decision #4 in the design) — it is used in docs and
 * analytics, not in dispatch.
 */
import type { RendererKind } from "../../api/schemas";
import type { ContextualView } from "../../api/types";

// ============================================================================
// Graph-shaped ViewKinds
// ============================================================================

/**
 * First-class graph-shaped ViewKinds that route to `GraphViewRenderer`
 * via the `rendererRegistry["graph"]` entry.
 */
export const GRAPH_KINDS = new Set([
  "call_graph",
  "dependency_graph",
  "data_flow",
  "impact_radius",
  "seam_map",
] as const);

export type GraphViewKind = (typeof GRAPH_KINDS)[number];

/**
 * Returns true when `kind` is a graph-shaped ViewKind.
 * Kept as a zero-cost exported helper — NOT in the JSX render path.
 */
export function isGraphViewKind(kind: string | undefined): boolean {
  return kind != null && GRAPH_KINDS.has(kind as GraphViewKind);
}

// ============================================================================
// Render strategy resolver
// ============================================================================

/**
 * Render strategy — the output of `resolveRenderStrategy`.
 *
 * `registry` means route through `rendererRegistry.render(rendererKind, body)`.
 * `blocks`   means render via the `Blocks` component.
 */
export type RenderStrategy =
  | { kind: "registry"; rendererKind: RendererKind }
  | { kind: "blocks" };

/**
 * Determine how to render a `ContextualView`.
 *
 * Resolution order:
 * 1. If `view_kind` is a known graph ViewKind → registry path with `"graph"`
 * 2. If `renderer_kind` is set and not `"json"` → registry path with that kind
 * 3. Otherwise → blocks path
 *
 * This exists because `ContextualView.renderer_kind` defaults to `"json"`
 * (schemas.ts L805), NOT `undefined`. A naive `renderer_kind` dispatch would
 * route all block-based views to JsonRenderer, breaking 29 block renderers.
 */
export function resolveRenderStrategy(view: ContextualView): RenderStrategy {
  // 1. Graph ViewKind → graph registry entry
  if (isGraphViewKind(view.view_kind)) {
    return { kind: "registry", rendererKind: "graph" };
  }

  // 2. Explicit renderer_kind (not "json" which is the default/fallback)
  if (view.renderer_kind != null && view.renderer_kind !== "json") {
    return { kind: "registry", rendererKind: view.renderer_kind };
  }

  // 3. Default: render blocks
  return { kind: "blocks" };
}
