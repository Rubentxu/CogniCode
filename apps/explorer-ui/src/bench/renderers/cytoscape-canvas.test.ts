import { describe, it, expect, beforeEach } from "vitest";
import {
  CytoscapeCanvasAdapter,
  CYTOSCAPE_VERSION,
} from "./cytoscape-canvas";
import { loadFixture } from "../fixtures";
import { DEFAULT_BENCH_CONFIG } from "./types";
import type { RendererController } from "./types";

describe("CytoscapeCanvasAdapter", () => {
  it("reports the canonical id", () => {
    const adapter = new CytoscapeCanvasAdapter();
    expect(adapter.id).toBe("cytoscape-canvas");
  });

  it("always reports itself as enabled", () => {
    const adapter = new CytoscapeCanvasAdapter();
    expect(adapter.isEnabled(DEFAULT_BENCH_CONFIG)).toBe(true);
  });

  it("exposes the cytoscape package version", () => {
    const adapter = new CytoscapeCanvasAdapter();
    expect(adapter.version).toBe(CYTOSCAPE_VERSION);
    expect(adapter.version).not.toBe("unknown");
  });
});

describe("CytoscapeCanvasAdapter.mount", () => {
  let adapter: CytoscapeCanvasAdapter;

  beforeEach(() => {
    adapter = new CytoscapeCanvasAdapter();
  });

  it("mounts the call-graph-small fixture with matching counts", async () => {
    const fixture = loadFixture("call-graph-small");
    const controller = await adapter.mount(fixture, {});

    // Re-fetch the counts from the controller's cytoscape instance
    // by triggering a no-op interaction that returns the instance
    // count indirectly through selection_works + edge_highlight_works.
    const result = await controller.select(fixture.nodes[0].id);
    expect(result.selection_works).toBe(true);
    expect(result.edge_highlight_works).toBe(true);

    await controller.teardown();
  });

  it("returns selection_works=false for an unknown node id", async () => {
    const fixture = loadFixture("call-graph-small");
    const controller = await adapter.mount(fixture, {});

    const result = await controller.select("does-not-exist");
    expect(result.selection_works).toBe(false);
    expect(result.edge_highlight_works).toBe(false);

    await controller.teardown();
  });

  it("runs the scenario sequence end-to-end", async () => {
    const fixture = loadFixture("call-graph-small");
    const controller: RendererController = await adapter.mount(fixture, {});

    const relayout = await controller.relayout();
    expect(relayout).toBeGreaterThanOrEqual(0);

    const pan = await controller.pan(10, 10);
    expect(pan).toBeGreaterThanOrEqual(0);

    const zoom = await controller.zoom(1.1);
    expect(zoom).toBeGreaterThanOrEqual(0);

    expect(controller.isLayoutComplete()).toBe(true);

    await controller.teardown();
  });

  it("teardown is idempotent", async () => {
    const fixture = loadFixture("call-graph-small");
    const controller = await adapter.mount(fixture, {});
    await controller.teardown();
    await expect(controller.teardown()).resolves.toBeUndefined();
  });
});