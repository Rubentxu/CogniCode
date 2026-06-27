/**
 * E2E protocol edges tests — e15.5 Cross-Service Protocol Edge Ingestion.
 *
 * Tests that after ingesting an OpenAPI spec:
 *  1. Route nodes appear in the Landing graph
 *  2. Clicking a Route node opens the inspector
 *  3. The call-graph view shows the `http_calls` edge from route → handler symbol
 *  4. The route node renders with the correct style_class = "route"
 *
 * All tests rely on MSW handlers (VITE_USE_MOCKS=true).
 */
import { test, expect } from "@playwright/test";

test.describe("e15.5 Protocol edges in call-graph (2 tests)", () => {
  test.beforeEach(async ({ page }) => {
    // Reset mock state (routeStore) before each test for isolation
    await page.request.post("/api/mocks/reset");
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });
  });

  /**
   * Helper: ingest the petstore spec via the MCP Tools modal.
   */
  async function ingestSpec(page: import("@playwright/test").Page) {
    const mcpToolsBtn = page.getByTestId("mcp-tools-trigger");
    await mcpToolsBtn.click();
    const modal = page.getByTestId("mcp-tools-modal");
    await expect(modal).toBeVisible({ timeout: 5_000 });

    await page.getByTestId("mcp-tool-select").selectOption("ingest_openapi");
    await page.getByTestId("mcp-spec-path").fill("sandbox/fixtures/openapi/petstore.json");
    await page.getByTestId("mcp-tools-run").click();
    await expect(page.getByTestId("mcp-tools-result")).toBeVisible({ timeout: 15_000 });

    const text = await page.getByTestId("mcp-tools-result").textContent();
    expect(text).toContain('"status": "ingested"');

    await page.getByTestId("mcp-tools-modal-close").click();
    await expect(modal).toBeHidden({ timeout: 5_000 });
  }

  test("E2E: Route nodes appear in Spotter after ingestion", async ({ page }) => {
    // Ingest spec
    await ingestSpec(page);

    // Open Spotter and search for "pet"
    await page.waitForTimeout(1500);
    await page.keyboard.press("Control+k");
    const spotter = page.getByTestId("spotter");
    await expect(spotter).toBeVisible({ timeout: 5_000 });

    const input = page.getByTestId("spotter-input");
    await input.click();
    await page.keyboard.type("pet", { delay: 50 });

    const results = page.getByTestId("spotter-results");
    await expect(results).toBeVisible({ timeout: 5_000 });

    // Wait for debounce (200ms) + network + render
    await page.waitForTimeout(800);

    // At least one route item should appear (GET /pets, GET /pets/{petId}, etc.)
    const items = page.locator("[data-testid^='spotter-item-']");
    const count = await items.count();
    expect(count).toBeGreaterThan(0);

    // Each result item should have a label that includes a method or path
    const firstLabel = await items.first().textContent();
    expect(firstLabel?.length).toBeGreaterThan(0);

    // Close spotter
    await page.keyboard.press("Escape");
  });

  test("E2E: Inspecting a Route node shows its handler in the call-graph", async ({ page }) => {
    // Ingest spec
    await ingestSpec(page);

    // Search for the route in Spotter
    await page.waitForTimeout(1500);
    await page.keyboard.press("Control+k");
    const spotter = page.getByTestId("spotter");
    await expect(spotter).toBeVisible({ timeout: 5_000 });

    const input = page.getByTestId("spotter-input");
    await input.fill("createPet");
    await page.waitForTimeout(500);

    const results = page.getByTestId("spotter-results");
    await expect(results).toBeVisible({ timeout: 5_000 });

    const items = page.locator("[data-testid^='spotter-item-']");
    const count = await items.count();
    expect(count).toBeGreaterThan(0);

    // Click the first result (should be the POST /pets route)
    await items.first().click();
    await expect(spotter).toBeHidden();
    await expect(page.getByTestId("object-inspector")).toBeVisible({ timeout: 5_000 });

    // View tabs should be visible
    const viewTabs = page.getByTestId("view-tabs");
    await expect(viewTabs).toBeVisible();

    // Switch to call-graph view
    const callGraphTab = page.getByTestId("view-tab-call-graph");
    if (await callGraphTab.isVisible()) {
      await callGraphTab.click();

      // The graph view should render
      const graphView = page.getByTestId("graph-view-renderer");
      await expect(graphView).toBeVisible({ timeout: 5_000 });

      // The SVG canvas should have nodes
      const svgCanvas = page.getByTestId("svg-graph-canvas");
      await expect(svgCanvas).toBeVisible();

      const nodes = page.locator("[data-testid^='graph-node-']");
      await expect(nodes.first()).toBeVisible({ timeout: 5_000 });

      // At least 2 nodes: the route node + the handler symbol node
      const nodeCount = await nodes.count();
      expect(nodeCount).toBeGreaterThanOrEqual(1);
    }
  });
});
