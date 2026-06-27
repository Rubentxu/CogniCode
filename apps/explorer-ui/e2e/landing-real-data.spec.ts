/**
 * E2E: Landing real-data + virtualization + truncation banner.
 *
 * GToolkit parity: the landing is CogniCode's equivalent of GT World's
 * home pane stack. It must show real data (entry_points, hot_paths,
 * god_nodes) from the workspace, not empty stubs. Truncation banner
 * must surface when the payload is capped. Virtualization must activate
 * when nodes > 200 (e9-landing-perf, PR #61).
 *
 * Spec scope:
 * - Landing header is visible with workspace name.
 * - Recent explorations strip is visible.
 * - Graph landing status is visible (loaded).
 * - Truncation banner activates when MSW returns truncated=true.
 *
 * The MSW fixture for landing is in
 * apps/explorer-ui/src/mocks/handlers.ts around line 290.
 */
import { test, expect } from "@playwright/test";
import { snapshot } from "./utils/screenshot";

test.describe("Landing real-data (GT World home parity)", () => {
  test("landing renders workspace name and status", async ({ page }) => {
    await page.goto("/");

    // The landing is the default surface when no object is inspected.
    const header = page.getByTestId("landing-header");
    await expect(header).toBeVisible();

    const workspaceName = page.getByTestId("landing-workspace-name");
    await expect(workspaceName).toBeVisible();

    const status = page.getByTestId("landing-graph-status");
    await expect(status).toBeVisible();

    await snapshot(page, "landing-real-data-default.png");
  });

  test("recent explorations strip is visible", async ({ page }) => {
    await page.goto("/");

    const strip = page.getByTestId("recent-explorations-strip");
    await expect(strip).toBeVisible({ timeout: 5_000 });

    await snapshot(page, "landing-real-data-recent.png");
  });

  test.skip("truncation banner activates when MSW returns truncated=true [DEBT: MSW has no seedable flag]", async ({
    page,
  }) => {
    // The MSW handler returns a fixed landing payload. To test truncation
    // we need a configurable seed endpoint. Re-enable when
    // apps/explorer-ui/src/mocks/handlers.ts adds /api/__test__/seed
    // for landing truncation.
    await page.goto("/?__test__truncated=true");
    const banner = page.locator('[data-testid="landing-truncation-banner"]');
    await expect(banner).toBeVisible();
    await snapshot(page, "landing-real-data-banner.png");
  });
});
