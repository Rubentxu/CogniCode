/**
 * E2E landing page tests — Phase 1 of the explorer-e2e-test-plan.
 *
 * Verifies the GraphLanding component renders as the first screen when
 * the user opens the Explorer without an explicit workspace selection
 * (F1 prerequisite: ShellBootstrap auto-selects the first workspace
 * from the MSW-mocked `/api/workspaces` endpoint).
 *
 * All tests rely on MSW handlers (VITE_USE_MOCKS=true; see
 * playwright.config.ts). No real axum backend is needed.
 *
 * Phase 1 scenarios (8 tests) from docs/explorer-e2e-test-plan.md:
 *   P1.1 Graph landing renders after workspace bootstrap
 *   P1.2 Landing shows root nodes in cytoscape canvas
 *   P1.3 Landing shows suggested questions strip
 *   P1.4 Landing canvas is interactive (pan/zoom)
 *   P1.5 Click root node → pane-stack opens
 *   P1.6 Landing header: workspace name + symbol count
 *   P1.7 Landing error state when fetch fails
 *   P1.8 Landing loading state during fetch
 */
import { test, expect } from "@playwright/test";

test.describe("Phase 1: Landing page (8 tests)", () => {
  test("P1.1 Graph landing renders after workspace bootstrap", async ({ page }) => {
    await page.goto("/");

    // Shell mounts
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // GraphLanding renders (not the InteractiveGraphPanel — that needs a rootId)
    await expect(page.getByTestId("graph-landing")).toBeVisible({ timeout: 10_000 });

    // Loading state clears once data arrives
    await expect(page.getByTestId("graph-landing-loading")).toBeHidden({
      timeout: 10_000,
    });
  });

  test("P1.2 Landing shows root nodes in cytoscape canvas", async ({ page }) => {
    await page.goto("/");

    // Wait for the landing canvas to appear (cytoscape mounts lazily)
    const canvas = page.getByTestId("graph-landing-canvas");
    await expect(canvas).toBeVisible({ timeout: 15_000 });

    // Cytoscape renders nodes with data-testid="graph-node-<id>"
    // The MSW landing mock returns several root nodes
    const nodes = page.locator("[data-testid^='graph-node-']");
    await expect(nodes.first()).toBeVisible({ timeout: 10_000 });
    const count = await nodes.count();
    expect(count).toBeGreaterThan(0);
  });

  test("P1.3 Landing shows suggested questions strip", async ({ page }) => {
    await page.goto("/");

    // The suggestion strip renders below the canvas
    const strip = page.getByTestId("landing-suggestion-strip");
    await expect(strip).toBeVisible({ timeout: 10_000 });

    // At least one suggestion is visible
    const suggestions = page.locator("[data-testid^='suggested-question-']");
    await expect(suggestions.first()).toBeVisible({ timeout: 5_000 });
  });

  test("P1.4 Landing canvas is interactive (pan/zoom)", async ({ page }) => {
    await page.goto("/");

    const canvas = page.getByTestId("graph-landing-canvas");
    await expect(canvas).toBeVisible({ timeout: 15_000 });

    // Cytoscape container has tabindex=0 for keyboard interaction
    const tabIndex = await canvas.getAttribute("tabindex");
    expect(tabIndex).toBe("0");

    // role=application is set for ARIA compatibility
    const role = await canvas.getAttribute("role");
    expect(role).toBe("application");
  });

  test("P1.5 Click root node → pane-stack opens", async ({ page }) => {
    await page.goto("/");

    // Wait for landing to render
    await expect(page.getByTestId("graph-landing")).toBeVisible({ timeout: 10_000 });

    // Wait for at least one node
    const nodes = page.locator("[data-testid^='graph-node-']");
    await expect(nodes.first()).toBeVisible({ timeout: 15_000 });

    // Click the first node — should select it
    await nodes.first().click();

    // Pane-stack renders the inspector
    await expect(page.getByTestId("object-inspector")).toBeVisible({
      timeout: 10_000,
    });
  });

  test("P1.6 Landing header: workspace name + symbol count", async ({ page }) => {
    await page.goto("/");

    // Header is visible
    const header = page.getByTestId("landing-header");
    await expect(header).toBeVisible({ timeout: 10_000 });

    // Workspace name is shown (derived from root_path basename in LandingHeader)
    const name = page.getByTestId("landing-workspace-name");
    await expect(name).toBeVisible();
    // workspaceSummaryFixture.root_path = "/var/.../CogniCode" → basename = "CogniCode"
    await expect(name).toContainText("CogniCode");

    // Graph status indicator is present
    const status = page.getByTestId("landing-graph-status");
    await expect(status).toBeVisible();
  });

  test("P1.7 Landing error state when fetch fails", async ({ page }) => {
    // Override the landing endpoint to return 500
    await page.route("**/api/workspaces/*/landing**", async (route) => {
      await route.fulfill({
        status: 500,
        contentType: "application/json",
        body: JSON.stringify({ error: "Internal server error" }),
      });
    });

    await page.goto("/");

    // Error state replaces the canvas
    const error = page.getByTestId("graph-landing-error");
    await expect(error).toBeVisible({ timeout: 15_000 });
  });

  test("P1.8 Landing loading state during fetch", async ({ page }) => {
    // Add a deliberate delay to the landing endpoint so the loading
    // state is observable.
    await page.route("**/api/workspaces/*/landing**", async (route) => {
      await new Promise((resolve) => setTimeout(resolve, 2000));
      await route.continue();
    });

    await page.goto("/");

    // Loading state is visible immediately (before the 2s delay completes)
    const loading = page.getByTestId("graph-landing-loading");
    await expect(loading).toBeVisible({ timeout: 5_000 });

    // Eventually resolves to the actual landing
    await expect(loading).toBeHidden({ timeout: 10_000 });
    await expect(page.getByTestId("graph-landing")).toBeVisible();
  });
});
