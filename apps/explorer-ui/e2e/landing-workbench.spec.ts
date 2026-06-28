/**
 * E2E tests for G7: LandingWorkbench (e18-1).
 *
 * Coverage: ADR-005, E18-1 milestone.
 *
 * Flow: open app → see landing workbench → 4 tabs render → click
 * entry point → Spotter opens.
 */
import { test, expect } from "@playwright/test";

test.describe("G7: LandingWorkbench (e18-1)", () => {
  test("renders 4 tabs with Start From as default", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    const workbench = page.getByTestId("landing-workbench");
    await expect(workbench).toBeVisible({ timeout: 5_000 });

    // 4 tabs visible
    await expect(page.getByTestId("landing-tab-start")).toBeVisible();
    await expect(page.getByTestId("landing-tab-investigations")).toBeVisible();
    await expect(page.getByTestId("landing-tab-resume")).toBeVisible();
    await expect(page.getByTestId("landing-tab-graph")).toBeVisible();

    // Default active tab is start
    await expect(workbench).toHaveAttribute("data-active-tab", "start");
    await expect(page.getByTestId("start-from-section")).toBeVisible();
  });

  test("clicking an entry point opens Spotter", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });
    await expect(page.getByTestId("landing-workbench")).toBeVisible({ timeout: 5_000 });

    // Click a Route entry point
    await page.getByTestId("entry-point-route").click();

    // Spotter opens
    const spotter = page.getByTestId("spotter");
    await expect(spotter).toBeVisible({ timeout: 5_000 });
  });

  test("switching to Graph tab shows GraphLanding", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });
    await expect(page.getByTestId("landing-workbench")).toBeVisible({ timeout: 5_000 });

    // Click Graph tab
    await page.getByTestId("landing-tab-graph").click();

    // Graph tab content (the graph canvas) is visible
    await expect(page.getByTestId("graph-landing-canvas")).toBeVisible({ timeout: 5_000 });
  });

  test("switching to Investigations tab shows investigation templates", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });
    await expect(page.getByTestId("landing-workbench")).toBeVisible({ timeout: 5_000 });

    await page.getByTestId("landing-tab-investigations").click();
    await expect(page.getByTestId("investigations-section")).toBeVisible();

    // At least one template visible
    await expect(page.getByTestId("investigation-template-trace-request")).toBeVisible();
  });
});
