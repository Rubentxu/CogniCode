/**
 * MSW node server used by tests.
 *
 * `src/test/setup.ts` calls `server.listen()` before the suite runs;
 * tests use `server.use(...)` to override handlers per-case (e.g. to
 * simulate a 404 + retry path).
 */
import { setupServer } from "msw/node";

import { handlers } from "./handlers";

export const server = setupServer(...handlers);
