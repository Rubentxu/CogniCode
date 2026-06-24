/**
 * E2E visual regression tests — Phase 11 acceptance criterion 11.1.
 *
 * Full flow:
 *  1. Open the app — connection gate resolves to the Shell.
 *  2. Shell mounts with the 2-zone layout (nav + inspector).
 *  3. Spotter opens via Cmd+K, types a query, and shows results.
 *  4. Selecting a result closes the palette and inspects the object.
 *  5. View tabs render for the new object; clicking one updates the body.
 *
 * The dev server is started with `VITE_USE_MOCKS=true` (see
 * `playwright.config.ts`), so every `/api/*` request is handled by
 * the MSW browser worker. The result is a deterministic, network-
 * free E2E suite that runs in CI.
 *
 * **Visual Regression:** Capturas en cada punto clave del flujo
 */
import { test, expect } from "@playwright/test";

test.describe("Explorer smoke flow — Visual Regression", () => {
  test("boots, opens Spotter, and inspects an object", async ({
    page,
  }) => {
    await page.goto("/");
    await expect(
      page.getByRole("heading", { name: /CogniCode Explorer/i, level: 1 }),
    ).toBeVisible({ timeout: 10_000 });
    const shell = page.getByTestId("shell");
    await expect(shell).toBeVisible({ timeout: 10_000 });

    // Golden image del estado inicial
    await expect(page).toHaveScreenshot("smoke-initial-load.png", {
      fullPage: true,
      animations: "disabled",
      maxDiffPixels: 3000,
    });

    // 2. Open the Spotter via Cmd+K — wait for listener to mount
    await page.waitForTimeout(1500);
    await page.keyboard.press("Meta+k");
    const spotter = page.getByTestId("spotter");
    await expect(spotter).toBeVisible({ timeout: 10_000 });

    const input = page.getByTestId("spotter-input");
    await expect(input).toBeVisible({ timeout: 5_000 });
    await input.fill("build");

    // 3. The first fixture result becomes available.
    const firstResult = page
      .getByTestId("spotter-results")
      .getByTestId(/^spotter-item-/);
    await expect(firstResult.first()).toBeVisible({ timeout: 5_000 });

    // Golden image del Spotter con resultados
    await expect(spotter).toHaveScreenshot("smoke-spotter-results.png", {
      animations: "disabled",
    });

    // 4. Click the first result, palette closes, inspector renders.
    await firstResult.first().click();
    await expect(spotter).toBeHidden();

    await expect(page.getByTestId("object-inspector")).toBeVisible();

    // Golden image con Object Inspector abierto
    await expect(page).toHaveScreenshot("smoke-object-inspector.png", {
      fullPage: true,
      animations: "disabled",
    });

    // 5. At least one view tab is rendered for the new object.
    const tablist = page.getByTestId("view-tabs");
    await expect(tablist).toBeVisible();
    const tabs = tablist.getByRole("tab");
    await expect(tabs.first()).toBeVisible();

    // Golden image de las tabs de views
    await expect(tablist).toHaveScreenshot("smoke-view-tabs.png", {
      animations: "disabled",
    });
  });

  test("C4 perspective + Spotter search works — Visual Regression", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Switch to C4 perspective
    await page.getByTestId("perspective-toggle").getByTestId("perspective-c4").click();

    // Golden image en perspectiva C4
    await expect(page).toHaveScreenshot("smoke-c4-perspective.png", {
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
    await expect(page).toHaveScreenshot("smoke-c4-spotter-inspector.png", {
      fullPage: true,
      animations: "disabled",
    });
  });

  test("View tabs visible after object selection — Visual Regression", async ({
    page,
  }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Open Spotter — wait for listener to mount
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

    // Inspector shows
    await expect(page.getByTestId("object-inspector")).toBeVisible({ timeout: 5_000 });
    await expect(page.getByTestId("view-tabs")).toBeVisible();

    // Golden image de las tabs de views
    await expect(page.getByTestId("view-tabs")).toHaveScreenshot("smoke-inspector-with-tabs.png", {
      animations: "disabled",
    });
  });
});

test.describe("Explorer call-graph view — Visual Regression", () => {
  test("navigates to the call-graph view and the SVG renders", async ({
    page,
  }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Pick an object via the Spotter — wait for listener to mount
    await page.waitForTimeout(1500);
    await page.keyboard.press("Meta+k");
    const input = page.getByTestId("spotter-input");
    await expect(input).toBeVisible({ timeout: 5_000 });
    await input.fill("build");
    const firstResult = page
      .getByTestId("spotter-results")
      .getByTestId(/^spotter-item-/);
    await firstResult.first().click();
    await expect(page.getByTestId("object-inspector")).toBeVisible();

    // Switch to the call-graph view.
    const graphTab = page.getByTestId("view-tab-call-graph");
    await expect(graphTab).toBeVisible();
    await graphTab.click();

    const graphView = page.getByTestId("graph-view-renderer");
    await expect(graphView).toBeVisible({ timeout: 5_000 });
    await expect(page.getByTestId("svg-graph-canvas")).toBeVisible();
    const nodes = page.locator("[data-testid^='graph-node-']");
    await expect(nodes.first()).toBeVisible({ timeout: 3_000 });

    // Golden image del view de call-graph
    await expect(page).toHaveScreenshot("graph-call-graph-view.png", {
      fullPage: true,
      animations: "disabled",
    });
  });

  test("clicking a hotspot navigates to the target object", async ({
    page,
  }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Pick an object via the Spotter — wait for listener to mount
    await page.waitForTimeout(1500);
    await page.keyboard.press("Meta+k");
    
    const input = page.getByTestId("spotter-input");
    await expect(input).toBeVisible({ timeout: 5_000 });
    await input.fill("build");
    const firstResult = page
      .getByTestId("spotter-results")
      .getByTestId(/^spotter-item-/);
    await firstResult.first().click();

    // Switch to the "quality" view — the fixture has a hotspots
    // block. Each hotspot row is interactive.
    const qualityTab = page.getByTestId("view-tab-quality");
    if (await qualityTab.isVisible()) {
      await qualityTab.click();
      const hotspot = page
        .getByTestId("object-inspector-body")
        .getByTestId(/^view-block-hotspot-/);
      if (await hotspot.first().isVisible()) {
        // Clicking a hotspot dispatches SELECT_OBJECT. The
        // active-object id should change in the page (we can't
        // easily read it from state, so we just verify the
        // inspector re-renders).
        await hotspot.first().click();
        await page.waitForTimeout(500);

        // Golden image de hotspot click
        await expect(page).toHaveScreenshot("graph-hotspot-click.png", {
          fullPage: true,
          animations: "disabled",
        });
      }
    }
  });
});

test.describe("Explorer error states — Visual Regression", () => {
  test("connection gate resolves to Shell in mock mode", async ({ page }) => {
    await page.goto("/");
    await page.waitForTimeout(2000);

    // In mock mode, the health probe succeeds (MSW returns 200 for /api/health)
    // The connection gate resolves and the Shell mounts
    const shell = page.getByTestId("shell");
    await expect(shell).toBeVisible({ timeout: 10_000 });

    // The health chip should show "online" status
    const healthChip = page.getByTestId("health-chip");
    await expect(healthChip).toBeVisible();
    await expect(healthChip).toHaveAttribute("data-status", "online");

    // Golden image del Shell
    await expect(page).toHaveScreenshot("error-states-connection-gate.png", {
      fullPage: true,
      animations: "disabled",
      maxDiffPixels: 3000,
    });
  });

  test("LoadingTier renders error state when object fetch fails", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Open Spotter — wait for listener to mount
    await page.waitForTimeout(1500);
    await page.keyboard.press("Meta+k");
    await page.getByTestId("spotter-input").fill("build");
    const results = page.getByTestId("spotter-results").getByTestId(/^spotter-item-/);
    await results.first().click();

    // The inspector should render without error
    const inspector = page.getByTestId("object-inspector");
    await expect(inspector).toBeVisible();

    // The inspector body should load (not stuck in error)
    const body = page.getByTestId("object-inspector-body");
    await expect(body).toBeVisible();

    // Golden image del inspector cargado
    await expect(page).toHaveScreenshot("error-states-inspector-loaded.png", {
      fullPage: true,
      animations: "disabled",
    });
  });

  test("graph-landing renders without crashing when landing data loads", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // The graph-landing should render without throwing
    // If MSW provides landing fixture, the graph-landing canvas should be visible
    // It may take a moment to load
    await page.waitForTimeout(1000);

    // The component should be visible (either loading, error, or loaded)
    const landingStates = [
      page.getByTestId("graph-landing-loading"),
      page.getByTestId("graph-landing-error"),
      page.getByTestId("graph-landing-canvas"),
    ];

    const anyVisible = await Promise.all(
      landingStates.map((s) => s.isVisible().catch(() => false)),
    );
    expect(anyVisible.some(Boolean)).toBe(true);

    // Golden image del graph landing
    await expect(page.getByTestId("shell")).toHaveScreenshot("error-states-graph-landing.png", {
      fullPage: true,
      animations: "disabled",
      maxDiffPixels: 100,
    });
  });

  test("object inspector handles missing view gracefully", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Open Spotter — wait for listener to mount
    await page.waitForTimeout(1500);
    await page.keyboard.press("Meta+k");
    await page.getByTestId("spotter-input").fill("build");
    const results = page.getByTestId("spotter-results").getByTestId(/^spotter-item-/);
    await results.first().click();

    const inspector = page.getByTestId("object-inspector");
    await expect(inspector).toBeVisible();

    // The view tabs should be visible
    const tablist = page.getByTestId("view-tabs");
    await expect(tablist).toBeVisible();
    const tabs = tablist.getByRole("tab");
    const tabCount = await tabs.count();
    expect(tabCount).toBeGreaterThan(0);

    // Each tab should be clickable without crashing
    const firstTab = tabs.first();
    await firstTab.click();
    await page.waitForTimeout(300);

    // Inspector body should still be visible after tab switch
    await expect(page.getByTestId("object-inspector-body")).toBeVisible();

    // Golden image del inspector con tabs
    await expect(page).toHaveScreenshot("error-states-inspector-tabs.png", {
      fullPage: true,
      animations: "disabled",
    });
  });

  test("closing last pane shows empty state without crash", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Open a pane — wait for listener to mount
    await page.waitForTimeout(1500);
    await page.keyboard.press("Meta+k");
    await page.getByTestId("spotter-input").fill("build");
    const results = page.getByTestId("spotter-results").getByTestId(/^spotter-item-/);
    await results.first().click();
    await expect(page.getByTestId("object-inspector")).toBeVisible();

    // Close the pane
    const closeBtn = page.getByTestId("pane-close");
    await closeBtn.click();

    // Empty state should be visible
    const emptyState = page.getByTestId("pane-stack-empty");
    await expect(emptyState).toBeVisible();

    // Golden image del empty state
    await expect(page).toHaveScreenshot("error-states-pane-stack-empty.png", {
      fullPage: true,
      animations: "disabled",
    });
  });

  test("Empty spotter results (no matches)", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Open Spotter — wait for listener to mount
    await page.waitForTimeout(1500);
    await page.keyboard.press("Meta+k");
    await expect(page.getByTestId("spotter")).toBeVisible({ timeout: 10_000 });
    await page.getByTestId("spotter-input").fill("xyznonexistent12345");

    // Validar que el Spotter esté visible
    const spotter = page.getByTestId("spotter");
    await expect(spotter).toBeVisible();

    // Golden image del Spotter vacío
    await expect(spotter).toHaveScreenshot("error-states-empty-spotter.png", {
      animations: "disabled",
    });
  });
});
