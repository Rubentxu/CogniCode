/**
 * Shared renderer plumbing for the cytoscape adapters.
 *
 * Both the canvas and the WebGL adapter feed the same data path:
 * they translate a `Fixture` into cytoscape elements and drive a
 * `RendererController` over the resulting instance. The only
 * difference lives in the renderer config block.
 *
 * This module exposes the shared pieces so each adapter stays a thin
 * file that focuses on what makes it different.
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
import type { RendererController } from "./types";

/**
 * Pinned cytoscape version. Bump this when bumping the `cytoscape`
 * dependency in `package.json`.
 */
export const CYTOSCAPE_VERSION: string = "3.34.0";

/**
 * Map a `Fixture` to cytoscape `ElementDefinition`. Mirrors the
 * production `toCytoscapeElements` but consumes `Fixture` directly
 * so the bench subtree does not depend on the REST response types.
 */
export function fixtureToCytoscapeElements(
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
 * Controller wrapping a cytoscape instance. Both canvas and WebGL
 * adapters use this; the renderer config differs but the scenario
 * sequence is identical.
 */
export class CytoscapeController implements RendererController {
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
 * Mount a fixture through cytoscape using the provided renderer
 * config block. Validates the fixture first. Returns a controller
 * that already passed `fit()`.
 *
 * `rendererConfig` flows into cytoscape's init options under
 * `renderer`. The canvas adapter passes `{ name: "canvas" }`; the
 * WebGL adapter passes `{ name: "canvas", webgl: true }`.
 *
 * cytoscape requires a real DOM container. jsdom provides one, but
 * cytoscape's renderer init crashes if the container is null. The
 * adapter creates an unattached `<div>` here so both vitest (jsdom)
 * and the bench script (real browser) work with the same code path.
 */
export function mountCytoscape(
  fixture: Fixture,
  rendererConfig: Record<string, unknown>,
): CytoscapeController {
  try {
    assertFixture(fixture);
  } catch (err) {
    if (err instanceof FixtureValidationError) {
      throw new Error(`FixtureValidationError: ${err.message}`, { cause: err });
    }
    throw err;
  }

  const elements = fixtureToCytoscapeElements(fixture.nodes, fixture.edges);

  const container = document.createElement("div");
  container.style.width = "800px";
  container.style.height = "600px";

  const cy = cytoscape({
    container,
    elements: [...elements.nodes, ...elements.edges],
    style: buildStylesheet(),
    layout: { name: "preset" },
    renderer: rendererConfig,
  });

  cy.fit(undefined, 20);

  return new CytoscapeController(cy, true);
}