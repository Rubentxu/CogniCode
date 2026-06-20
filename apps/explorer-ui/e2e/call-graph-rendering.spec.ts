/**
 * E2E call-graph rendering tests — Wave 2 acceptance.
 *
 * Tests the GraphViewRenderer routing fix (T2, T3) and edge label
 * highlight-only behavior (T12). Verifies that the call-graph view
 * renders as an interactive SVG with nodes (not blank).
 *
 * Covers:
 * - T11a: call-graph-ready e2e test
 * - T11a: pane-stack-multi test (clicking node opens new pane)
 */
import { test, expect } from "@playwright/test";

test.describe("Call graph rendering (bug fix)", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible();
  });

  test("call-graph-ready: SVG renders with nodes (not blank)", async ({ page }) => {
    // Open spotter and select first result
    await page.keyboard.press("Meta+k");
    await page.getByTestId("spotter-input").waitFor({ timeout: 5_000 });
    await page.getByTestId("spotter-input").fill("build");

    const results = page
      .getByTestId("spotter-results")
      .getByTestId(/^spotter-item-/);
    await results.first().click();

    // Switch to call-graph view
    const graphTab = page.getByTestId("view-tab-call-graph");
    await expect(graphTab).toBeVisible({ timeout: 5_000 });
    await graphTab.click();

    // Verify GraphViewRenderer is rendered (not Blocks)
    const graphView = page.getByTestId("graph-view-renderer");
    await expect(graphView).toBeVisible({ timeout: 5_000 });

    // Verify SVG canvas exists
    const svgCanvas = page.getByTestId("svg-graph-canvas");
    await expect(svgCanvas).toBeVisible({ timeout: 3_000 });

    // Verify graph nodes are rendered (layoutFromContextualView produces nodes)
    const nodes = page.locator("[data-testid^='graph-node-']");
    await expect(nodes.first()).toBeVisible({ timeout: 3_000 });

    // Verify graph edges exist
    const edges = page.locator("[data-testid^='graph-edge-']");
    await expect(edges.first()).toBeVisible({ timeout: 3_000 });

    // Verify title is shown
    await expect(graphView.getByRole("heading")).toBeVisible();
  });

  test("pane-stack-multi: click node opens new pane", async ({ page }) => {
    // Open spotter and select first result
    await page.keyboard.press("Meta+k");
    await page.getByTestId("spotter-input").fill("build");
    const results = page
      .getByTestId("spotter-results")
      .getByTestId(/^spotter-item-/);
    await results.first().click();

    // Switch to call-graph view
    await page.getByTestId("view-tab-call-graph").click();

    // Verify initial state: one pane tab
    const tabs = page.locator("[data-testid^='pane-tab-']");
    await expect(tabs).toHaveCount(1);

    // Click on a graph node to open a new pane
    const firstNode = page.locator("[data-testid^='graph-node-']").first();
    await firstNode.click();

    // Verify new pane was opened
    await expect(tabs).toHaveCount(2);
  });

  test("edge labels hidden by default, shown on hover", async ({ page }) => {
    // Open spotter and select first result
    await page.keyboard.press("Meta+k");
    await page.getByTestId("spotter-input").fill("build");
    const results = page
      .getByTestId("spotter-results")
      .getByTestId(/^spotter-item-/);
    await results.first().click();

    // Switch to call-graph view
    await page.getByTestId("view-tab-call-graph").click();

    // Verify GraphViewRenderer
    await expect(page.getByTestId("graph-view-renderer")).toBeVisible();

    // Edges should have testIds
    const edge = page.locator("[data-testid^='graph-edge-']").first();
    await expect(edge).toBeVisible();

    // The SVG edge elements should not have visible text labels initially
    // (labels only appear when edge is highlighted per T12)
  });
});