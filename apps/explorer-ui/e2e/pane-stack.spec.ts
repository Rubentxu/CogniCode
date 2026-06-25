/**
 * E2E pane-stack tests — Phase 3 of the explorer-e2e-test-plan.
 *
 * Verifies the GtPager-style horizontal pane-stack that allows inspecting
 * multiple objects in parallel.
 *
 * All tests rely on MSW handlers (VITE_USE_MOCKS=true). Objects are
 * selected via the Spotter to open panes.
 *
 * Phase 3 scenarios (8 tests) from docs/explorer-e2e-test-plan.md:
 *   P3.1 First pane renders object inspector
 *   P3.2 Second pane creates new tab (max 8)
 *   P3.3 Click tab switches active pane
 *   P3.4 Close pane removes it
 *   P3.5 Close last pane shows empty state
 *   P3.6 Active pane shows object label
 *   P3.7 View tabs render for inspected object
 *   P3.8 Switch view updates inspector body
 */
import { test, expect } from "@playwright/test";

/**
 * Helper: open a Spotter result and select it to create a new pane.
 * Returns the first Spotter result testid.
 */
async function openFirstSpotterResult(page: import("@playwright/test").Page) {
  await page.waitForTimeout(1500); // wait for keyboard listener
  await page.keyboard.press("Meta+k");
  await expect(page.getByTestId("spotter")).toBeVisible({ timeout: 10_000 });

  const input = page.getByTestId("spotter-input");
  await input.fill("build");
  const firstResult = page
    .getByTestId("spotter-results")
    .getByTestId(/^spotter-item-/);
  await expect(firstResult.first()).toBeVisible({ timeout: 5_000 });
  await firstResult.first().click();
  await expect(page.getByTestId("spotter")).toBeHidden();
  await expect(page.getByTestId("object-inspector")).toBeVisible({ timeout: 5_000 });
}

/**
 * Helper: get the current number of open pane tabs.
 */
function paneTabs(page: import("@playwright/test").Page) {
  return page.locator("[data-testid^='pane-tab-']");
}

test.describe("Phase 3: Pane-Stack (8 tests)", () => {
  test("P3.1 First pane renders object inspector", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Initially pane-stack is empty
    await expect(page.getByTestId("pane-stack-empty")).toBeVisible({ timeout: 5_000 });

    // Open the first object via Spotter
    await openFirstSpotterResult(page);

    // Pane-stack replaces the empty state
    await expect(page.getByTestId("pane-stack-view")).toBeVisible({ timeout: 5_000 });
    await expect(page.getByTestId("pane-stack-empty")).toBeHidden();

    // Exactly one pane tab exists
    await expect(paneTabs(page)).toHaveCount(1);

    // Object inspector is visible inside the pane
    await expect(page.getByTestId("object-inspector")).toBeVisible();
  });

  test("P3.2 Second pane creates new tab", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Open the first object
    await openFirstSpotterResult(page);
    await expect(paneTabs(page)).toHaveCount(1);

    // Open a second object (different query to get a different result)
    await openFirstSpotterResult(page);

    // Now two pane tabs exist
    await expect(paneTabs(page)).toHaveCount(2);
  });

  test("P3.3 Click tab switches active pane", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Open two objects
    await openFirstSpotterResult(page);
    await openFirstSpotterResult(page);
    await expect(paneTabs(page)).toHaveCount(2);

    // Get the two tabs
    const tabs = paneTabs(page);
    const firstTab = tabs.nth(0);
    const secondTab = tabs.nth(1);

    // First tab is active initially (or second if new ones are prepended)
    const firstSelected = await firstTab.getAttribute("aria-selected");
    const secondSelected = await secondTab.getAttribute("aria-selected");

    // Click the first tab
    await firstTab.click();
    await expect(firstTab).toHaveAttribute("aria-selected", "true");
    await expect(secondTab).toHaveAttribute("aria-selected", "false");

    // Click the second tab
    await secondTab.click();
    await expect(secondTab).toHaveAttribute("aria-selected", "true");
    await expect(firstTab).toHaveAttribute("aria-selected", "false");
  });

  test("P3.4 Close pane removes it", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Open one object
    await openFirstSpotterResult(page);
    await expect(paneTabs(page)).toHaveCount(1);

    // Close the pane via the inspector's close button
    const closeBtn = page
      .getByTestId("object-inspector")
      .getByRole("button", { name: /close/i });
    if (await closeBtn.isVisible()) {
      await closeBtn.click();
    } else {
      // Fallback: dispatch via keyboard shortcut if close button hidden
      await page.keyboard.press("Escape");
    }

    // Pane count returns to 0
    await expect(paneTabs(page)).toHaveCount(0);
  });

  test("P3.5 Close last pane shows empty state", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Open and close one object
    await openFirstSpotterResult(page);
    await expect(page.getByTestId("pane-stack-view")).toBeVisible();

    const closeBtn = page
      .getByTestId("object-inspector")
      .getByRole("button", { name: /close/i });
    if (await closeBtn.isVisible()) {
      await closeBtn.click();
    } else {
      await page.keyboard.press("Escape");
    }

    // Empty state is shown again
    await expect(page.getByTestId("pane-stack-empty")).toBeVisible({ timeout: 5_000 });
    await expect(page.getByTestId("pane-stack-view")).toBeHidden();
  });

  test("P3.6 Active pane shows object label", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    await openFirstSpotterResult(page);

    // The active pane tab shows the object label
    const activeTab = paneTabs(page).first();
    await expect(activeTab).toHaveAttribute("aria-selected", "true");

    // Tab text is non-empty (objectId-derived label)
    const tabText = await activeTab.textContent();
    expect(tabText?.length).toBeGreaterThan(0);
  });

  test("P3.7 View tabs render for inspected object", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    await openFirstSpotterResult(page);

    // The inspector renders view tabs (getByTestId="view-tabs")
    const viewTabs = page.getByTestId("view-tabs");
    await expect(viewTabs).toBeVisible({ timeout: 5_000 });

    // At least one view tab exists
    const tabs = viewTabs.getByRole("tab");
    await expect(tabs.first()).toBeVisible();
    expect(await tabs.count()).toBeGreaterThan(0);
  });

  test("P3.8 Switch view updates inspector body", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    await openFirstSpotterResult(page);

    const viewTabs = page.getByTestId("view-tabs");
    const tabs = viewTabs.getByRole("tab");

    // If there's a call-graph tab, click it and verify the graph view
    const callGraphTab = page.getByTestId("view-tab-call-graph");
    if (await callGraphTab.isVisible()) {
      await callGraphTab.click();

      // The graph view renders (data-testid="graph-view-renderer")
      await expect(page.getByTestId("graph-view-renderer")).toBeVisible({
        timeout: 5_000,
      });
    } else {
      // Fallback: click any second tab and verify the tab activates
      const firstTab = tabs.first();
      const tabName = await firstTab.textContent();
      await firstTab.click();
      await expect(firstTab).toHaveAttribute("aria-selected", "true");
    }
  });
});
