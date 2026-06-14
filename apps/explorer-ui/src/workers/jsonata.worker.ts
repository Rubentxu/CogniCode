/**
 * `jsonata.worker.ts` — Sandboxed JSONata execution Web Worker.
 *
 * Loaded lazily (dynamic `import()`) only when the TransformStep
 * mounts and the user has entered a JSONata expression.
 *
 * Safety guarantees:
 * - 100ms evaluation timeout via `setTimeout` + `worker.terminate()`
 * - 1MB input size cap before sending to worker
 * - Structured error response for parse / runtime / timeout errors
 *
 * Protocol:
 *   main → worker:  `{ expression: string, input: unknown }`
 *   worker → main:  `{ ok: true, output: unknown, duration_ms: number }`
 *                or `{ ok: false, error: string, duration_ms: number }`
 */

import type { JsonataRequest, JsonataResponse } from "./jsonata.types";

const MAX_INPUT_SIZE = 1 * 1024 * 1024; // 1 MB

self.onmessage = async (event: MessageEvent<JsonataRequest>) => {
  const { expression, input } = event.data;
  const serialized = JSON.stringify(input);

  // 1 MB input cap
  if (serialized.length > MAX_INPUT_SIZE) {
    const response: JsonataResponse = {
      ok: false,
      error: `Input too large: ${(serialized.length / 1024).toFixed(1)} KB (max 1024 KB).`,
      duration_ms: 0,
    };
    self.postMessage(response);
    return;
  }

  const start = performance.now();

  try {
    // Lazy-load jsonata only when first needed
    const jsonataModule = await import("jsonata");
    const jsonata = jsonataModule.default;

    const expr = jsonata(expression);
    const output = await expr.evaluate(input);
    const duration_ms = Math.round(performance.now() - start);

    const response: JsonataResponse = {
      ok: true,
      output,
      duration_ms,
    };
    self.postMessage(response);
  } catch (err) {
    const duration_ms = Math.round(performance.now() - start);
    const message =
      err instanceof Error ? err.message : typeof err === "string" ? err : "Unknown JSONata error";
    const response: JsonataResponse = {
      ok: false,
      error: message,
      duration_ms,
    };
    self.postMessage(response);
  }
};

// Export for type-checking only — worker context doesn't use ES module exports
export type {};
