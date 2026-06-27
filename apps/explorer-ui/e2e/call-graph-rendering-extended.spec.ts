/**
 * E2E: Call-graph interaction (SVG render + node click).
 *
 * GToolkit parity: Mondrian / graph views are interactive. Clicking a
 * node opens a new pane (GtPager-style). The SVG canvas supports
 * pan/zoom via mouse drag.
 */
import { test, expect } from "@playwright/test";
import { snapshot } from "./utils/screenshot";

test.describe("Call-graph interaction (Mondrian parity)", () => {
  test("call-graph tab renders SVG with nodes (not blank)", async ({ page }) => {
    await page.goto("/");
    await page.waitForTimeout(500);
    await page.keyboard.press("Meta+k");
    const spotter = page.getByTestId("spotter");
    await expect(spotter).toBeVisible();
    const input = page.getByTestId("spotter-input");
    await input.fill("build");
    const firstHit = page
      .getByTestId("spotter-results")
      .locator('[data-family="symbol"]')
      .first();
    await expect(firstHit).toBeVisible({ timeout: 5_000 });
    await firstHit.click();
    await expect(spotter).toBeHidden();

    // Click the call-graph tab.
    const callGraphTab = page.getByTestId("view-tab-call-graph");
    await expect(callGraphTab).toBeVisible();
    await callGraphTab.click();

    // SVG canvas visible.
    const svg = page.getByTestId("svg-graph-canvas");
    await expect(svg).toBeVisible({ timeout: 5_000 });

    // At least one node.
    const nodes = page.locator('[data-testid^="graph-node-"]');
    const nodeCount = await nodes.count();
    expect(nodeCount).toBeGreaterThan(0);

    await snapshot(page, "call-graph-extended-render.png");
  });

  test.skip("clicking a graph node opens a new pane [DEBT: no data-pane-on-node attribute]", async ({
    page,
  }) => {
    // Graph node click handler is wired (per the inventory) but lacks a
    // data-testid or assertion surface for E2E. The pane-stack-drilldown
    // spec covers the equivalent flow via Spotter; re-enable this spec
    // when graph nodes expose an assertion-friendly attribute.
    await page.goto("/");
    // ... drill into call graph and click a node ...
  });
});
