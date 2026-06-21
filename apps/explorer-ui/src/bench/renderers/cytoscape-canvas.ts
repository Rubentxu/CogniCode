/**
 * Cytoscape canvas renderer adapter.
 *
 * Baseline renderer. Every future renderer adapter must beat this
 * baseline to justify migration.
 *
 * Production `InteractiveGraph` also uses canvas, but this adapter
 * takes its input from the bench `Fixture` shape -- not the REST
 * `SubgraphResponse`. Reuse of production code is limited to the
 * cytoscape stylesheet; the data path is independently mirrored so
 * the bench subtree does not depend on the REST types.
 */

import {
  FixtureValidationError,
  type Fixture,
} from "../fixture-schema";
import {
  type BenchConfig,
  type MountHooks,
  type RendererAdapter,
  RendererMountError,
} from "./types";
import {
  CYTOSCAPE_VERSION,
  mountCytoscape,
} from "./cytoscape-shared";

/**
 * Cytoscape renderer config used by the canvas adapter.
 * `name: "canvas"` is cytoscape's default; we pass it explicitly so
 * the renderer config block is observable in metrics records.
 */
const CANVAS_RENDERER_CONFIG = Object.freeze({
  name: "canvas",
});

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
  ): Promise<ReturnType<typeof mountCytoscape>> {
    hooks.onLoadStart?.();
    hooks.onLoadEnd?.();
    hooks.onFirstRender?.();

    try {
      const controller = mountCytoscape(fixture, { ...CANVAS_RENDERER_CONFIG });
      hooks.onFit?.();
      return controller;
    } catch (err) {
      if (err instanceof FixtureValidationError) {
        throw new RendererMountError(this.id, err.message, err);
      }
      throw new RendererMountError(
        this.id,
        `failed to mount cytoscape canvas: ${describeError(err)}`,
        err,
      );
    }
  }
}

function describeError(err: unknown): string {
  if (err instanceof Error) return err.message;
  return String(err);
}