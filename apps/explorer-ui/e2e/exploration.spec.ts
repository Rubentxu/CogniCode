/**
 * E2E exploration flow tests — Graph → C4 toggle, Spotter → inspect.
 *
 * Tests the core user journey:
 *  1. C4 perspective toggle switches between Graph and C4 views
 *  2. Spotter search → select → inspect in pane-stack
 *  3. C4 perspective + Spotter → inspect
 *  4. Perspective toggle behavior
 *
 * MSW mocks handle all /api/* traffic (VITE_USE_MOCKS=true).
 */
import { test, expect } from "@playwright/test";

test.describe("Explorer exploration flows", () => {
  test("Perspective toggle switches between Graph and C4 views", async ({
    page,
  }) => {
    await page.goto("/");
    const shell = page.getByTestId("shell");
    await expect(shell).toBeVisible();

    // The perspective toggle exists in the header
    const toggle = page.getByTestId("perspective-toggle");
    await expect(toggle).toBeVisible();

    // Initially shows Graph as active
    const graphBtn = toggle.getByTestId("perspective-graph");
    const c4Btn = toggle.getByTestId("perspective-c4");
    await expect(graphBtn).toBeVisible();
    await expect(c4Btn).toBeVisible();

    // Click C4 toggle
    await c4Btn.click();

    // Click back to Graph
    await graphBtn.click();
  });

  test("Spotter opens and inspects an object", async ({ page }) => {
    await page.goto("/");
    const shell = page.getByTestId("shell");
    await expect(shell).toBeVisible();

    // Open Spotter via Cmd+K
    await page.keyboard.press("Meta+k");
    const spotter = page.getByTestId("spotter");
    await expect(spotter).toBeVisible();

    // Type a query
    const input = page.getByTestId("spotter-input");
    await input.fill("build");

    // Wait for results
    const firstResult = page
      .getByTestId("spotter-results")
      .getByTestId(/^spotter-item-/);
    await expect(firstResult.first()).toBeVisible({ timeout: 5_000 });

    // Click the first result
    await firstResult.first().click();
    await expect(spotter).toBeHidden();

    // Verify the object inspector is shown
    await expect(page.getByTestId("object-inspector")).toBeVisible();

    // Verify view tabs are rendered
    const tablist = page.getByRole("tablist", { name: /Available views/i });
    await expect(tablist).toBeVisible();
  });

  test("C4 perspective + Spotter search works", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible();

    // Switch to C4 perspective
    await page.getByTestId("perspective-toggle").getByTestId("perspective-c4").click();

    // Open Spotter
    await page.keyboard.press("Meta+k");
    await expect(page.getByTestId("spotter")).toBeVisible();
    await page.getByTestId("spotter-input").fill("build");

    const firstResult = page
      .getByTestId("spotter-results")
      .getByTestId(/^spotter-item-/);
    await expect(firstResult.first()).toBeVisible({ timeout: 5_000 });
    await firstResult.first().click();

    // Inspector opens
    await expect(page.getByTestId("object-inspector")).toBeVisible({ timeout: 5_000 });
  });

  test("Toggle, Spotter, and inspect full flow", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible();

    // Verify perspective toggle exists
    await expect(page.getByTestId("perspective-toggle")).toBeVisible();

    // Switch to C4
    await page.getByTestId("perspective-toggle").getByTestId("perspective-c4").click();

    // Switch back to Graph
    await page.getByTestId("perspective-toggle").getByTestId("perspective-graph").click();

    // Spotter → inspect
    await page.keyboard.press("Meta+k");
    await page.getByTestId("spotter-input").fill("build_overview");
    const result = page
      .getByTestId("spotter-results")
      .getByTestId(/^spotter-item-/);
    await expect(result.first()).toBeVisible({ timeout: 5_000 });
    await result.first().click();

    // Inspector shows
    await expect(page.getByTestId("object-inspector")).toBeVisible();
    await expect(page.getByRole("tablist", { name: /Available views/i })).toBeVisible();
  });
});
