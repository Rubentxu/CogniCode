/**
 * E2E pane-stack tests — multi-pane inspection, tab switching, and close.
 *
 * Covers Phase 3 of the explorer E2E test battery:
 *  - P3.1  First pane renders object inspector
 *  - P3.2  Second pane creates new tab
 *  - P3.3  Click tab switches active pane
 *  - P3.4  Close pane removes it
 *  - P3.5  Close last pane shows empty state
 *  - P3.7  View tabs render for inspected object
 *  - P3.8  Switch view updates inspector body
 *
 * VISUAL VALIDATION: All tests capture screenshots for regression testing.
 *
 * The MSW browser worker provides deterministic fixtures:
 * - Spotter results: "build_overview" + "build_callgraph" (two distinct symbols)
 * - Each symbol has 4 views: overview, call-graph, source, quality
 */
import { test, expect } from "@playwright/test";

async function openSpotterAndSelect(page: import("@playwright/test").Page, query: string, resultIndex = 0) {
  // Wait 1500ms for keyboard listener to mount
  await page.waitForTimeout(1500);
  await page.keyboard.press("Meta+k");
  const input = page.getByTestId("spotter-input");
  await input.fill(query);
  const results = page.getByTestId("spotter-results").getByTestId(/^spotter-item-/);
  await expect(results.nth(resultIndex)).toBeVisible({ timeout: 5_000 });
  await results.nth(resultIndex).click();
  await expect(page.getByTestId("object-inspector")).toBeVisible({ timeout: 5_000 });
}

// The spotter fixture returns 2 results for "build": build_overview (index 0) and build_callgraph (index 1)
const SPOTTER_QUERY = "build";
const FIRST_RESULT_INDEX = 0;
const SECOND_RESULT_INDEX = 1;

test.describe("Explorer pane-stack flows", () => {
  test.use({ screenshot: "on" });

  test("P3.1 — first pane renders object inspector", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible();

    // Open first object via Spotter
    await openSpotterAndSelect(page, "build");

    // Verify inspector is shown with the object's label
    const inspectorHeader = page.locator("[data-testid='object-inspector'] h2");
    await expect(inspectorHeader).toBeVisible();
    // The label may be "(loading)" briefly before the object resolves
    await expect(inspectorHeader).not.toHaveText("(loading)");

    // VISUAL VALIDATION: Capture first pane rendering
    await expect(page.getByTestId("shell")).toHaveScreenshot("panestack-first-pane.png", {
      animations: "disabled",
      fullPage: true,
    });
  });

  test("P3.2 — second pane creates a second tab", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible();

    // Open first pane
    await openSpotterAndSelect(page, SPOTTER_QUERY, FIRST_RESULT_INDEX);

    // Open second pane — use second spotter result (build_callgraph)
    await openSpotterAndSelect(page, SPOTTER_QUERY, SECOND_RESULT_INDEX);

    // Should now have 2 pane tabs
    const tabs = page.locator("[data-testid^='pane-tab-']");
    await expect(tabs).toHaveCount(2);

    // VISUAL VALIDATION: Capture two-pane layout
    await expect(page.getByTestId("shell")).toHaveScreenshot("panestack-two-panes.png", {
      animations: "disabled",
      fullPage: true,
    });
  });

  test("P3.3 — click tab switches active pane", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible();

    // Open first pane (build_overview)
    await openSpotterAndSelect(page, SPOTTER_QUERY, FIRST_RESULT_INDEX);

    // Open second pane (build_callgraph)
    await openSpotterAndSelect(page, SPOTTER_QUERY, SECOND_RESULT_INDEX);

    // Both tabs visible
    const tabs = page.locator("[data-testid^='pane-tab-']");
    await expect(tabs).toHaveCount(2);

    // The second pane is active (most recently opened)
    // Its tab has aria-selected="true"
    const secondTab = tabs.last();
    await expect(secondTab).toHaveAttribute("aria-selected", "true");

    // Click the first tab to switch back
    await tabs.first().click();

    // Verify the first tab is now active
    await expect(tabs.first()).toHaveAttribute("aria-selected", "true");

    // After switching, the active pane's body should still be visible
    // Use .last() since the previously-active (second) pane is still in DOM
    await expect(page.getByTestId("object-inspector-body").last()).toBeVisible();

    // VISUAL VALIDATION: Capture tab switching
    await expect(page.getByTestId("shell")).toHaveScreenshot("panestack-tab-switching.png", {
      animations: "disabled",
      fullPage: true,
    });
  });

  test("P3.4 — close pane removes it", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible();

    // Open two panes
    await openSpotterAndSelect(page, SPOTTER_QUERY, FIRST_RESULT_INDEX);
    await openSpotterAndSelect(page, SPOTTER_QUERY, SECOND_RESULT_INDEX);

    // Confirm 2 tabs
    const tabs = page.locator("[data-testid^='pane-tab-']");
    await expect(tabs).toHaveCount(2);

    // Close the active (second) pane via the ✕ button
    // The active pane is rendered last in the DOM, so .last() targets it
    await page.getByTestId("pane-close").last().click();

    // Should now have 1 tab
    await expect(tabs).toHaveCount(1);
    // Inspector should still be visible (first pane is still active)
    await expect(page.getByTestId("object-inspector")).toBeVisible();

    // VISUAL VALIDATION: Capture after pane close
    await expect(page.getByTestId("shell")).toHaveScreenshot("panestack-after-close.png", {
      animations: "disabled",
      fullPage: true,
    });
  });

  test("P3.5 — close last pane shows empty state", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible();

    // Open exactly one pane
    await openSpotterAndSelect(page, "build");

    // Verify inspector is visible
    await expect(page.getByTestId("object-inspector")).toBeVisible();

    // Close the pane
    const closeBtn = page.getByTestId("pane-close");
    await closeBtn.click();

    // Empty state should appear
    const emptyState = page.getByTestId("pane-stack-empty");
    await expect(emptyState).toBeVisible();
    await expect(emptyState).toContainText("No panes open");

    // VISUAL VALIDATION: Capture empty state
    await expect(page.getByTestId("shell")).toHaveScreenshot("panestack-empty-state.png", {
      animations: "disabled",
      fullPage: true,
    });
  });

  test("P3.7 + P3.8 — view tabs render and switching updates body", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible();

    // Open pane — the inspectable fixture has 4 views: overview, call-graph, source, quality
    await openSpotterAndSelect(page, "build");

    // View tabs should be present
    const tablist = page.getByRole("tablist", { name: /Available views/i });
    await expect(tablist).toBeVisible();
    const tabs = tablist.getByRole("tab");
    const tabCount = await tabs.count();
    expect(tabCount).toBeGreaterThanOrEqual(2);

    // Inspector body is visible
    const body = page.getByTestId("object-inspector-body");
    await expect(body).toBeVisible();

    // VISUAL VALIDATION: Capture view tabs
    await expect(page.getByTestId("shell")).toHaveScreenshot("panestack-view-tabs.png", {
      animations: "disabled",
      fullPage: true,
    });

    // Click the "Call graph" tab if available
    const callGraphTab = page.getByTestId("view-tab-call-graph");
    if (await callGraphTab.isVisible()) {
      await callGraphTab.click();
      // Body should still be present after tab switch
      await expect(body).toBeVisible();
    }

    // Click the "Source" tab if available
    const sourceTab = page.getByTestId("view-tab-source");
    if (await sourceTab.isVisible()) {
      await sourceTab.click();
      await expect(body).toBeVisible();
    }
  });

  test("P3.6 — active pane shows object label in tab", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible();

    // Open pane with "build_overview"
    await openSpotterAndSelect(page, "build");

    // The active tab should contain a fragment of the object label
    const tabs = page.locator("[data-testid^='pane-tab-']");
    await expect(tabs).toHaveCount(1);
    // Tab title is the truncated objectId; "build_overview:16" last segment is "16"
    await expect(tabs.first()).toContainText("16");

    // VISUAL VALIDATION: Capture tab with object label
    await expect(page.getByTestId("shell")).toHaveScreenshot("panestack-object-label-in-tab.png", {
      animations: "disabled",
      fullPage: true,
    });
  });

  test("P3.2 variant — opening same object twice activates existing pane (not duplicate)", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible();

    // Open build_overview (first spotter result)
    await openSpotterAndSelect(page, SPOTTER_QUERY, FIRST_RESULT_INDEX);

    // Try to open the same object again (same query + same result index)
    await openSpotterAndSelect(page, SPOTTER_QUERY, FIRST_RESULT_INDEX);

    // Should still be exactly 1 tab (re-uses existing pane, does not duplicate)
    const tabs = page.locator("[data-testid^='pane-tab-']");
    await expect(tabs).toHaveCount(1);

    // VISUAL VALIDATION: Capture deduplication behavior
    await expect(page.getByTestId("shell")).toHaveScreenshot("panestack-no-duplicate-panes.png", {
      animations: "disabled",
      fullPage: true,
    });
  });
});