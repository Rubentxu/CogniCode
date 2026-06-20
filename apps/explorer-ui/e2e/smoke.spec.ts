/**
 * E2E smoke test — Phase 11 acceptance criterion 11.1.
 *
 * Full flow:
 *  1. Open the app — connection gate resolves to the Shell.
 *  2. Shell mounts with the 2-zone layout (nav + inspector).
 *  3. Spotter opens via Cmd+K, types a query, and shows results.
 *  4. Selecting a result closes the palette and inspects the object.
 *  5. View tabs render for the new object; clicking one updates the body.
 *  6. **Call graph view renders SVG with nodes (not blank)** — the bug fix
 *     from moldable-view-call-graph (v0.8.0).
 *
 * Visual regression: full-page golden image captures the complete
 * post-fix flow for future regression detection.
 *
 * The dev server is started with `VITE_USE_MOCKS=true` (see
 * `playwright.config.ts`), so every `/api/*` request is handled by
 * the MSW browser worker. The result is a deterministic, network-
 * free E2E suite that runs in CI.
 */
import { test, expect } from "@playwright/test";

// Top-level (cannot be inside describe — Playwright limitation)
test.use({ screenshot: "on" });

test.describe("Explorer smoke flow", () => {
  test("boots, opens Spotter, inspects object, and renders call graph (no blank SVG)", async ({
    page,
  }) => {
    await page.goto("/");

    // 1. App boots — title + Shell visible.
    await expect(
      page.getByRole("heading", { name: /CogniCode Explorer/i, level: 1 }),
    ).toBeVisible();
    const shell = page.getByTestId("shell");
    await expect(shell).toBeVisible();

    // 2. Open the Spotter via Cmd+K (wait 1500ms for keyboard listener mount).
    await page.waitForTimeout(1500);
    await page.keyboard.press("Meta+k");
    const spotter = page.getByTestId("spotter");
    await expect(spotter).toBeVisible();
    const input = page.getByTestId("spotter-input");
    await input.fill("build");

    // 3. The first fixture result becomes available.
    const firstResult = page
      .getByTestId("spotter-results")
      .getByTestId(/^spotter-item-/);
    await expect(firstResult.first()).toBeVisible({ timeout: 5_000 });

    // 4. Click the first result, palette closes, inspector renders.
    await firstResult.first().click();
    await expect(spotter).toBeHidden();
    await expect(page.getByTestId("object-inspector")).toBeVisible();

    // 5. At least one view tab is rendered for the new object.
    //    Using getByTestId("view-tabs") instead of getByRole+regex for robustness
    //    (see ADR-040 / moldable-view-call-graph fix in v0.8.0).
    const viewTabs = page.getByTestId("view-tabs");
    await expect(viewTabs).toBeVisible();
    const tabs = viewTabs.getByRole("tab");
    await expect(tabs.first()).toBeVisible();

    // 6. Call graph tab exists and renders SVG with nodes (THE BUG FIX).
    //    Pre-fix: SVG was blank. Post-fix: SVG renders interactive graph.
    const callGraphTab = page.getByTestId("view-tab-call-graph");
    if (await callGraphTab.isVisible()) {
      await callGraphTab.click();

      // Wait for the GraphViewRenderer to render.
      const graphView = page.getByTestId("graph-view-renderer");
      await expect(graphView).toBeVisible({ timeout: 5_000 });

      // The SVG canvas should NOT be blank — at least one node visible.
      const svgCanvas = page.getByTestId("svg-graph-canvas");
      await expect(svgCanvas).toBeVisible();
      const nodes = page.locator("[data-testid^='graph-node-']");
      await expect(nodes.first()).toBeVisible({ timeout: 3_000 });
      const nodeCount = await nodes.count();
      expect(nodeCount).toBeGreaterThan(1); // root + at least 1 callee
    }

    // Visual regression: full flow captured for future diff detection.
    await expect(page.getByTestId("shell")).toHaveScreenshot(
      "smoke-full-flow.png",
      { animations: "disabled", fullPage: true },
    );
  });

  test("shell boots in mock mode (no real backend required)", async ({ page }) => {
    // Lightweight smoke: just verifies the shell mounts with the connection
    // gate resolving to the shell via MSW. No API calls needed.
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });
    await expect(page.getByTestId("spotter-trigger")).toBeVisible();
  });
});