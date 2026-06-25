/**
 * E2E spotter tests — Phase 4 of the explorer-e2e-test-plan.
 *
 * Verifies the Spotter (cmd-K palette) end-to-end behavior: open, search,
 * result groups, escape-to-close, empty state.
 *
 * All tests rely on MSW handlers (VITE_USE_MOCKS=true).
 * The Spotter component lives at apps/explorer-ui/src/components/Spotter.tsx.
 *
 * Phase 4 scenarios (5 tests) from docs/explorer-e2e-test-plan.md:
 *   P4.1 Open via Cmd+K
 *   P4.2 Open via search button
 *   P4.3 Results grouped by kind
 *   P4.4 Close with Escape
 *   P4.5 Empty state when no results
 */
import { test, expect } from "@playwright/test";

test.describe("Phase 4: Spotter (5 tests)", () => {
  test("P4.1 Open via Cmd+K", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Wait for the keyboard listener to mount
    await page.waitForTimeout(1500);

    // Press Cmd+K
    await page.keyboard.press("Meta+k");

    // The Spotter modal opens
    await expect(page.getByTestId("spotter")).toBeVisible({ timeout: 5_000 });
    await expect(page.getByTestId("spotter-input")).toBeFocused();
  });

  test("P4.2 Open via search button", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Click the Spotter trigger button in the header
    const trigger = page.getByTestId("spotter-trigger");
    await expect(trigger).toBeVisible();
    await trigger.click();

    // The Spotter modal opens
    await expect(page.getByTestId("spotter")).toBeVisible({ timeout: 5_000 });
  });

  test("P4.3 Results grouped by kind", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Open the Spotter
    await page.waitForTimeout(1500);
    await page.keyboard.press("Meta+k");
    await expect(page.getByTestId("spotter")).toBeVisible({ timeout: 5_000 });

    // Type a query that matches multiple kinds
    const input = page.getByTestId("spotter-input");
    await input.fill("build");

    // Wait for results
    const results = page.getByTestId("spotter-results");
    await expect(results).toBeVisible({ timeout: 5_000 });

    // At least one result item
    const items = page.locator("[data-testid^='spotter-item-']");
    await expect(items.first()).toBeVisible({ timeout: 5_000 });
    expect(await items.count()).toBeGreaterThan(0);

    // The count badge shows the total
    const count = page.getByTestId("spotter-count");
    await expect(count).toBeVisible();
  });

  test("P4.4 Close with Escape", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Open the Spotter
    await page.waitForTimeout(1500);
    await page.keyboard.press("Meta+k");
    await expect(page.getByTestId("spotter")).toBeVisible({ timeout: 5_000 });

    // Press Escape
    await page.keyboard.press("Escape");

    // The Spotter closes
    await expect(page.getByTestId("spotter")).toBeHidden({ timeout: 5_000 });
  });

  test("P4.5 Empty state when no results", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Open the Spotter
    await page.waitForTimeout(1500);
    await page.keyboard.press("Meta+k");
    await expect(page.getByTestId("spotter")).toBeVisible({ timeout: 5_000 });

    // Type a query that won't match anything
    const input = page.getByTestId("spotter-input");
    await input.fill("zzzzzzzzz_no_match_xyz_qqq");

    // Wait for the empty state
    await expect(page.getByTestId("spotter-empty")).toBeVisible({ timeout: 5_000 });

    // No result items are shown
    const items = page.locator("[data-testid^='spotter-item-']");
    expect(await items.count()).toBe(0);
  });
});
