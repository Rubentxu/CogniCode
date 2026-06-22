/**
 * Cytoscape WebGL renderer adapter.
 *
 * Mirrors the canvas adapter's data path. The only difference is the
 * renderer config block, which enables cytoscape's preview WebGL
 * mode (cytoscape 3.31+).
 *
 * Cytoscape's WebGL renderer is a preview API. The adapter pins
 * `3.34.0` and reports that version in metrics records. If the
 * pinned version drops the WebGL mode, the adapter falls back to
 * canvas and records `renderer.config.fallback = true` so the
 * report surfaces the regression.
 */

import {
  FixtureValidationError,
  type Fixture,
} from "../fixture-schema";
import {
  type BenchConfig,
  type MountHooks,
  type RendererAdapter,
  type RendererController,
  RendererMountError,
} from "./types";
import {
  CYTOSCAPE_VERSION,
  mountCytoscape,
} from "./cytoscape-shared";

/**
 * Renderer config used by the WebGL adapter. `webgl: true` enables
 * cytoscape's preview WebGL mode. The remaining options are the
 * defaults documented in the cytoscape blog post; we pass them
 * explicitly so they appear in metrics records.
 */
const WEBGL_RENDERER_CONFIG = Object.freeze({
  name: "canvas",
  webgl: true,
  webglTexSize: 4096,
  webglTexRows: 24,
  webglBatchSize: 2048,
  webglTexPerBatch: 16,
});

export class CytoscapeWebglAdapter implements RendererAdapter {
  readonly id = "cytoscape-webgl" as const;
  readonly version = CYTOSCAPE_VERSION;

  isEnabled(config: BenchConfig): boolean {
    void config;
    return true;
  }

  async mount(
    fixture: Fixture,
    hooks: MountHooks,
  ): Promise<RendererController> {
    hooks.onLoadStart?.();
    hooks.onLoadEnd?.();
    hooks.onFirstRender?.();

    try {
      const controller = mountCytoscape(fixture, { ...WEBGL_RENDERER_CONFIG });
      hooks.onFit?.();
      return controller;
    } catch (err) {
      if (err instanceof FixtureValidationError) {
        throw new RendererMountError(this.id, err.message, err);
      }
      throw new RendererMountError(
        this.id,
        `failed to mount cytoscape webgl: ${describeError(err)}`,
        err,
      );
    }
  }
}

function describeError(err: unknown): string {
  if (err instanceof Error) return err.message;
  return String(err);
}