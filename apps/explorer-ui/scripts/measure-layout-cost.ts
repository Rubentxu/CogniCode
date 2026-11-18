/**
 * The cost of a layout, measured on purpose.
 *
 * The wall-clock budget of the layout does not live in the unit
 * suite: a threshold on a clock is not a deterministic test, and the
 * suite runs on machines whose speed nobody controls. What the unit
 * suite certifies is the CAUSE of the cost — the ELK runtime is built
 * once and reused — and this script measures the cost itself.
 *
 * Run it with:
 *   npm run bench:layout
 *
 * Two numbers come out:
 *   cold — the first layout of the process, which also pays the
 *          bootstrap of the ELK runtime;
 *   warm — the layouts that follow, which reuse that runtime.
 *
 * The verdict is the RATIO between them: when the runtime is built
 * again for every layout, the ratio is 1 and the cost of a re-layout
 * in the browser is the cold one. The absolute budget of 500 ms is
 * reported, not enforced: a machine under load says so, and the
 * ratio does not move with the load.
 */
import { createLayoutWorker } from "../src/components/InteractiveGraph/layout.worker";
import type { ElementsDefinition } from "cytoscape";

/** The size of the graph the cost is measured on. */
const NODES = 200;

/** The budget of a warm layout, in milliseconds. */
const WARM_BUDGET_MS = 500;

/**
 * The share of the cold cost a warm layout may reach. Measured on
 * 2026-11-18: cold 262 ms, warm 84 ms — a ratio of 0.32. A runtime
 * built per layout would give a ratio of 1.
 */
const MAX_WARM_RATIO = 0.6;

function chainElements(n: number): ElementsDefinition {
  const nodes = Array.from({ length: n }, (_, i) => ({ data: { id: `n${i}` } }));
  const edges = Array.from({ length: n - 1 }, (_, i) => ({
    data: { id: `e${i}`, source: `n${i}`, target: `n${i + 1}` },
  }));
  return { nodes, edges };
}

async function timed(run: () => Promise<unknown>): Promise<number> {
  const t0 = performance.now();
  await run();
  return performance.now() - t0;
}

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? ((sorted[mid - 1] ?? 0) + (sorted[mid] ?? 0)) / 2
    : (sorted[mid] ?? 0);
}

async function main(): Promise<void> {
  const worker = createLayoutWorker();
  const elements = chainElements(NODES);
  const options = { algorithm: "layered", animate: false } as const;

  const cold = await timed(() => worker.layout(elements, options));

  const warmRuns: number[] = [];
  for (let i = 0; i < 3; i++) {
    warmRuns.push(await timed(() => worker.layout(elements, options)));
  }
  const warm = median(warmRuns);
  const ratio = cold > 0 ? warm / cold : Number.POSITIVE_INFINITY;

  console.log(`nodes:               ${NODES}`);
  console.log(`cold layout:         ${cold.toFixed(1)} ms`);
  console.log(`warm layout:         ${warm.toFixed(1)} ms (median of ${warmRuns.map((v) => v.toFixed(1)).join(", ")})`);
  console.log(`warm / cold:         ${ratio.toFixed(3)}`);
  console.log(`budget:              ${WARM_BUDGET_MS} ms`);

  if (ratio > MAX_WARM_RATIO) {
    console.error(
      `\nFAIL: the warm layout costs ${(ratio * 100).toFixed(0)}% of the cold one (max ${(MAX_WARM_RATIO * 100).toFixed(0)}%). The ELK runtime is being built again for every layout.`,
    );
    process.exit(1);
  }

  if (warm > WARM_BUDGET_MS) {
    console.log(
      `\nNOTE: the warm layout is over the ${WARM_BUDGET_MS} ms budget. The ratio holds, so the runtime is reused; the machine is probably loaded.`,
    );
    return;
  }

  console.log("\nOK: the runtime is reused and the warm layout is inside the budget.");
}

await main();
