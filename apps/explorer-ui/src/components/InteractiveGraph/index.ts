/**
 * Public surface for the `InteractiveGraph` component.
 *
 * The component is a heavy dependency (cytoscape + elkjs). Callers
 * should import via `React.lazy(() => import("./InteractiveGraph"))`
 * to keep it out of the initial bundle.
 */
export { InteractiveGraph } from "./InteractiveGraph";
export type { InteractiveGraphProps } from "./InteractiveGraph";
export { toCytoscapeElements } from "./adapter";
export {
  buildStylesheet,
  resolveNodeStyleClass,
  KNOWN_NODE_CLASSES,
  KNOWN_EDGE_CLASSES,
  applyCorroborationStyles,
} from "./stylesheet";
export {
  createLayoutWorker,
  InvalidLayoutOption,
  LayoutCancelled,
  LayoutTooLarge,
} from "./layout.worker";
export type { LayoutAlgorithm, LayoutOptions, LayoutWorker } from "./layout.worker";
