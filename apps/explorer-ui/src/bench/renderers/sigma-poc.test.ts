import {
  describe,
  it,
  expect,
  beforeEach,
} from "vitest";

import { SigmaPocAdapter, resetSigmaMock, seedSigmaMock } from "./sigma-poc";
import { loadFixture } from "../fixtures";
import {
  DEFAULT_BENCH_CONFIG,
  type RendererController,
} from "./types";

/**
 * Sigma.js needs WebGL. jsdom cannot run it. The Sigma adapter is
 * verified through:
 *
 *   - `isEnabled` contract -- the only test that does NOT need a
 *     real browser
 *   - structural mount -- exercises the adapter contract without
 *     invoking the real sigma module
 *
 * The real mount path lives in the bench script (T9, real browser).
 */
describe("SigmaPocAdapter gating", () => {
  it("is disabled by default", () => {
    const adapter = new SigmaPocAdapter();
    expect(adapter.isEnabled(DEFAULT_BENCH_CONFIG)).toBe(false);
  });

  it("is enabled when the config opts in", () => {
    const adapter = new SigmaPocAdapter();
    const config = { ...DEFAULT_BENCH_CONFIG, enable_sigma: true };
    expect(adapter.isEnabled(config)).toBe(true);
  });

  it("reports the canonical id", () => {
    const adapter = new SigmaPocAdapter();
    expect(adapter.id).toBe("sigma-poc");
  });

  it("is not the same id as the cytoscape adapters", () => {
    const adapter = new SigmaPocAdapter();
    expect(adapter.id).not.toBe("cytoscape-canvas");
    expect(adapter.id).not.toBe("cytoscape-webgl");
  });
});

describe("SigmaPocAdapter.mount", () => {
  let adapter: SigmaPocAdapter;

  beforeEach(() => {
    adapter = new SigmaPocAdapter();
    resetSigmaMock();
  });

  it("mounts even when not enabled (gating is the runner's job)", async () => {
    // The runner never invokes `mount` for disabled adapters -- the
    // gate lives in the runner. The adapter itself only reports
    // `isEnabled`. This test documents that contract.
    const fixture = loadFixture("call-graph-small");
    seedSigmaMock(fixture);

    const controller: RendererController = await adapter.mount(fixture, {});
    expect(controller).toBeDefined();

    await controller.teardown();
  });

  it("mounts the call-graph-small fixture with working selection", async () => {
    const adapterEnabled = new SigmaPocAdapter();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (adapterEnabled as any).isEnabled = () => true;

    const fixture = loadFixture("call-graph-small");
    seedSigmaMock(fixture);

    const controller: RendererController = await adapterEnabled.mount(
      fixture,
      {},
    );

    const result = await controller.select(fixture.nodes[0]!.id);
    expect(result.selection_works).toBe(true);
    expect(result.edge_highlight_works).toBe(true);

    await controller.teardown();
  });

  it("returns selection_works=false for an unknown node id", async () => {
    const adapterEnabled = new SigmaPocAdapter();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (adapterEnabled as any).isEnabled = () => true;

    const fixture = loadFixture("call-graph-small");
    seedSigmaMock(fixture);

    const controller = await adapterEnabled.mount(fixture, {});
    const result = await controller.select("does-not-exist");
    expect(result.selection_works).toBe(false);
    expect(result.edge_highlight_works).toBe(false);

    await controller.teardown();
  });

  it("runs the scenario sequence end-to-end", async () => {
    const adapterEnabled = new SigmaPocAdapter();
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (adapterEnabled as any).isEnabled = () => true;

    const fixture = loadFixture("call-graph-small");
    seedSigmaMock(fixture);

    const controller = await adapterEnabled.mount(fixture, {});
    expect(await controller.relayout()).toBeGreaterThanOrEqual(0);
    expect(await controller.pan(10, 10)).toBeGreaterThanOrEqual(0);
    expect(await controller.zoom(1.1)).toBeGreaterThanOrEqual(0);
    expect(controller.isLayoutComplete()).toBe(true);

    await controller.teardown();
  });
});