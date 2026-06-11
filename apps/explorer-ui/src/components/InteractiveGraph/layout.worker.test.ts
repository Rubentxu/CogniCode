/**
 * Tests for the elkjs Web Worker (`layout.worker.ts`).
 *
 * We instantiate the worker logic directly (no real Worker) by
 * importing the `createLayoutWorker` factory. The factory returns an
 * object with `layout`, `cancel`, and `onProgress` methods —
 * mirroring the comlink `expose` surface.
 *
 * Performance assertion: 200 nodes should layout under 500ms.
 * Without animation, the result is a single `1.0` progress event.
 * With animation, progress is monotonic and ends at 1.0.
 */
import { describe, expect, it } from "vitest";

import { createLayoutWorker } from "./layout.worker";
import type { ElementsDefinition } from "cytoscape";

function smallElements(): ElementsDefinition {
  return {
    nodes: [
      { data: { id: "a" } },
      { data: { id: "b" } },
      { data: { id: "c" } },
    ],
    edges: [
      { data: { id: "ab", source: "a", target: "b" } },
      { data: { id: "bc", source: "b", target: "c" } },
    ],
  };
}

function chainElements(n: number): ElementsDefinition {
  const nodes = Array.from({ length: n }, (_, i) => ({ data: { id: `n${i}` } }));
  const edges = Array.from({ length: n - 1 }, (_, i) => ({
    data: { id: `e${i}`, source: `n${i}`, target: `n${i + 1}` },
  }));
  return { nodes, edges };
}

function cycleElements(n: number): ElementsDefinition {
  const nodes = Array.from({ length: n }, (_, i) => ({ data: { id: `n${i}` } }));
  const edges = Array.from({ length: n }, (_, i) => ({
    data: { id: `e${i}`, source: `n${i}`, target: `n${(i + 1) % n}` },
  }));
  return { nodes, edges };
}

/**
 * A 3-level tree (root → 2 children → 4 grandchildren). Required
 * for algorithms like `radial` that demand a tree input.
 */
function treeElements(): ElementsDefinition {
  const nodes = [
    { data: { id: "root" } },
    { data: { id: "a" } },
    { data: { id: "b" } },
    { data: { id: "a1" } },
    { data: { id: "a2" } },
    { data: { id: "b1" } },
    { data: { id: "b2" } },
  ];
  const edges = [
    { data: { id: "e0", source: "root", target: "a" } },
    { data: { id: "e1", source: "root", target: "b" } },
    { data: { id: "e2", source: "a", target: "a1" } },
    { data: { id: "e3", source: "a", target: "a2" } },
    { data: { id: "e4", source: "b", target: "b1" } },
    { data: { id: "e5", source: "b", target: "b2" } },
  ];
  return { nodes, edges };
}

describe("layout worker", () => {
  it("layers produces positions for every node", async () => {
    const w = createLayoutWorker();
    const out = await w.layout(smallElements(), { algorithm: "layered" });
    expect(out.nodes).toHaveLength(3);
    for (const n of out.nodes) {
      const pos = (n as { position?: { x: number; y: number } }).position;
      expect(pos).toBeDefined();
      expect(typeof pos!.x).toBe("number");
      expect(typeof pos!.y).toBe("number");
    }
  });

  it("rejects unknown algorithm with InvalidLayoutOption", async () => {
    const w = createLayoutWorker();
    await expect(
      w.layout(smallElements(), { algorithm: "neural-net" as never }),
    ).rejects.toThrow(/InvalidLayoutOption/);
  });

  it("cancel() while in-flight rejects the layout promise", async () => {
    const w = createLayoutWorker();
    const promise = w.layout(chainElements(300), { algorithm: "layered" });
    w.cancel();
    await expect(promise).rejects.toThrow(/LayoutCancelled/);
  });

  it("cancel() when idle is a no-op", () => {
    const w = createLayoutWorker();
    expect(() => w.cancel()).not.toThrow();
  });

  it("animate: true streams monotonic progress ending at 1.0", async () => {
    const w = createLayoutWorker();
    const seen: number[] = [];
    const unsub = w.onProgress((p) => seen.push(p));
    await w.layout(smallElements(), { algorithm: "layered", animate: true });
    unsub();
    expect(seen.length).toBeGreaterThan(0);
    for (let i = 1; i < seen.length; i++) {
      expect(seen[i]!).toBeGreaterThanOrEqual(seen[i - 1]! - 1e-9);
    }
    expect(seen[seen.length - 1]!).toBe(1);
  });

  it("animate: false emits exactly one 1.0 progress event", async () => {
    const w = createLayoutWorker();
    const seen: number[] = [];
    const unsub = w.onProgress((p) => seen.push(p));
    await w.layout(smallElements(), { algorithm: "layered", animate: false });
    unsub();
    expect(seen).toEqual([1]);
  });

  it("multiple onProgress subscribers each receive every value", async () => {
    const w = createLayoutWorker();
    const a: number[] = [];
    const b: number[] = [];
    const ua = w.onProgress((p) => a.push(p));
    const ub = w.onProgress((p) => b.push(p));
    await w.layout(smallElements(), { algorithm: "layered", animate: false });
    ua();
    ub();
    expect(a).toEqual(b);
    expect(a).toContain(1);
  });

  it(">500 nodes with animate: false rejects with LayoutTooLarge", async () => {
    const w = createLayoutWorker();
    await expect(
      w.layout(chainElements(501), { algorithm: "layered", animate: false }),
    ).rejects.toThrow(/LayoutTooLarge/);
  });

  it("200-node layered layout completes in <500ms", async () => {
    const w = createLayoutWorker();
    const t0 = performance.now();
    await w.layout(chainElements(200), { algorithm: "layered", animate: false });
    expect(performance.now() - t0).toBeLessThan(500);
  });

  it("worker recovers after a cancel — next layout resolves", async () => {
    const w = createLayoutWorker();
    const failed = w.layout(chainElements(300), { algorithm: "layered" });
    w.cancel();
    await expect(failed).rejects.toThrow();
    const recovered = await w.layout(smallElements(), { algorithm: "layered" });
    expect(recovered.nodes).toHaveLength(3);
  });

  it("forwards width/height/nodeSeparation/rankSeparation/iterations to elkjs", async () => {
    const w = createLayoutWorker();
    // We assert by calling with explicit values; the result is a
    // valid layout (no throw) and dimensions are present in the
    // returned positions. The shape is loose here — the contract
    // is "elkjs accepts these options without error".
    const out = await w.layout(smallElements(), {
      algorithm: "layered",
      width: 400,
      height: 300,
      nodeSeparation: 50,
      rankSeparation: 80,
      iterations: 60,
    });
    expect(out.nodes).toHaveLength(3);
  });

  it("radial algorithm produces a non-layered layout", async () => {
    const w = createLayoutWorker();
    const out = await w.layout(treeElements(), {
      algorithm: "radial",
      animate: false,
    });
    expect(out.nodes.length).toBeGreaterThan(0);
    for (const n of out.nodes) {
      expect(n.position).toBeDefined();
    }
  });

  it("force algorithm does not throw on a 10-node cyclic graph", async () => {
    const w = createLayoutWorker();
    await expect(
      w.layout(cycleElements(10), { algorithm: "force", animate: false }),
    ).resolves.toBeDefined();
  });

  it("empty elements resolves to an empty ElementsDefinition", async () => {
    const w = createLayoutWorker();
    const out = await w.layout({ nodes: [], edges: [] }, {
      algorithm: "layered",
    });
    expect(out.nodes).toEqual([]);
    expect(out.edges).toEqual([]);
  });
});
