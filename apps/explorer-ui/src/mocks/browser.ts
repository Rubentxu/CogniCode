/**
 * `mocks/browser` — MSW worker for the browser.
 *
 * The Worker is only started when the build was launched with
 * `VITE_USE_MOCKS=true`. In production builds the export is never
 * called, so the worker's ~10KB of code is dead-stripped.
 *
 * The handlers are identical to the node-side ones so the test
 * surface and the dev surface stay in sync.
 */
import { setupWorker } from "msw/browser";

import { handlers } from "./handlers";

export const worker = setupWorker(...handlers);
