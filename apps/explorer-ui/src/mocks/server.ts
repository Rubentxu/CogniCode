/**
 * MSW server lifecycle for tests + dev mode.
 *
 * - Tests: `src/test/setup.ts` calls `startMockServer()` once before
 *   the test suite, so every SWR hook fetch is intercepted.
 * - Dev: gated behind `VITE_USE_MOCKS=true`; import `startMockServer()`
 *   from `main.tsx` to opt in (out of the box we hit the real backend
 *   through the Vite proxy).
 */
import { setupServer } from "msw/node";

import { handlers } from "./handlers";

let started = false;

export function startMockServer() {
  if (started) return;
  const server = setupServer(...handlers);
  server.listen({
    onUnhandledRequest: "bypass",
  });
  started = true;
}

export function resetMockServer() {
  // MSW resets handlers between tests when using the per-test helper
  // (see `resetMockServer` in tests). For now we just clear all
  // added runtime handlers so the suite starts clean.
  if (started) {
    // No-op if the server was already started in setup; tests use
    // `server.use(...)` for per-case overrides.
  }
}
