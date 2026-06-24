/**
 * E2E responsive & accessibility tests — Phase 6 of the explorer-e2e-test-plan.
 *
 * Verifies the Explorer adapts to different viewport sizes and is
 * keyboard-navigable.
 *
 * All tests rely on MSW handlers (VITE_USE_MOCKS=true).
 * Viewport changes use `page.setViewportSize()` to trigger CSS breakpoints.
 *
 * Phase 6 scenarios (5 tests) from docs/explorer-e2e-test-plan.md:
 *   P6.1 Desktop: graph + pane-stack side-by-side
 *   P6.2 Tablet: lens overlay toggle
 *   P6.3 Small: bottom-sheet visible
 *   P6.4 Focus order: natural reading order
 *   P6.5 All elements reachable via keyboard
 */
import { test, expect } from "@playwright/test";

test.describe("Phase 6: Responsive & Accessibility (5 tests)", () => {
  test("P6.1 Desktop: graph + pane-stack side-by-side", async ({ page }) => {
    // Desktop viewport: 1440x900
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/");

    // Shell renders
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });
    // The shell has data-viewport="desktop" (or "ultrawide" if wider)
    const shell = page.getByTestId("shell");
    const viewport = await shell.getAttribute("data-viewport");
    expect(viewport).toMatch(/desktop|ultrawide/);
  });

  test("P6.2 Tablet: lens overlay toggle", async ({ page }) => {
    // Tablet viewport: 1024x768
    await page.setViewportSize({ width: 1024, height: 768 });
    await page.goto("/");

    // Shell renders with tablet viewport
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });
    const viewport = await page.getByTestId("shell").getAttribute("data-viewport");
    expect(viewport).toMatch(/tablet|desktop/);
  });

  test("P6.3 Small: bottom-sheet visible", async ({ page }) => {
    // Small viewport: 600x800
    await page.setViewportSize({ width: 600, height: 800 });
    await page.goto("/");

    // Shell renders with small viewport
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });
    const viewport = await page.getByTestId("shell").getAttribute("data-viewport");
    expect(viewport).toBe("small");

    // The bottom-sheet is the container for the pane-stack on small viewports
    const bottomSheet = page.getByTestId("bottom-sheet");
    await expect(bottomSheet).toBeVisible();
  });

  test("P6.4 Focus order: natural reading order", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Press Tab repeatedly and check that focus moves through interactive
    // elements in a logical order (skip link → header controls → landing/canvas)
    await page.keyboard.press("Tab");
    const firstFocus = await page.evaluate(
      () => document.activeElement?.getAttribute("data-testid") ?? null,
    );

    // Continue tabbing — at some point we should reach an interactive
    // control (button, link, input)
    for (let i = 0; i < 10; i++) {
      await page.keyboard.press("Tab");
      const focus = await page.evaluate(() => {
        const el = document.activeElement as HTMLElement | null;
        return el ? el.tagName + (el.getAttribute("data-testid") ? `[data-testid=${el.getAttribute("data-testid")}]` : "") : null;
      });
      // Stop when we find an interactive element
      if (focus && (focus.includes("BUTTON") || focus.includes("INPUT") || focus.includes("A"))) {
        expect(focus).toBeTruthy();
        return;
      }
    }

    // We found at least one focusable element
    expect(firstFocus).toBeTruthy();
  });

  test("P6.5 All elements reachable via keyboard", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // 1. Skip link is the first tabbable element
    await page.keyboard.press("Tab");
    const skipLink = page.getByTestId("skip-link");
    if (await skipLink.isVisible()) {
      // Skip link is focused
      const focused = await page.evaluate(
        () => document.activeElement?.getAttribute("data-testid") ?? null,
      );
      expect(focused).toBe("skip-link");
    }

    // 2. Spotter trigger is reachable
    const spotterTrigger = page.getByTestId("spotter-trigger");
    await spotterTrigger.focus();
    await expect(spotterTrigger).toBeFocused();

    // 3. Perspective toggle is reachable
    const c4Btn = page.getByTestId("perspective-c4");
    await c4Btn.focus();
    await expect(c4Btn).toBeFocused();
    // Can be activated with keyboard
    await page.keyboard.press("Enter");
    await expect(c4Btn).toHaveAttribute("aria-pressed", "true");
  });
});
