/**
 * Shared types between `jsonata.worker.ts` and `useJsonataPreview.ts`.
 */

/** Request payload sent from the main thread to the worker. */
export interface JsonataRequest {
  expression: string;
  input: unknown;
}

/**
 * Response payload sent from the worker back to the main thread.
 */
export interface JsonataResponse {
  ok: boolean;
  output?: unknown;
  error?: string;
  duration_ms: number;
}
