/**
 * E2E tests for ADR-005 E21-6: ExportMenu artifact auto-save.
 *
 * When an investigation is active, exporting (PNG/SVG/draw.io) automatically
 * adds the artifact to that investigation via the API.
 *
 * These tests use the MSW mocks for deterministic behavior.
 * Tests that require creating new investigations via API are marked as integration tests.
 */
import { test, expect } from "@playwright/test";

test.describe("E21-6: Export Menu — Artifact Auto-Save", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });
    await expect(page.getByTestId("loading-shell")).not.toBeVisible({ timeout: 10_000 });
  });

  test("ExportMenu renders all export options without crashing when no investigation is active", async ({ page }) => {
    // Navigate to a symbol to have a view to export
    await page.getByTestId("spotter-trigger").click();
    const spotterInput = page.getByTestId("spotter-input");
    await spotterInput.fill("UserService");
    await page.getByRole("option").first().click();

    // Wait for the pane to render
    await expect(page.getByTestId("object-inspector")).toBeVisible({ timeout: 10000 });

    // Open export menu
    await page.getByTestId("export-menu-trigger").click();

    // All menu items should be visible
    await expect(page.getByTestId("export-menu-open-drawio")).toBeVisible();
    await expect(page.getByTestId("export-menu-download-png")).toBeVisible();
    await expect(page.getByTestId("export-menu-download-svg")).toBeVisible();

    // Close menu
    await page.keyboard.press("Escape");
  });

  test("ExportMenu draw.io action handles missing Mermaid gracefully", async ({ page }) => {
    await page.getByTestId("spotter-trigger").click();
    const spotterInput = page.getByTestId("spotter-input");
    await spotterInput.fill("UserService");
    await page.getByRole("option").first().click();

    await expect(page.getByTestId("object-inspector")).toBeVisible({ timeout: 10000 });

    // Open export menu and try draw.io
    await page.getByTestId("export-menu-trigger").click();
    await page.getByTestId("export-menu-open-drawio").click();

    // Menu should still be accessible (notification shown or draw.io opened)
    await expect(page.getByTestId("export-menu-trigger")).toBeVisible({ timeout: 3000 });
  });

  test("ExportMenu PNG download does not crash", async ({ page }) => {
    await page.getByTestId("spotter-trigger").click();
    const spotterInput = page.getByTestId("spotter-input");
    await spotterInput.fill("UserService");
    await page.getByRole("option").first().click();

    await expect(page.getByTestId("object-inspector")).toBeVisible({ timeout: 10000 });

    // Intercept snapshot API to avoid actual download
    await page.route("**/api/snapshots/**", (route) => {
      route.fulfill({
        status: 200,
        contentType: "image/png",
        body: Buffer.from("fake-png-data"),
      });
    });

    // Open export menu and try PNG download
    await page.getByTestId("export-menu-trigger").click();
    await page.getByTestId("export-menu-download-png").click();

    // Menu should still be accessible
    await expect(page.getByTestId("export-menu-trigger")).toBeVisible({ timeout: 3000 });
  });

  test("ExportMenu SVG download does not crash", async ({ page }) => {
    await page.getByTestId("spotter-trigger").click();
    const spotterInput = page.getByTestId("spotter-input");
    await spotterInput.fill("UserService");
    await page.getByRole("option").first().click();

    await expect(page.getByTestId("object-inspector")).toBeVisible({ timeout: 10000 });

    // Intercept snapshot API
    await page.route("**/api/snapshots/**", (route) => {
      route.fulfill({
        status: 200,
        contentType: "image/svg+xml",
        body: Buffer.from("<svg></svg>"),
      });
    });

    // Open export menu and try SVG download
    await page.getByTestId("export-menu-trigger").click();
    await page.getByTestId("export-menu-download-svg").click();

    // Menu should still be accessible
    await expect(page.getByTestId("export-menu-trigger")).toBeVisible({ timeout: 3000 });
  });
});

test.describe("E21-6: Investigations Section — UI Components", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });
    await expect(page.getByTestId("loading-shell")).not.toBeVisible({ timeout: 10_000 });
  });

  test("investigations tab is accessible and shows section", async ({ page }) => {
    await page.getByTestId("landing-tab-investigations").click();
    await expect(page.getByTestId("investigations-section")).toBeVisible();
  });

  test("new investigation form can be opened", async ({ page }) => {
    await page.getByTestId("landing-tab-investigations").click();
    await expect(page.getByTestId("investigations-section")).toBeVisible();

    // Click new investigation button
    await page.getByTestId("new-investigation-button").click();

    // Form should be visible
    await expect(page.getByTestId("new-investigation-form")).toBeVisible();
  });

  test("new investigation form has title and goal inputs", async ({ page }) => {
    await page.getByTestId("landing-tab-investigations").click();
    await page.getByTestId("new-investigation-button").click();

    const form = page.getByTestId("new-investigation-form");
    await expect(form.locator("input[type='text']")).toBeVisible();
    await expect(form.locator("textarea")).toBeVisible();
  });

  test("investigation templates are shown when no investigations exist", async ({ page }) => {
    await page.getByTestId("landing-tab-investigations").click();
    await expect(page.getByTestId("investigations-section")).toBeVisible();

    // Templates should be visible as fallback when no investigations exist
    // (checked by looking for template buttons)
    const templates = page.getByTestId(/investigation-template-/);
    await expect(templates.first()).toBeVisible({ timeout: 5000 });
  });
});
