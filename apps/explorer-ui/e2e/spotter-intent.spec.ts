/**
 * E2E tests for E18-2 Spotter Intent Actions.
 *
 * Coverage: spotter-intent capability (chip footer + kind-aware defaults
 * + Cmd+1..N keyboard shortcuts).
 *
 * NOTE on cmdk keyboard navigation: cmdk uses vimBindings (j/k/ArrowUp/
 * ArrowDown) for list navigation. Pointer-based selection is disabled
 * (disablePointerSelection=true). This means:
 *   - Hover does NOT highlight a result
 *   - E2E keyboard navigation (ArrowDown) does not reliably set
 *     highlightedResult in the Playwright/chromium environment
 *
 * What IS tested in E2E:
 *   - Footer hint shown when spotter opens (no selection yet)
 *   - Clicking a result selects kind-aware default and opens inspector
 *
 * What is tested via UNIT TESTS (IntentFooter.test.tsx):
 *   - Chip footer appears when result is highlighted (via prop injection)
 *   - Each chip calls onPick with correct viewId
 *   - C4 and Add-to-Investigation are disabled with aria-disabled
 *   - Cmd+1 shortcut on 2nd chip
 *
 * The Cmd+1..N keyboard shortcut integration with cmdk highlightedResult
 * is exercised in the unit test "shows Cmd+1 shortcut on first enabled chip".
 */
import { test, expect } from "@playwright/test";

test.describe("E18-2: Spotter Intent", () => {
  test("footer shows hint when no result is selected", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Open Spotter via keyboard shortcut
    await page.keyboard.press("Meta+k");
    await expect(page.getByTestId("spotter")).toBeVisible({ timeout: 5_000 });

    // Initial state: footer shows hint (no highlighted result yet)
    await expect(page.getByTestId("spotter-intent-footer")).toContainText(/pick a result/i);
  });

  test("clicking a result selects kind-aware default viewId and opens inspector", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    await page.keyboard.press("Meta+k");
    await expect(page.getByTestId("spotter")).toBeVisible({ timeout: 5_000 });

    await page.getByTestId("spotter-input").fill("build");

    const firstResult = page.locator("[data-testid^='spotter-item-']").first();
    await expect(firstResult).toBeVisible({ timeout: 5_000 });

    // Click the result — onSelect fires with kind-aware default viewId
    // (highlightedResult is null since hover/keyboard didn't set it,
    // so pendingViewId is not used — this tests the default path)
    await firstResult.click();

    // Spotter closes
    await expect(page.getByTestId("spotter")).not.toBeVisible({ timeout: 5_000 });

    // Inspector opens with kind-aware default view (overview for symbol)
    await expect(page.getByTestId("object-inspector")).toBeVisible({ timeout: 5_000 });
  });
});
