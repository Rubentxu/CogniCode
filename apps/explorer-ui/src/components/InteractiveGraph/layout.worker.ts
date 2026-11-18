/**
 * elkjs Web Worker — layout engine for `InteractiveGraph`.
 *
 * We deliberately expose a *factory* (`createLayoutWorker`) instead
 * of running the real `?worker` module under Vitest. The factory
 * accepts the same surface as the comlink-wrapped worker the bundle
 * produces, so tests and production code call them identically.
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
 *
 * The engine:
 * - The ELK runtime is built ONCE per JavaScript realm and shared
 *   by every worker of that realm. Building it pays the GWT
 *   bootstrap, and a layout that builds its own runtime pays that
 *   bootstrap on every call. Measured on 2026-11-18 over a
 *   200-node chain: a fresh runtime cost 262 ms (39 ms of
 *   constructor plus 223 ms of layout) where a warm one cost 84 ms.
 * - The engine is injectable, so the tests drive a double and
 *   observe the reuse without paying the bootstrap.
 * - The layouts of a worker run one after the other. The engine is
 *   shared, and two layouts on it at the same time would interleave.
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

/**
 * The part of the ELK runtime this worker drives.
 *
 * The production implementation is a real `ELK`; the tests inject a
 * double that counts how many times the runtime is built and that
 * observes whether two layouts overlap.
 */
export type LayoutEngine = {
  layout: (graph: ElkNode) => Promise<ElkNode>;
};

/**
 * The dependencies of a worker.
 *
 * `createEngine` builds the runtime the worker drives, and it is
 * called at most once per worker. Without it the worker takes the
 * runtime of the realm, which every worker of that realm shares.
 */
export type LayoutWorkerDeps = {
  createEngine?: () => LayoutEngine;
};

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

/**
 * The ELK runtime of this JavaScript realm.
 *
 * A browser tab and a test worker are two realms, so each one keeps
 * its own. The runtime is built the first time a layout asks for it.
 */
let realmEngine: LayoutEngine | null = null;

function realmEngineFactory(): LayoutEngine {
  if (!realmEngine) {
    const elk = new ELK();
    realmEngine = {
      layout: (graph) => elk.layout(graph),
    };
  }
  return realmEngine;
}

export function createLayoutWorker(deps: LayoutWorkerDeps = {}): LayoutWorker {
  const subscribers = new Set<ProgressCallback>();
  let inFlight: { cancelled: boolean } | null = null;
  let engine: LayoutEngine | null = null;
  // The layouts of a worker run one after the other: the engine is
  // shared, and two layouts on it at the same time would interleave.
  let queue: Promise<unknown> = Promise.resolve();

  function emit(p: number) {
    for (const cb of subscribers) cb(p);
  }

  function engineOf(): LayoutEngine {
    if (!engine) {
      engine = deps.createEngine ? deps.createEngine() : realmEngineFactory();
    }
    return engine;
  }

  async function runLayout(
    elements: ElementsDefinition,
    options: LayoutOptions,
    algorithm: LayoutAlgorithm,
    animate: boolean,
    token: { cancelled: boolean },
  ): Promise<ElementsDefinition> {
    // A layout cancelled before it started does not reach the engine:
    // the work it asked for is work nobody is waiting for.
    if (token.cancelled) {
      throw new LayoutCancelled();
    }

    // Build the elk graph from the cytoscape elements. Elk uses
    // `id`, `width`, `height` for nodes and `sources`/`targets`
    // for edges. We use uniform node sizing — the cytoscape
    // stylesheet refines visual dimensions later.
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

    const result = await engineOf().layout(elkGraph);

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

    // Only the layout that is still in flight clears the slot: a
    // layout queued behind it already took the slot over.
    if (inFlight === token) {
      inFlight = null;
    }
    return { nodes: outNodes, edges: outEdges };
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

      const run = queue.then(() =>
        runLayout(elements, options, algorithm, animate, token),
      );
      // The queue survives a failed layout: the next one runs.
      queue = run.then(
        () => undefined,
        () => undefined,
      );
      return run;
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
//
// The default surface is ONE worker. `cancel()` and `onProgress()`
// have to reach the layout that `layout()` started, and a worker
// built per call would leave both talking to an instance nobody
// waits for.
const defaultWorker = createLayoutWorker();

export default {
  layout: (elements: ElementsDefinition, options?: LayoutOptions) =>
    defaultWorker.layout(elements, options),
  cancel: () => defaultWorker.cancel(),
  onProgress: (cb: ProgressCallback) => defaultWorker.onProgress(cb),
};
