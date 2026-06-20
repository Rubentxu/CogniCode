/**
 * E2E exploration flow tests — Graph → C4 toggle, Spotter → inspect.
 *
 * Tests the core user journey:
 *  1. C4 perspective toggle switches between Graph and C4 views
 *  2. Spotter search → select → inspect in pane-stack
 *  * 3. C4 perspective + Spotter → inspect
 *   * 4. Perspective toggle behavior
 * *
 * **Visual Regression:** Capturas en cada punto clave del flujo
 * MSW mocks handle all /api/* traffic (VITE_USE_MOCKS=true).
 */
import { test, expect } from "@playwright/test";

test.describe("Explorer exploration flows — Visual Regression", () => {
  test("Perspective toggle switches between Graph and C4 views", async ({
    page,
  }) => {
    await page.goto("/");
    const shell = page.getByTestId("shell");
    await expect(shell).toBeVisible({ timeout: 10_000 }); // Increased timeout

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

    // Golden image en perspectiva C4
    await expect(page).toHaveScreenshot("exploration-c4-perspective.png", {
      fullPage: true,
      animations: "disabled",
    });

    // Click back to Graph
    await graphBtn.click();

    // Golden image de vuelta a Graph
    await expect(page).toHaveScreenshot("exploration-graph-perspective.png", {
      fullPage: true,
      animations: "disabled",
    });
  });

  test("Spotter opens and inspects an object", async ({ page }) => {
    await page.goto("/");
    const shell = page.getByTestId("shell");
    await expect(shell).toBeVisible({ timeout: 10_000 });

    // Golden image del estado inicial
    await expect(page).toHaveScreenshot("exploration-spotter-initial.png", {
      fullPage: true,
      animations: "disabled",
    });

    // Open Spotter via Cmd+K — wait for listener to mount
    await page.waitForTimeout(1500); // Wait for keyboard listener to mount
    await page.keyboard.press("Meta+k");
    const spotter = page.getByTestId("spotter");
    await expect(spotter).toBeVisible({ timeout: 10_000 });

    // Golden image con Spotter abierto (empty)
    await expect(spotter).toHaveScreenshot("exploration-spotter-open.png", {
      animations: "disabled",
    });

    // Type a query — wait for input to be visible
    const input = page.getByTestId("spotter-input");
    await expect(input).toBeVisible({ timeout: 5_000 });
    await input.fill("build");

    // Wait for results
    const firstResult = page
      .getByTestId("spotter-results")
      .getByTestId(/^spotter-item-/);
    await expect(firstResult.first()).toBeVisible({ timeout: 5_000 });

    // Golden image con resultados del Spotter
    await expect(spotter).toHaveScreenshot("exploration-spotter-with-results.png", {
      animations: "disabled",
    });

    // Click the first result
    await firstResult.first().click();
    await expect(spotter).toBeHidden();

    // Verify the object inspector is shown
    await expect(page.getByTestId("object-inspector")).toBeVisible({ timeout: 5_000 });

    // Golden image con Object Inspector abierto
    await expect(page).toHaveScreenshot("exploration-object-inspector.png", {
      fullPage: true,
      animations: "disabled",
    });

    // Verify view tabs are rendered
    const tablist = page.getByRole("tablist", { name: /Available views/i });
    await expect(tablist).toBeVisible();

    // Golden image de las tabs de views
    await expect(tablist).toHaveScreenshot("exploration-view-tabs.png", {
      animations: "disabled",
    });
  });

  test("C4 perspective + Spotter search works", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Switch to C4 perspective
    const toggle = page.getByTestId("perspective-toggle");
    await expect(toggle).toBeVisible();
    await toggle.getByTestId("perspective-c4").click();

    // Golden image en C4 antes de Spotter
    await expect(page).toHaveScreenshot("exploration-c4-before-spotter.png", {
      fullPage: true,
      animations: "disabled",
    });

    // Open Spotter — wait for listener to mount
    await page.waitForTimeout(1500);
    await page.keyboard.press("Meta+k");
    await expect(page.getByTestId("spotter")).toBeVisible({ timeout: 10_000 });
    
    const input = page.getByTestId("spotter-input");
    await expect(input).toBeVisible({ timeout: 5_000 });
    await input.fill("build");

    const firstResult = page
      .getByTestId("spotter-results")
      .getByTestId(/^spotter-item-/);
    await expect(firstResult.first()).toBeVisible({ timeout: 5_000 });
    await firstResult.first().click();

    // Inspector opens
    await expect(page.getByTestId("object-inspector")).toBeVisible({ timeout: 5_000 });

    // Golden image de C4 + Spotter + Inspector
    await expect(page).toHaveScreenshot("exploration-c4-spotter-inspector.png", {
      fullPage: true,
      animations: "disabled",
    });
  });

  test("Toggle, Spotter, and inspect full flow", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Verify perspective toggle exists
    await expect(page.getByTestId("perspective-toggle")).toBeVisible();

    // Switch to C4
    await page.getByTestId("perspective-toggle").getByTestId("perspective-c4").click();

    // Golden image después de switch a C4
    await expect(page).toHaveScreenshot("exploration-full-c4.png", {
      fullPage: true,
      animations: "disabled",
    });

    // Switch back to Graph
    await page.getByTestId("perspective-toggle").getByTestId("perspective-graph").click();

    // Golden image después de switch a Graph
    await expect(page).toHaveScreenshot("exploration-full-graph.png", {
      fullPage: true,
      animations: "disabled",
    });

    // Spotter → inspect — wait for listener to mount
    await page.waitForTimeout(1500);
    await page.keyboard.press("Meta+k");
    
    const input = page.getByTestId("spotter-input");
    await expect(input).toBeVisible({ timeout: 5_000 });
    await input.fill("build_overview");
    
    const result = page
      .getByTestId("spotter-results")
      .getByTestId(/^spotter-item-/);
    await expect(result.first()).toBeVisible({ timeout: 5_000 });
    await result.first().click();

    // Golden image de Spotter con build_overview
    await expect(page.getByTestId("spotter")).toHaveScreenshot("exploration-spotter-build-overview.png", {
      animations: "disabled",
    });

    // Inspector shows
    await expect(page.getByTestId("object-inspector")).toBeVisible({ timeout: 5_000 });
    await expect(page.getByRole("tablist", { name: /Available views/i })).toBeVisible();

    // Golden image del flujo completo
    await expect(page).toHaveScreenshot("exploration-full-flow.png", {
      fullPage: true,
      animations: "disabled",
    });
  });
});