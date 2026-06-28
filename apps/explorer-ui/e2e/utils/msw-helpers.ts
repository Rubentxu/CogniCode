/**
 * MSW test helpers — deterministic state setup for E2E specs.
 *
 * Centralises common patterns so individual specs stay focused on
 * user-visible assertions:
 *
 * - `freezeTime()` — pins `Date.now()` to a fixed instant so timestamps
 *   in screenshots don't drift between runs.
 * - `waitForNoPendingRequests()` — drains MSW handlers before screenshot
 *   capture so no race-condition half-renders slip into the baseline.
 * - `seedWorkspace(...)` — sets up the MSW fixture state for a known
 *   workspace (entry points, hot paths, god nodes, etc.).
 *
 * MSW is loaded by the app via `worker.start()` when `VITE_USE_MOCKS=true`
 * (see `playwright.config.ts`). Helpers here run inside the page context.
 */
import type { Page } from "@playwright/test";

/**
 * Pin the page's `Date.now()` to a fixed instant.
 *
 * Useful for time-dependent UI like scan progress, "x minutes ago"
 * labels, and TTL-driven animations. Pair with a freezable timestamp.
 */
export async function freezeTime(page: Page, iso: string = "2026-06-27T10:00:00Z"): Promise<void> {
  await page.addInitScript((frozenIso: string) => {
    const fixed = new Date(frozenIso).getTime();
    const realNow = Date.now.bind(Date);
    // Override Date.now + Date constructor for deterministic clocks.
    // We don't override Date completely because some libs (e.g. dayjs)
    // rely on the original prototype.
    // eslint-disable-next-line no-extend-native
    Date.now = () => fixed;
    // eslint-disable-next-line no-extend-native
    const realDate = Date;
    // eslint-disable-next-line no-global-assign
    Date = class extends realDate {
      constructor(...args: ConstructorParameters<typeof Date>) {
        if (args.length === 0) {
          super(fixed);
        } else {
          // @ts-expect-error spread args
          super(...args);
        }
      }
      static now(): number {
        return fixed;
      }
    };
    // Expose for tests to verify freeze worked.
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (window as any).__FROZEN_NOW__ = fixed;
    // Sanity check.
    if (realNow() < 0) throw new Error("unreachable");
  }, iso);
}

/**
 * Wait for the MSW handler queue to drain before continuing.
 *
 * Useful before screenshots so we don't capture a half-rendered state
 * where a request is still in-flight.
 */
export async function waitForNoPendingRequests(page: Page): Promise<void> {
  // MSW tracks pending requests internally. We expose a hook via
  // window.__MSW__ when VITE_USE_MOCKS=true.
  await page.waitForFunction(
    () => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const msw = (window as any).__MSW__;
      if (!msw) return true;
      return msw.requests?.size === 0;
    },
    undefined,
    { timeout: 5_000 },
  );
}

/**
 * Seed the MSW workspace fixture with a known set of entry points.
 *
 * The MSW handlers in `apps/explorer-ui/src/mocks/handlers.ts` read from
 * a fixture file. This helper triggers a state setup by hitting a
 * dedicated `/api/__test__/seed` endpoint that the MSW handlers listen to.
 */
export async function seedWorkspace(
  page: Page,
  opts: { nodes?: number; truncated?: boolean; entryPointCount?: number } = {},
): Promise<void> {
  await page.request.post("/api/__test__/seed", {
    data: {
      nodes: opts.nodes ?? 50,
      truncated: opts.truncated ?? false,
      entryPointCount: opts.entryPointCount ?? 3,
    },
  });
}
