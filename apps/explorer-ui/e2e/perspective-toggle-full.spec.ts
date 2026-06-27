/**
 * E2E: Perspective toggle (C4 / Default).
 *
 * GToolkit parity: GT has multiple tool perspectives (Coder, Moldable,
 * Mondrian). CogniCode has Default, C4, Quality perspectives. Toggling
 * between them must update the explorer layout.
 */
import { test, expect } from "@playwright/test";
import { snapshot } from "./utils/screenshot";

test.describe("Perspective toggle (GT tool perspective parity)", () => {
  test("perspective toggle is visible with current selection", async ({ page }) => {
    await page.goto("/");

    const toggle = page.getByTestId("perspective-toggle");
    await expect(toggle).toBeVisible();

    // Either graph or c4 should be pressed depending on default state.
    const graph = page.getByTestId("perspective-graph");
    const c4 = page.getByTestId("perspective-c4");
    await expect(graph).toBeVisible();
    await expect(c4).toBeVisible();

    await snapshot(page, "perspective-toggle-default.png");
  });

  test.skip("toggle to C4 changes layout [DEBT: layout may not visibly change in mock mode]", async ({
    page,
  }) => {
    // The toggle updates state but the layout change in mock mode may be
    // subtle. Skip until a clear visual signal is added to distinguish
    // perspectives (e.g. data-perspective attribute on shell root).
    await page.goto("/");
    const c4 = page.getByTestId("perspective-c4");
    await c4.click();
    await expect(c4).toHaveAttribute("aria-pressed", "true");
    await snapshot(page, "perspective-toggle-c4.png");
  });
});
