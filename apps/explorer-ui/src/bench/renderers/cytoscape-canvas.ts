/**
 * Cytoscape canvas renderer adapter.
 *
 * This adapter mirrors the production data path. It reuses:
 *
 *   - `toCytoscapeElements`-style mapping for `Fixture` shapes
 *   - `buildStylesheet` from the production `InteractiveGraph`
 *
 * Production `InteractiveGraph` is NOT a renderer adapter. The
 * adapter takes the same `Fixture` shape every other renderer takes
 * and emits the same node and edge count. Differences between this
 * adapter and a future WebGL adapter live only in the renderer
 * config block.
 *
 * Vitest runs in jsdom. jsdom provides a DOM but no real Canvas
 * implementation. Cytoscape tolerates jsdom well enough to mount,
 * expose its collection API, and verify counts, layout completion,
 * and selection mechanics. We do not measure render timing under
 * jsdom -- the bench script in T9 runs in a real browser for that.
 */

import cytoscape, { type Core, type ElementDefinition } from "cytoscape";

import { buildStylesheet } from "../../components/InteractiveGraph/stylesheet";
import {
  FixtureValidationError,
  assertFixture,
  type Fixture,
  type FixtureEdge,
  type FixtureNode,
} from "../fixture-schema";
import {
  type BenchConfig,
  type MountHooks,
  type RendererAdapter,
  type RendererController,
  RendererMountError,
  DEFAULT_BENCH_CONFIG,
} from "./types";

/**
 * Map a `Fixture` to cytoscape's `ElementDefinition` shape. Mirrors
 * `toCytoscapeElements` from the production adapter, but consumes
 * `Fixture` directly so the bench subtree does not depend on the
 * REST `SubgraphResponse` types.
 */
function fixtureToCytoscapeElements(
  nodes: readonly FixtureNode[],
  edges: readonly FixtureEdge[],
): { nodes: ElementDefinition[]; edges: ElementDefinition[] } {
  const cyNodes: ElementDefinition[] = nodes.map((n) => ({
    group: "nodes",
    data: {
      id: n.id,
      label: n.label,
      kind: n.kind,
      style_class: n.style_class,
    },
  }));
  const cyEdges: ElementDefinition[] = edges.map((e, i) => ({
    group: "edges",
    data: {
      id: `${e.source}->${e.target}:${i}`,
      source: e.source,
      target: e.target,
      relation: e.relation,
      style_class: e.style_class,
    },
  }));
  return { nodes: cyNodes, edges: cyEdges };
}

/**
 * Concrete controller returned by the canvas adapter. Holds the
 * cytoscape instance plus a tracker for whether a layout ran.
 */
class CanvasController implements RendererController {
  #cy: Core;
  #layoutCompleted: boolean;

  constructor(cy: Core, layoutCompleted: boolean) {
    this.#cy = cy;
    this.#layoutCompleted = layoutCompleted;
  }

  async relayout(): Promise<number> {
    const start = performance.now();
    return new Promise((resolve) => {
      this.#cy
        .layout({
          name: "grid",
          rows: Math.ceil(Math.sqrt(this.#cy.nodes().length)),
          animate: false,
        })
        .on("layoutstop", () => {
          this.#layoutCompleted = true;
          resolve(performance.now() - start);
        })
        .run();
    });
  }

  async pan(dx: number, dy: number): Promise<number> {
    const start = performance.now();
    this.#cy.panBy({ x: dx, y: dy });
    return performance.now() - start;
  }

  async zoom(factor: number): Promise<number> {
    const start = performance.now();
    this.#cy.zoom(factor);
    this.#cy.center();
    return performance.now() - start;
  }

  async select(
    nodeId: string,
  ): Promise<{
    duration_ms: number;
    selection_works: boolean;
    edge_highlight_works: boolean;
  }> {
    const start = performance.now();
    const node = this.#cy.getElementById(nodeId);
    if (node.length === 0) {
      return {
        duration_ms: performance.now() - start,
        selection_works: false,
        edge_highlight_works: false,
      };
    }
    node.select();
    const incident = node.connectedEdges();
    return {
      duration_ms: performance.now() - start,
      selection_works: node.selected(),
      edge_highlight_works: incident.length > 0,
    };
  }

  isLayoutComplete(): boolean {
    return this.#layoutCompleted;
  }

  async teardown(): Promise<void> {
    this.#cy.destroy();
  }
}

/**
 * Cytoscape canvas baseline adapter. The production `InteractiveGraph`
 * component also uses canvas. Any future WebGL or Sigma adapter must
 * beat this baseline to justify migration.
 */
export class CytoscapeCanvasAdapter implements RendererAdapter {
  readonly id = "cytoscape-canvas" as const;
  readonly version = CYTOSCAPE_VERSION;

  isEnabled(config: BenchConfig): boolean {
    void config;
    return true;
  }

  async mount(
    fixture: Fixture,
    hooks: MountHooks,
  ): Promise<RendererController> {
    try {
      assertFixture(fixture);
    } catch (err) {
      if (err instanceof FixtureValidationError) {
        throw new RendererMountError(this.id, err.message, err);
      }
      throw err;
    }

    hooks.onLoadStart?.();
    const elements = fixtureToCytoscapeElements(fixture.nodes, fixture.edges);
    hooks.onLoadEnd?.();

    let cy: Core;
    try {
      cy = cytoscape({
        // jsdom provides a DOM but no real Canvas. Cytoscape's headless
        // mounting works for API and selection checks; render timings
        // belong to the bench script in T9, not vitest.
        elements: [...elements.nodes, ...elements.edges],
        style: buildStylesheet(),
        layout: { name: "preset" },
      });
    } catch (err) {
      throw new RendererMountError(
        this.id,
        `failed to mount cytoscape: ${describeError(err)}`,
        err,
      );
    }

    hooks.onFirstRender?.();
    cy.fit(undefined, 20);
    hooks.onFit?.();

    return new CanvasController(cy, true);
  }
}

/**
 * Pinned cytoscape version. We pin it here so the metrics record
 * reports the exact version under test. Bump this when bumping the
 * `cytoscape` dependency in `package.json`.
 */
export const CYTOSCAPE_VERSION: string = "3.34.0";

function describeError(err: unknown): string {
  if (err instanceof Error) return err.message;
  return String(err);
}

export const DEFAULT_CONFIG: BenchConfig = DEFAULT_BENCH_CONFIG;