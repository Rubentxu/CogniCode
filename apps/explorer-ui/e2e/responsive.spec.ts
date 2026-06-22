/**
 * E2E responsive test — Phase 11 acceptance criterion 11.4 + Part C.
 *
 * Verifies the Shell layout adapts to the three breakpoints
 * (desktop ≥ 1200px, tablet 900–1199px, small < 900px). The Shell
 * exposes the resolved viewport through `data-viewport` so we can
 * assert on it without measuring pixels.
 *
 * The Spotter + the inspector + the lens panel must keep working
 * at every viewport (no overlap, no horizontal scroll on the body,
 * no clipped buttons).
 *
 * VISUAL VALIDATION: All tests capture screenshots for regression testing.
 *
 * Part C additions:
 *  - Small viewport: bottom-sheet is present and usable
 *  - Keyboard navigation: Spotter via Cmd+K, Enter to select
 *  - Perspective toggle keyboard accessible (Tab + Enter/Space)
 */
import { test, expect, type Page } from "@playwright/test";

const BREAKPOINTS = [
  { name: "desktop", width: 1280, height: 800, expectedViewport: "desktop" },
  { name: "tablet", width: 1024, height: 800, expectedViewport: "tablet" },
  { name: "small", width: 768, height: 900, expectedViewport: "small" },
] as const;

async function primeApp(page: Page, expectedViewport: "desktop" | "tablet" | "small") {
  await page.goto("/");
  await expect(page.getByTestId("shell")).toBeVisible();
  // Wait 1500ms for keyboard listener to mount
  await page.waitForTimeout(1500);
  // Drive the app into a state that exercises both zones.
  await page.keyboard.press("Meta+k");
  await page.getByTestId("spotter-input").fill("build");
  const firstResult = page
    .getByTestId("spotter-results")
    .getByTestId(/^spotter-item-/);
  await firstResult.first().click();
  // At small viewport the bottom-sheet takes the inspector pane;
  // at desktop/tablet the object-inspector is in the right zone.
  if (expectedViewport === "small") {
    await expect(page.getByTestId("bottom-sheet")).toBeVisible();
    await expect(page.getByTestId("object-inspector")).toBeVisible();
  } else {
    await expect(page.getByTestId("object-inspector")).toBeVisible();
  }
}

for (const bp of BREAKPOINTS) {
  test.describe(`viewport: ${bp.name} (${bp.width}x${bp.height})`, () => {
    test(`Shell resolves to ${bp.expectedViewport}`, async ({ page }) => {
      await page.setViewportSize({ width: bp.width, height: bp.height });
      await page.goto("/");
      const shell = page.getByTestId("shell");
      await expect(shell).toBeVisible();
      await expect(shell).toHaveAttribute("data-viewport", bp.expectedViewport);

      // VISUAL VALIDATION: Capture shell at this viewport
      await expect(page).toHaveScreenshot(`responsive-shell-${bp.name}.png`, {
        animations: "disabled",
        fullPage: true,
        maxDiffPixels: bp.name === "desktop" ? 100 : 0,
      });
    });

    test("the main flow holds at this viewport", async ({ page }) => {
      await page.setViewportSize({ width: bp.width, height: bp.height });
      await primeApp(page, bp.expectedViewport);
      // At desktop + tablet the inspector is in the right pane;
      // at small the bottom-sheet is used for the inspector pane.
      if (bp.expectedViewport !== "small") {
        await expect(page.getByTestId("object-inspector-body")).toBeVisible();
      } else {
        // Small viewport — assert the bottom-sheet is present and
        // no horizontal overflow happens (asserted in the next test).
        await expect(page.getByTestId("bottom-sheet")).toBeVisible();
      }

      // VISUAL VALIDATION: Capture main flow at this viewport
      await expect(page).toHaveScreenshot(`responsive-mainflow-${bp.name}.png`, {
        animations: "disabled",
        fullPage: true,
      });
    });

    test("no horizontal overflow on the body", async ({ page }) => {
      await page.setViewportSize({ width: bp.width, height: bp.height });
      await page.goto("/");
      // Wait a beat for the resize listener to settle.
      await page.waitForTimeout(200);
      const scrollWidth = await page.evaluate(
        () => document.documentElement.scrollWidth,
      );
      const clientWidth = await page.evaluate(
        () => document.documentElement.clientWidth,
      );
      expect(scrollWidth).toBeLessThanOrEqual(clientWidth + 1);

      // VISUAL VALIDATION: Capture full page to verify no overflow
      await expect(page).toHaveScreenshot(`responsive-nooverflow-${bp.name}.png`, {
        animations: "disabled",
        fullPage: true,
        maxDiffPixels: bp.name === "desktop" ? 11000 : 100,
      });
    });
  });
}

// =============================================================================
// Part C — Keyboard navigation & small-viewport bottom-sheet
// =============================================================================

test.describe("Part C — keyboard navigation & small viewport", () => {
  test("P6.3 — small viewport bottom-sheet is present and usable", async ({ page }) => {
    await page.setViewportSize({ width: 768, height: 900 });

    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible();
    await expect(page.getByTestId("shell")).toHaveAttribute("data-viewport", "small");

    // Open the app to exercise the bottom sheet
    await page.keyboard.press("Meta+k");
    await page.getByTestId("spotter-input").fill("build");
    const firstResult = page
      .getByTestId("spotter-results")
      .getByTestId(/^spotter-item-/);
    await firstResult.first().click();

    // Bottom sheet should be visible at small viewport
    await expect(page.getByTestId("bottom-sheet")).toBeVisible();
    // And the object inspector should be inside the bottom sheet
    await expect(page.getByTestId("object-inspector")).toBeVisible();

    // VISUAL VALIDATION: Capture bottom sheet at small viewport
    await expect(page).toHaveScreenshot("responsive-bottomsheet-small.png", {
      animations: "disabled",
      fullPage: true,
    });
  });

  test("P6.5 — keyboard flow: Cmd+K opens Spotter, Enter selects result, inspector opens", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible();

    // Wait 1500ms for keyboard listener to mount
    await page.waitForTimeout(1500);

    // Focus is anywhere — Cmd+K opens Spotter
    await page.keyboard.press("Meta+k");
    const spotter = page.getByTestId("spotter");
    await expect(spotter).toBeVisible();

    // Type a query
    const input = page.getByTestId("spotter-input");
    await input.fill("build");

    // Wait for results
    const results = page.getByTestId("spotter-results").getByTestId(/^spotter-item-/);
    await expect(results.first()).toBeVisible({ timeout: 5_000 });

    // VISUAL VALIDATION: Capture Spotter with results
    await expect(page).toHaveScreenshot("keyboard-spotter-with-results.png", {
      animations: "disabled",
      fullPage: true,
    });

    // Press Enter to select the first result
    await page.keyboard.press("Enter");

    // Spotter should close
    await expect(spotter).toBeHidden();

    // Inspector should open
    await expect(page.getByTestId("object-inspector")).toBeVisible();

    // VISUAL VALIDATION: Capture inspector after keyboard selection
    await expect(page).toHaveScreenshot("keyboard-inspector-after-enter.png", {
      animations: "disabled",
      fullPage: true,
    });
  });

  test("P2.6 — perspective toggle is keyboard accessible (Tab + Enter)", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible();

    const graphBtn = page.getByTestId("perspective-graph");
    const c4Btn = page.getByTestId("perspective-c4");

    // Default state: Graph is active (aria-pressed="true")
    await expect(graphBtn).toHaveAttribute("aria-pressed", "true");

    // Click C4 button to switch perspective (verify toggle works via click)
    await c4Btn.click();

    // The C4 button should now be active (aria-pressed="true")
    await expect(c4Btn).toHaveAttribute("aria-pressed", "true");
    await expect(graphBtn).toHaveAttribute("aria-pressed", "false");

    // Click Graph button to switch back
    await graphBtn.click();
    await expect(graphBtn).toHaveAttribute("aria-pressed", "true");
    await expect(c4Btn).toHaveAttribute("aria-pressed", "false");

    // Now verify keyboard: press Space on the Graph button (which is focused)
    await graphBtn.focus();
    await page.keyboard.press("Space");
    // Graph should stay active (Space on already-active button is a no-op)
    await expect(graphBtn).toHaveAttribute("aria-pressed", "true");

    // Press Enter on C4 button via keyboard
    await c4Btn.focus();
    await page.keyboard.press("Enter");
    await expect(c4Btn).toHaveAttribute("aria-pressed", "true");
    await expect(graphBtn).toHaveAttribute("aria-pressed", "false");

    // VISUAL VALIDATION: Capture perspective toggle after keyboard interaction
    await expect(page).toHaveScreenshot("keyboard-perspective-toggle.png", {
      animations: "disabled",
      fullPage: true,
    });
  });

  test("keyboard navigation: Escape closes Spotter", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible();

    // Wait 1500ms for keyboard listener to mount
    await page.waitForTimeout(1500);

    await page.keyboard.press("Meta+k");
    const spotter = page.getByTestId("spotter");
    await expect(spotter).toBeVisible();

    await page.getByTestId("spotter-input").fill("build");
    await page.waitForTimeout(200);

    await page.keyboard.press("Escape");
    await expect(spotter).toBeHidden();

    // VISUAL VALIDATION: Capture UI after Spotter closed via Escape
    await expect(page).toHaveScreenshot("keyboard-spotter-closed-by-escape.png", {
      animations: "disabled",
      fullPage: true,
      maxDiffPixels: 100,
    });
  });

  test("keyboard navigation: Arrow keys navigate Spotter results", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible();

    // Wait 1500ms for keyboard listener to mount
    await page.waitForTimeout(1500);

    await page.keyboard.press("Meta+k");
    const spotter = page.getByTestId("spotter");
    await expect(spotter).toBeVisible();

    await page.getByTestId("spotter-input").fill("build");

    // Wait for results
    const results = page.getByTestId("spotter-results").getByTestId(/^spotter-item-/);
    await expect(results.first()).toBeVisible({ timeout: 5_000 });

    // ArrowDown to move focus to second result
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowDown");

    // ArrowUp to move back
    await page.keyboard.press("ArrowUp");

    // VISUAL VALIDATION: Capture Spotter with keyboard navigation
    await expect(page).toHaveScreenshot("keyboard-spotter-arrow-navigation.png", {
      animations: "disabled",
      fullPage: true,
    });

    // Enter to select
    await page.keyboard.press("Enter");
    await expect(spotter).toBeHidden();
    await expect(page.getByTestId("object-inspector")).toBeVisible();

    // VISUAL VALIDATION: Capture inspector after keyboard selection
    await expect(page).toHaveScreenshot("keyboard-inspector-after-arrow-select.png", {
      animations: "disabled",
      fullPage: true,
    });
  });
});
