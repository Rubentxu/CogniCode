/**
 * `useJsonataPreview` — Debounced JSONata live preview hook.
 *
 * Lazily spawns `jsonata.worker.ts` on first expression change and
 * cancels in-flight executions when either `input` or `expression`
 * changes again.
 *
 * Debounce: 300ms (aligns with the 100ms execution budget + headroom).
 *
 * Returns `{ output, error, loading }`:
 * - `output`: the transformed result, or `null` when no expression is set
 * - `error`: a human-readable error string, or `null`
 * - `loading`: `true` when an execution is in-flight
 */
import { useCallback, useEffect, useRef, useState } from "react";

import type { JsonataRequest, JsonataResponse } from "../workers/jsonata.types";

export interface JsonataPreviewResult {
  output: unknown | null;
  error: string | null;
  loading: boolean;
}

const DEBOUNCE_MS = 300;
const TIMEOUT_MS = 100;
const MAX_INPUT_SIZE = 1 * 1024 * 1024; // 1 MB

/**
 * Execute JSONata against `input` with `expression`, returning
 * the result or error.
 *
 * @param expression  A valid JSONata expression string.
 * @param input       The JSON data to evaluate against.
 * @param signal      AbortSignal to cancel the execution.
 */
async function runJsonata(
  expression: string,
  input: unknown,
  signal: AbortSignal,
): Promise<{ output: unknown; duration_ms: number } | { error: string; duration_ms: number }> {
  return new Promise((resolve) => {
    if (signal.aborted) {
      resolve({ error: "Aborted", duration_ms: 0 });
      return;
    }

    // eslint-disable-next-line prefer-const -- worker is assigned below after declaration
    let worker: Worker;
    let settled = false;

    const timeoutId = setTimeout(() => {
      if (!settled) {
        settled = true;
        worker?.terminate();
        resolve({ error: "Evaluation timed out (100ms budget).", duration_ms: TIMEOUT_MS });
      }
    }, TIMEOUT_MS);

    signal.addEventListener("abort", () => {
      if (!settled) {
        settled = true;
        clearTimeout(timeoutId);
        worker?.terminate();
        resolve({ error: "Aborted", duration_ms: 0 });
      }
    });

    worker = new Worker(
      new URL("../workers/jsonata.worker.ts", import.meta.url),
      { type: "module" },
    );

    worker.onmessage = (event: MessageEvent<JsonataResponse>) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeoutId);
      if (event.data.ok) {
        resolve({ output: event.data.output, duration_ms: event.data.duration_ms });
      } else {
        resolve({ error: event.data.error ?? "Unknown error", duration_ms: event.data.duration_ms });
      }
    };

    worker.onerror = (err) => {
      if (settled) return;
      settled = true;
      clearTimeout(timeoutId);
      resolve({ error: `Worker error: ${err.message}`, duration_ms: 0 });
    };

    // 1 MB input cap — surface as an immediate error rather than sending to worker
    const serialized = JSON.stringify(input);
    if (serialized.length > MAX_INPUT_SIZE) {
      settled = true;
      clearTimeout(timeoutId);
      worker.terminate();
      resolve({
        error: `Input too large: ${(serialized.length / 1024).toFixed(1)} KB (max 1024 KB).`,
        duration_ms: 0,
      });
      return;
    }

    const request: JsonataRequest = { expression, input };
    worker.postMessage(request);
  });
}

export function useJsonataPreview(
  input: unknown,
  expression: string | null,
): JsonataPreviewResult {
  const [output, setOutput] = useState<unknown | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  // Derive isEmpty during render — no state needed for the clear case
  const isEmpty = expression === null || expression.trim().length === 0;

  // Track in-flight execution for debounce + race cancellation
  const abortControllerRef = useRef<AbortController | null>(null);
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const execute = useCallback(
    async (expr: string, data: unknown) => {
      // Cancel any previous in-flight execution
      abortControllerRef.current?.abort();
      const controller = new AbortController();
      abortControllerRef.current = controller;

      setLoading(true);
      setError(null);

      const result = await runJsonata(expr, data, controller.signal);

      // Guard: don't update state if the execution was superseded
      if (controller.signal.aborted) return;

      setLoading(false);
      if ("output" in result) {
        setOutput(result.output);
      } else {
        setError(result.error);
        setOutput(null);
      }
    },
    [], // stable — all state via refs
  );

  useEffect(() => {
    if (isEmpty) {
      abortControllerRef.current?.abort();
      if (debounceTimerRef.current !== null) {
        clearTimeout(debounceTimerRef.current);
        debounceTimerRef.current = null;
      }
      return;
    }

    // Debounce: reset the timer on every change
    if (debounceTimerRef.current !== null) {
      clearTimeout(debounceTimerRef.current);
    }

    debounceTimerRef.current = setTimeout(() => {
      debounceTimerRef.current = null;
      void execute(expression, input);
    }, DEBOUNCE_MS);

    return () => {
      if (debounceTimerRef.current !== null) {
        clearTimeout(debounceTimerRef.current);
        debounceTimerRef.current = null;
      }
    };
  }, [expression, input, execute, isEmpty]);

  return {
    output: isEmpty ? null : output,
    error: isEmpty ? null : error,
    loading: isEmpty ? false : loading,
  };
}
