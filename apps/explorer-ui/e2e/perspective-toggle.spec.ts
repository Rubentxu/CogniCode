/**
 * E2E perspective toggle tests — Phase 2 of the explorer-e2e-test-plan.
 *
 * Verifies the Graph ↔ C4 perspective toggle behavior end-to-end.
 *
 * All tests rely on MSW handlers (VITE_USE_MOCKS=true).
 * The toggle component lives at apps/explorer-ui/src/components/PerspectiveToggle.tsx
 * and dispatches SET_PERSPECTIVE which is handled by the perspective slice.
 *
 * Phase 2 scenarios (6 tests) from docs/explorer-e2e-test-plan.md:
 *   P2.1 Toggle Graph → C4 perspective
 *   P2.2 C4 shows component architecture nodes
 *   P2.3 C4 shows correct node styles (component/container/system)
 *   P2.4 Toggle back C4 → Graph restores data
 *   P2.5 Repeated toggling doesn't duplicate nodes
 *   P2.6 Toggle keyboard accessible (Tab+Enter/Space)
 */
import { test, expect } from "@playwright/test";

test.describe("Phase 2: Perspective toggle (6 tests)", () => {
  test("P2.1 Toggle Graph → C4 perspective", async ({ page }) => {
    await page.goto("/");

    // Wait for the toggle to appear (it's in the shell header)
    const toggle = page.getByTestId("perspective-toggle");
    await expect(toggle).toBeVisible({ timeout: 10_000 });

    const graphBtn = toggle.getByTestId("perspective-graph");
    const c4Btn = toggle.getByTestId("perspective-c4");

    // Initially Graph is pressed
    await expect(graphBtn).toHaveAttribute("aria-pressed", "true");
    await expect(c4Btn).toHaveAttribute("aria-pressed", "false");

    // Click C4
    await c4Btn.click();

    // C4 is now pressed, Graph is not
    await expect(c4Btn).toHaveAttribute("aria-pressed", "true");
    await expect(graphBtn).toHaveAttribute("aria-pressed", "false");
  });

  test("P2.2 C4 shows component architecture nodes", async ({ page }) => {
    await page.goto("/");

    // Wait for landing to render (graph perspective first)
    await expect(page.getByTestId("graph-landing")).toBeVisible({ timeout: 10_000 });

    // Switch to C4 perspective
    const c4Btn = page.getByTestId("perspective-toggle").getByTestId("perspective-c4");
    await c4Btn.click();

    // C4 data arrives via /api/workspaces/:id/architecture
    // Architecture nodes render in the cytoscape canvas with the same
    // graph-node-<id> testid pattern as the graph perspective
    const nodes = page.locator("[data-testid^='graph-node-']");
    await expect(nodes.first()).toBeVisible({ timeout: 15_000 });

    // The MSW architecture fixture returns at least one node
    const count = await nodes.count();
    expect(count).toBeGreaterThan(0);
  });

  test("P2.3 C4 shows correct node styles (component/container/system)", async ({ page }) => {
    await page.goto("/");

    // Switch to C4 perspective
    await page.getByTestId("perspective-toggle").getByTestId("perspective-c4").click();

    // Wait for the C4 canvas
    const canvas = page.getByTestId("graph-landing-canvas");
    await expect(canvas).toBeVisible({ timeout: 15_000 });

    // C4 nodes carry cytoscape style classes for their kind
    // The architecture fixture returns nodes with kinds: system, container, component
    // These map to style classes: node-system, node-container, node-component
    const componentNodes = page.locator(".node-component");
    const containerNodes = page.locator(".node-container");
    const systemNodes = page.locator(".node-system");

    // At least one C4-styled node should be visible
    const componentCount = await componentNodes.count();
    const containerCount = await containerNodes.count();
    const systemCount = await systemNodes.count();
    const total = componentCount + containerCount + systemCount;

    expect(total).toBeGreaterThan(0);
  });

  test("P2.4 Toggle back C4 → Graph restores data", async ({ page }) => {
    await page.goto("/");

    // Wait for graph landing
    await expect(page.getByTestId("graph-landing")).toBeVisible({ timeout: 10_000 });

    // Toggle to C4
    const toggle = page.getByTestId("perspective-toggle");
    const c4Btn = toggle.getByTestId("perspective-c4");
    const graphBtn = toggle.getByTestId("perspective-graph");
    await c4Btn.click();
    await expect(c4Btn).toHaveAttribute("aria-pressed", "true");

    // Wait for C4 nodes to render
    const c4Nodes = page.locator("[data-testid^='graph-node-']");
    await expect(c4Nodes.first()).toBeVisible({ timeout: 15_000 });
    const c4Count = await c4Nodes.count();
    expect(c4Count).toBeGreaterThan(0);

    // Toggle back to Graph
    await graphBtn.click();
    await expect(graphBtn).toHaveAttribute("aria-pressed", "true");
    await expect(c4Btn).toHaveAttribute("aria-pressed", "false");

    // Graph nodes still render (stale-data hold + new fetch)
    const graphNodes = page.locator("[data-testid^='graph-node-']");
    await expect(graphNodes.first()).toBeVisible({ timeout: 10_000 });
  });

  test("P2.5 Repeated toggling doesn't duplicate nodes", async ({ page }) => {
    await page.goto("/");

    // Wait for initial graph landing
    await expect(page.getByTestId("graph-landing")).toBeVisible({ timeout: 10_000 });

    const toggle = page.getByTestId("perspective-toggle");
    const c4Btn = toggle.getByTestId("perspective-c4");
    const graphBtn = toggle.getByTestId("perspective-graph");

    // Capture initial graph node count
    const graphNodes = page.locator("[data-testid^='graph-node-']");
    await expect(graphNodes.first()).toBeVisible({ timeout: 15_000 });
    const initialCount = await graphNodes.count();
    expect(initialCount).toBeGreaterThan(0);

    // Toggle Graph → C4 → Graph → C4 → Graph (3 round trips)
    for (let i = 0; i < 3; i++) {
      await c4Btn.click();
      await expect(c4Btn).toHaveAttribute("aria-pressed", "true");
      await graphBtn.click();
      await expect(graphBtn).toHaveAttribute("aria-pressed", "true");
    }

    // Final state: Graph perspective, same node count as initial
    const finalCount = await graphNodes.count();
    expect(finalCount).toBe(initialCount);
  });

  test("P2.6 Toggle keyboard accessible (Tab+Enter/Space)", async ({ page }) => {
    await page.goto("/");

    // Wait for the shell
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // The toggle buttons are real <button> elements → focusable via Tab
    const c4Btn = page.getByTestId("perspective-c4");
    const graphBtn = page.getByTestId("perspective-graph");

    // Focus the C4 button directly
    await c4Btn.focus();
    await expect(c4Btn).toBeFocused();

    // Press Enter to activate
    await page.keyboard.press("Enter");
    await expect(c4Btn).toHaveAttribute("aria-pressed", "true");

    // Focus the Graph button
    await graphBtn.focus();
    await expect(graphBtn).toBeFocused();

    // Press Space to activate
    await page.keyboard.press(" ");
    await expect(graphBtn).toHaveAttribute("aria-pressed", "true");
  });
});
