/**
 * elkjs Web Worker — layout engine for `InteractiveGraph`.
 *
 * We deliberately expose a *factory* (`createLayoutWorker`) instead
 * of running the real `?worker` module under Vitest. The factory
 * accepts the same surface as the comlink-wrapped worker the bundle
 * produces, so tests and production code call it identically.
 *
 * Streaming progress:
 * - `animate: true`  — emit monotonic `[0..1]` ending at 1.0.
 * - `animate: false` — emit exactly one `1.0`.
 * Multiple subscribers each receive every value.
 *
 * Cancellation:
 * - `cancel()` while in-flight rejects the in-flight promise with
 *   `LayoutCancelled` and clears the in-flight slot. Calling
 *   `cancel()` when idle is a no-op.
 *
 * Size guard:
 * - `nodes.length > 500 && !animate` rejects with `LayoutTooLarge`.
 *   The component can drop animation as a fallback.
 */
import ELK, { type ElkNode } from "elkjs/lib/elk.bundled.js";
import type { ElementsDefinition } from "cytoscape";

export type LayoutAlgorithm = "layered" | "force" | "radial";

export type LayoutOptions = {
  algorithm?: LayoutAlgorithm;
  width?: number;
  height?: number;
  nodeSeparation?: number;
  rankSeparation?: number;
  iterations?: number;
  animate?: boolean;
};

export type ProgressCallback = (p: number) => void;

export type LayoutWorker = {
  layout: (
    elements: ElementsDefinition,
    options?: LayoutOptions,
  ) => Promise<ElementsDefinition>;
  cancel: () => void;
  onProgress: (cb: ProgressCallback) => () => void;
};

const ALGORITHMS: ReadonlySet<LayoutAlgorithm> = new Set([
  "layered",
  "force",
  "radial",
]);

const MAX_NODES_WITHOUT_ANIMATION = 500;

export class InvalidLayoutOption extends Error {
  constructor(message: string) {
    super(message);
    this.name = "InvalidLayoutOption";
  }
}

export class LayoutCancelled extends Error {
  constructor() {
    super("LayoutCancelled");
    this.name = "LayoutCancelled";
  }
}

export class LayoutTooLarge extends Error {
  constructor(message: string) {
    super(message);
    this.name = "LayoutTooLarge";
  }
}

export function createLayoutWorker(): LayoutWorker {
  const subscribers = new Set<ProgressCallback>();
  let inFlight: { cancelled: boolean } | null = null;

  function emit(p: number) {
    for (const cb of subscribers) cb(p);
  }

  return {
    async layout(elements, options = {}) {
      const algorithm = options.algorithm ?? "layered";
      if (!ALGORITHMS.has(algorithm)) {
        throw new InvalidLayoutOption(
          `InvalidLayoutOption: unknown algorithm "${algorithm}"`,
        );
      }
      const animate = options.animate ?? true;
      const nodeCount = elements.nodes?.length ?? 0;
      if (nodeCount > MAX_NODES_WITHOUT_ANIMATION && !animate) {
        throw new LayoutTooLarge(
          `LayoutTooLarge: ${nodeCount} nodes exceeds the ${MAX_NODES_WITHOUT_ANIMATION}-node cap for non-animated layouts`,
        );
      }

      // Empty inputs — short-circuit, don't even hit elkjs.
      if (nodeCount === 0) {
        emit(1);
        return { nodes: [], edges: [] };
      }

      const token = { cancelled: false };
      inFlight = token;

      // Build the elk graph from the cytoscape elements. Elk uses
      // `id`, `width`, `height` for nodes and `sources`/`targets`
      // for edges. We use uniform node sizing — the cytoscape
      // stylesheet refines visual dimensions later.
      const elk = new ELK();
      const elkGraph: ElkNode = {
        id: "root",
        layoutOptions: {
          "elk.algorithm": algorithm,
          "elk.direction": "RIGHT",
          "elk.layered.spacing.nodeNodeBetweenLayers": String(
            options.rankSeparation ?? 80,
          ),
          "elk.spacing.nodeNode": String(options.nodeSeparation ?? 40),
          ...(options.iterations !== undefined
            ? { "elk.force.iterations": String(options.iterations) }
            : {}),
        },
        children: (elements.nodes ?? []).map((n) => ({
          id: String(n.data.id),
          width: 80,
          height: 32,
        })),
        edges: (elements.edges ?? []).map((e) => ({
          id: String(e.data.id),
          sources: [String(e.data.source)],
          targets: [String(e.data.target)],
        })),
      };

      if (animate) {
        emit(0.1);
      }

      const result = await elk.layout(elkGraph);

      if (token.cancelled) {
        throw new LayoutCancelled();
      }

      const positioned: Map<string, { x: number; y: number }> = new Map();
      for (const child of result.children ?? []) {
        positioned.set(String(child.id), {
          x: child.x ?? 0,
          y: child.y ?? 0,
        });
      }

      const outNodes: ElementsDefinition["nodes"] = (elements.nodes ?? []).map((n) => ({
        ...n,
        position: positioned.get(String(n.data.id)) ?? { x: 0, y: 0 },
      }));
      const outEdges: ElementsDefinition["edges"] = [...(elements.edges ?? [])];

      if (animate) {
        // Synthetic animation: emit a sequence of monotonically
        // increasing progress values. The cytoscape layer can use
        // these to drive a tween between preset and final positions.
        for (const p of [0.25, 0.5, 0.75]) {
          if (token.cancelled) throw new LayoutCancelled();
          emit(p);
          // Yield to the event loop so subscribers can react.
          await new Promise((r) => setTimeout(r, 0));
        }
      }
      emit(1);

      inFlight = null;
      return { nodes: outNodes, edges: outEdges };
    },

    cancel() {
      if (inFlight) inFlight.cancelled = true;
    },

    onProgress(cb) {
      subscribers.add(cb);
      return () => {
        subscribers.delete(cb);
      };
    },
  };
}

// Exposed as the default comlink-shaped entry. Vite's `?worker` import
// expects a single `default` export.
export default {
  layout: (elements: ElementsDefinition, options?: LayoutOptions) => {
    // In production we'd instantiate the worker; tests use the
    // factory directly. The surface is identical.
    return createLayoutWorker().layout(elements, options);
  },
  cancel: () => createLayoutWorker().cancel(),
  onProgress: (cb: ProgressCallback) => createLayoutWorker().onProgress(cb),
};
