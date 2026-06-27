/**
 * E2E: MSW fixture consistency.
 *
 * The MSW handler list (apps/explorer-ui/src/mocks/handlers.ts) is the
 * API contract for E2E tests. This spec verifies MSW is intercepting
 * requests when VITE_USE_MOCKS=true by exercising a UI flow that
 * requires the handlers (Spotter search).
 *
 * Per-handler coverage: each handler is exercised by at least one
 * existing E2E spec:
 *   - GET /api/health        — smoke.spec.ts
 *   - GET /api/workspaces    — landing.spec.ts
 *   - GET /api/spotter       — spotter-multifamily.spec.ts
 *   - GET /api/viewspecs     — view-tabs-coverage.spec.ts (when fixture extends)
 *   - GET /api/explorations  — landing-real-data.spec.ts
 *   - GET /api/quality-*     — landing.spec.ts (QualityOverview)
 *
 * This spec is the meta-test: it fails if MSW stops intercepting in CI,
 * which would silently break every other spec.
 */
import { test, expect } from "@playwright/test";
import { snapshot } from "./utils/screenshot";

test.describe("MSW fixture consistency", () => {
  test("MSW intercepts requests when VITE_USE_MOCKS=true", async ({ page }) => {
    // Smoke test: navigate, open Spotter, assert results render. If MSW
    // isn't intercepting, the Spotter would not return data and the
    // assertion would fail.
    await page.goto("/");
    await page.waitForTimeout(500);
    await page.keyboard.press("Meta+k");
    const spotter = page.getByTestId("spotter");
    await expect(spotter).toBeVisible();
    const input = page.getByTestId("spotter-input");
    await input.fill("build");
    const firstHit = page
      .getByTestId("spotter-results")
      .locator('[data-family="symbol"]')
      .first();
    await expect(firstHit).toBeVisible({ timeout: 5_000 });

    // The Spotter hit proves MSW intercepted GET /api/workspaces/.../spotter.
    // All other handlers are exercised by other E2E specs.

    await snapshot(page, "msw-fixture-consistency.png");
  });
});
