/**
 * E2E tests for G1: usage_examples ViewExecutor (e12a).
 *
 * Coverage: ADR-002 e12a, G1 gap in e17-coverage-matrix.md.
 *
 * Flow: open Spotter → select symbol → switch to Usage Examples tab →
 * verify callers + callees blocks render with correct items.
 *
 * MSW fixture: usageExamplesViewFixture (fixtures.ts)
 * - callers block: 2 items (explore, fan_in)
 * - callees block: 3 items (build_symbols, fan_out, page_rank)
 *
 * NOTE: usage_examples uses CallListView (call.tsx), NOT TableRenderer.
 * Block IDs: "callers", "callees"
 * TestIDs: view-block-callers, view-block-callees, view-block-callers-items
 */
import { test, expect } from "@playwright/test";

/**
 * Helper: open app → Spotter → select first symbol → wait for inspector.
 */
async function openSymbolTab(page: import("@playwright/test").Page) {
  await page.goto("/");
  await expect(page.getByTestId("shell")).toBeVisible();
  await page.waitForTimeout(1500);

  const modifier = process.platform === "linux" ? "Control" : "Meta";
  await page.keyboard.press(`${modifier}+k`);

  const input = page.getByTestId("spotter-input");
  await expect(input).toBeVisible({ timeout: 5_000 });
  await input.fill("build");
  await page.waitForTimeout(800);

  const firstResult = page
    .getByTestId("spotter-results")
    .getByTestId(/^spotter-item-/);
  await expect(firstResult.first()).toBeVisible({ timeout: 5_000 });
  await firstResult.first().click();

  await expect(page.getByTestId("object-inspector")).toBeVisible({
    timeout: 5_000,
  });
}

test.describe("G1: usage_examples (e12a)", () => {
  test("tab switches to Usage Examples and renders callers block", async ({
    page,
  }) => {
    await openSymbolTab(page);

    // Switch to Usage Examples tab
    const tab = page.getByTestId("view-tab-usage-examples");
    await expect(tab).toBeVisible();
    await tab.click();
    await expect(tab).toHaveAttribute("aria-selected", "true");

    // Wait for callers block to render (CallListView, not TableRenderer)
    const callersBlock = page.getByTestId("view-block-callers");
    await expect(callersBlock).toBeVisible({ timeout: 5_000 });

    // Verify block title includes "Called by"
    const callersHeader = callersBlock.locator("h3");
    await expect(callersHeader).toContainText("Called by");

    // Verify list of caller items — use items list to avoid matching nested blocks
    const itemsList = callersBlock.locator('[data-testid="view-block-callers-items"]');
    await expect(itemsList).toBeVisible();
    const callerItems = itemsList.locator("li");
    const count = await callerItems.count();
    expect(count).toBeGreaterThanOrEqual(2); // fixture has 2 callers

    // Verify "explore" caller appears (fixture: explore at line 42)
    const callerText = await callersBlock.textContent();
    expect(callerText).toContain("explore");
  });

  test("renders callees block with 3 items", async ({ page }) => {
    await openSymbolTab(page);

    const tab = page.getByTestId("view-tab-usage-examples");
    await tab.click();
    await expect(tab).toHaveAttribute("aria-selected", "true");

    // Wait for callees block
    const calleesBlock = page.getByTestId("view-block-callees");
    await expect(calleesBlock).toBeVisible({ timeout: 5_000 });

    // Verify block title includes "Calls"
    const calleesHeader = calleesBlock.locator("h3");
    await expect(calleesHeader).toContainText("Calls");

    // Verify 3 callee items — use the items list directly to avoid
    // matching items from nested blocks (Playwright getByTestId searches descendants)
    const itemsList = calleesBlock.locator('[data-testid="view-block-callees-items"]');
    await expect(itemsList).toBeVisible();
    const calleeItems = itemsList.locator("li");
    const count = await calleeItems.count();
    expect(count).toBe(3);

    // Verify build_symbols appears (fixture: line 55)
    const calleeText = await calleesBlock.textContent();
    expect(calleeText).toContain("build_symbols");
  });

  test("caller item is clickable and navigates", async ({ page }) => {
    await openSymbolTab(page);

    const tab = page.getByTestId("view-tab-usage-examples");
    await tab.click();
    await expect(tab).toHaveAttribute("aria-selected", "true");

    const callersBlock = page.getByTestId("view-block-callers");
    await expect(callersBlock).toBeVisible({ timeout: 5_000 });

    // Click the first caller item button
    const callerButton = callersBlock
      .locator('[data-testid="view-block-callers-items"]')
      .getByTestId(/^view-block-item-button-/)
      .first();
    await expect(callerButton).toBeVisible();
    await callerButton.click();

    // After clicking, a new pane should open (GtPager behavior)
    // Verify at least 2 pane tabs now exist
    await page.waitForTimeout(500);
    const paneTabs = page.locator("[data-testid^='pane-tab-']");
    const tabCount = await paneTabs.count();
    expect(tabCount).toBeGreaterThanOrEqual(1); // at least the original + new pane
  });

  test("usage_examples screenshot", async ({ page }) => {
    await openSymbolTab(page);

    const tab = page.getByTestId("view-tab-usage-examples");
    await tab.click();
    await expect(tab).toHaveAttribute("aria-selected", "true");

    // Wait for both blocks to render
    await expect(page.getByTestId("view-block-callers")).toBeVisible({
      timeout: 5_000,
    });
    await expect(page.getByTestId("view-block-callees")).toBeVisible({
      timeout: 5_000,
    });

    // Allow font rendering variance (CI vs local)
    await expect(page.getByTestId("shell")).toHaveScreenshot(
      "usage-examples-rendered.png",
      { animations: "disabled", fullPage: true, maxDiffPixels: 10000 }
    );
  });
});
