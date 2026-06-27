/**
 * E2E tests for Phase 1 executors (e12a–e12e).
 *
 * Covers:
 * - G1: usage_examples  (USAGE_EXAMPLES_EXECUTOR — e12a)
 * - G2: api_surface     (API_SURFACE_EXECUTOR — e12b)
 * - G3: test_slice      (TEST_SLICE_EXECUTOR — e12c)
 * - G4: debug_slice     (DEBUG_SLICE_EXECUTOR — e12d)
 * - G5: change_impact_story (CHANGE_IMPACT_STORY_EXECUTOR — e12e)
 *
 * Each executor is reached by: open Spotter → select symbol →
 * switch to the view tab.  These tests verify the tabs are registered
 * and reachable from the UI — the actual block rendering is covered
 * by unit tests in ViewBlock.test.tsx.
 */
import { test, expect } from "@playwright/test";

test.describe("Phase 1 executors (e12a–e12e)", () => {
  // Shared navigation: open app → spotter → select symbol
  async function openSymbol(page: any) {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible();
    await page.waitForTimeout(1500);

    // Linux headless uses Control, not Meta
    const modifier = process.platform === "linux" ? "Control" : "Meta";
    await page.keyboard.press(`${modifier}+k`);

    const input = page.getByTestId("spotter-input");
    await expect(input).toBeVisible({ timeout: 5_000 });
    await input.fill("build");
    // Wait for debounce + network
    await page.waitForTimeout(800);

    const firstResult = page
      .getByTestId("spotter-results")
      .getByTestId(/^spotter-item-/);
    await expect(firstResult.first()).toBeVisible({ timeout: 5_000 });
    await firstResult.first().click();

    await expect(page.getByTestId("object-inspector")).toBeVisible();
  }

  // ---------------------------------------------------------------------------
  // G1: usage_examples (e12a)
  // ---------------------------------------------------------------------------
  test("usage-examples tab is registered and clickable", async ({ page }) => {
    await openSymbol(page);

    const tab = page.getByTestId("view-tab-usage-examples");
    await expect(tab).toBeVisible();
    await tab.click();

    // Verify the tab becomes active after clicking
    await expect(tab).toHaveAttribute("aria-selected", "true");
  });

  // ---------------------------------------------------------------------------
  // G2: api_surface (e12b)
  // NOTE: api_surface is a SCOPE view, not a SYMBOL view. It appears only
  // when inspecting a Scope object (crate/module/directory), not a Symbol.
  // The tab will not appear for Symbol context — this is correct behaviour.
  // G2 requires a separate test that opens a Scope object via Spotter.
  // ---------------------------------------------------------------------------
  test.skip("api-surface tab is registered and clickable", async ({ page }) => {
    // To implement G2: open Spotter, search for a scope (e.g. "explorer" or
    // the workspace root), select the scope object, then check for the tab.
    await openSymbol(page);

    const tab = page.getByTestId("view-tab-api-surface");
    await expect(tab).toBeVisible();
    await tab.click();

    await expect(tab).toHaveAttribute("aria-selected", "true");
  });

  // ---------------------------------------------------------------------------
  // G3: test_slice (e12c)
  // ---------------------------------------------------------------------------
  test("test-slice tab is registered and clickable", async ({ page }) => {
    await openSymbol(page);

    const tab = page.getByTestId("view-tab-test-slice");
    await expect(tab).toBeVisible();
    await tab.click();

    await expect(tab).toHaveAttribute("aria-selected", "true");
  });

  // ---------------------------------------------------------------------------
  // G4: debug_slice (e12d)
  // ---------------------------------------------------------------------------
  test("debug-slice tab is registered and clickable", async ({ page }) => {
    await openSymbol(page);

    const tab = page.getByTestId("view-tab-debug-slice");
    await expect(tab).toBeVisible();
    await tab.click();

    await expect(tab).toHaveAttribute("aria-selected", "true");
  });

  // ---------------------------------------------------------------------------
  // G5: change_impact_story (e12e)
  // ---------------------------------------------------------------------------
  test("change-impact-story tab is registered and clickable", async ({ page }) => {
    await openSymbol(page);

    const tab = page.getByTestId("view-tab-change-impact-story");
    await expect(tab).toBeVisible();
    await tab.click();

    await expect(tab).toHaveAttribute("aria-selected", "true");
  });
});
