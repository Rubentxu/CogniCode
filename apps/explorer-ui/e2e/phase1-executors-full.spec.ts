/**
 * E2E tests for Phase 1 executors G2–G5 (e12b–e12e).
 *
 * Coverage: ADR-002 e12b–e12e, gaps in e17-coverage-matrix.md.
 *
 * Each view is reached by: open Spotter → select object →
 * switch to the view tab → verify content renders.
 *
 * Block IDs and fixtures:
 * - G2 (api_surface):  block ID "symbols"      → FileSymbolsView (registered)
 * - G3 (test_slice):    block ID "callers"      → CallListView (registered)
 * - G4 (debug_slice):   block IDs "callers/callees" → CallListView (registered)
 * - G5 (change_impact): block IDs "callers/callees" → CallListView (registered)
 */
import { test, expect } from "@playwright/test";

// =============================================================================
// Helpers
// =============================================================================

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

async function openScopeTab(page: import("@playwright/test").Page) {
  await page.goto("/");
  await expect(page.getByTestId("shell")).toBeVisible();
  await page.waitForTimeout(1500);

  const modifier = process.platform === "linux" ? "Control" : "Meta";
  await page.keyboard.press(`${modifier}+k`);

  const input = page.getByTestId("spotter-input");
  await expect(input).toBeVisible({ timeout: 5_000 });
  await input.fill("cogni");
  await page.waitForTimeout(800);

  const scopeResult = page
    .getByTestId("spotter-results")
    .getByTestId(/^spotter-item-scope:/);
  await expect(scopeResult).toBeVisible({ timeout: 5_000 });
  await scopeResult.click();

  await expect(page.getByTestId("object-inspector")).toBeVisible({
    timeout: 5_000,
  });
}

// =============================================================================
// G2: api_surface (e12b) — Scope view, renders via FileSymbolsView
// =============================================================================

test.describe("G2: api_surface (e12b)", () => {
  test("renders symbols list via FileSymbolsView", async ({ page }) => {
    await openScopeTab(page);

    // Switch to API Surface tab
    const tab = page.getByTestId("view-tab-api-surface");
    await expect(tab).toBeVisible();
    await tab.click();
    await expect(tab).toHaveAttribute("aria-selected", "true");

    // FileSymbolsView renders block with data-testid="view-block-symbols"
    const symbolsBlock = page.getByTestId("view-block-symbols");
    await expect(symbolsBlock).toBeVisible({ timeout: 5_000 });

    // Verify block title contains the scope path
    const header = symbolsBlock.locator("h3");
    await expect(header).toContainText("cognicode-explorer/src");

    // Verify at least 6 symbol items render (fixture has 6)
    const items = symbolsBlock.locator("li");
    const count = await items.count();
    expect(count).toBeGreaterThanOrEqual(6);

    // Verify specific symbols appear (build_overview, CommunityDetection)
    const blockText = await symbolsBlock.textContent();
    expect(blockText).toContain("build_overview");
    expect(blockText).toContain("CommunityDetection");
  });

  test("api_surface screenshot", async ({ page }) => {
    await openScopeTab(page);

    const tab = page.getByTestId("view-tab-api-surface");
    await tab.click();
    await expect(tab).toHaveAttribute("aria-selected", "true");
    await expect(page.getByTestId("view-block-symbols")).toBeVisible({
      timeout: 5_000,
    });

    await expect(page.getByTestId("shell")).toHaveScreenshot(
      "api-surface-rendered.png",
      { animations: "disabled", fullPage: true, maxDiffPixels: 10000 }
    );
  });
});

// =============================================================================
// G3: test_slice (e12c) — Symbol view, renders via CallListView (callers)
// =============================================================================

test.describe("G3: test_slice (e12c)", () => {
  test("renders test list via CallListView", async ({ page }) => {
    await openSymbolTab(page);

    const tab = page.getByTestId("view-tab-test-slice");
    await expect(tab).toBeVisible();
    await tab.click();
    await expect(tab).toHaveAttribute("aria-selected", "true");

    // CallListView renders block with data-testid="view-block-callers"
    const testBlock = page.getByTestId("view-block-callers");
    await expect(testBlock).toBeVisible({ timeout: 5_000 });

    // Verify title "Tests (2)"
    const header = testBlock.locator("h3");
    await expect(header).toContainText("Tests");

    // Verify 2 test items (fixture: test_build_overview, test_overview_fan_in)
    const itemsList = testBlock.locator(
      '[data-testid="view-block-callers-items"]'
    );
    await expect(itemsList).toBeVisible();
    const items = itemsList.locator("li");
    expect(await items.count()).toBe(2);

    // Verify specific test names
    const blockText = await testBlock.textContent();
    expect(blockText).toContain("test_build_overview");
    expect(blockText).toContain("test_overview_fan_in");
  });

  test("test_slice screenshot", async ({ page }) => {
    await openSymbolTab(page);

    const tab = page.getByTestId("view-tab-test-slice");
    await tab.click();
    await expect(tab).toHaveAttribute("aria-selected", "true");
    await expect(page.getByTestId("view-block-callers")).toBeVisible({
      timeout: 5_000,
    });

    await expect(page.getByTestId("shell")).toHaveScreenshot(
      "test-slice-rendered.png",
      { animations: "disabled", fullPage: true, maxDiffPixels: 10000 }
    );
  });
});

// =============================================================================
// G4: debug_slice (e12d) — Symbol view, renders via CallListView
// =============================================================================

test.describe("G4: debug_slice (e12d)", () => {
  test("renders debug callers and callees via CallListView", async ({
    page,
  }) => {
    await openSymbolTab(page);

    const tab = page.getByTestId("view-tab-debug-slice");
    await expect(tab).toBeVisible();
    await tab.click();
    await expect(tab).toHaveAttribute("aria-selected", "true");

    // Debug callers block
    const callersBlock = page.getByTestId("view-block-callers");
    await expect(callersBlock).toBeVisible({ timeout: 5_000 });
    await expect(callersBlock.locator("h3")).toContainText("Debug Callers");

    // 1 debug caller: log_debug
    const callerItems = callersBlock
      .locator('[data-testid="view-block-callers-items"]')
      .locator("li");
    expect(await callerItems.count()).toBe(1);
    const callersText = await callersBlock.textContent();
    expect(callersText).toContain("log_debug");

    // Debug callees block
    const calleesBlock = page.getByTestId("view-block-callees");
    await expect(calleesBlock).toBeVisible({ timeout: 5_000 });
    await expect(calleesBlock.locator("h3")).toContainText("Debug Callees");

    // 2 debug callees: assert_eq, dbg_trace
    const calleeItems = calleesBlock
      .locator('[data-testid="view-block-callees-items"]')
      .locator("li");
    expect(await calleeItems.count()).toBe(2);
    const calleesText = await calleesBlock.textContent();
    expect(calleesText).toContain("assert_eq");
    expect(calleesText).toContain("dbg_trace");
  });

  test("debug_slice screenshot", async ({ page }) => {
    await openSymbolTab(page);

    const tab = page.getByTestId("view-tab-debug-slice");
    await tab.click();
    await expect(tab).toHaveAttribute("aria-selected", "true");
    await expect(page.getByTestId("view-block-callers")).toBeVisible({
      timeout: 5_000,
    });

    await expect(page.getByTestId("shell")).toHaveScreenshot(
      "debug-slice-rendered.png",
      { animations: "disabled", fullPage: true, maxDiffPixels: 10000 }
    );
  });
});

// =============================================================================
// G5: change_impact_story (e12e) — Symbol view, renders via CallListView
// =============================================================================

test.describe("G5: change_impact_story (e12e)", () => {
  test("renders upstream and downstream impact via CallListView", async ({
    page,
  }) => {
    await openSymbolTab(page);

    const tab = page.getByTestId("view-tab-change-impact-story");
    await expect(tab).toBeVisible();
    await tab.click();
    await expect(tab).toHaveAttribute("aria-selected", "true");

    // Upstream block (callers): 3 items
    const upstreamBlock = page.getByTestId("view-block-callers");
    await expect(upstreamBlock).toBeVisible({ timeout: 5_000 });
    await expect(upstreamBlock.locator("h3")).toContainText("Upstream");

    const upstreamItems = upstreamBlock
      .locator('[data-testid="view-block-callers-items"]')
      .locator("li");
    expect(await upstreamItems.count()).toBe(3);
    const upstreamText = await upstreamBlock.textContent();
    expect(upstreamText).toContain("explore");
    expect(upstreamText).toContain("fan_in");

    // Downstream block (callees): 3 items
    const downstreamBlock = page.getByTestId("view-block-callees");
    await expect(downstreamBlock).toBeVisible({ timeout: 5_000 });
    await expect(downstreamBlock.locator("h3")).toContainText("Downstream");

    const downstreamItems = downstreamBlock
      .locator('[data-testid="view-block-callees-items"]')
      .locator("li");
    expect(await downstreamItems.count()).toBe(3);
    const downstreamText = await downstreamBlock.textContent();
    expect(downstreamText).toContain("build_symbols");
    expect(downstreamText).toContain("page_rank");
  });

  test("change_impact_story screenshot", async ({ page }) => {
    await openSymbolTab(page);

    const tab = page.getByTestId("view-tab-change-impact-story");
    await tab.click();
    await expect(tab).toHaveAttribute("aria-selected", "true");
    await expect(page.getByTestId("view-block-callers")).toBeVisible({
      timeout: 5_000,
    });

    await expect(page.getByTestId("shell")).toHaveScreenshot(
      "change-impact-story-rendered.png",
      { animations: "disabled", fullPage: true, maxDiffPixels: 10000 }
    );
  });
});
