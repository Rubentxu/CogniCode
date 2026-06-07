/**
 * E2E responsive test — Phase 11 acceptance criterion 11.4.
 *
 * Verifies the Shell layout adapts to the three breakpoints
 * (desktop ≥ 1200px, tablet 900–1199px, small < 900px). The Shell
 * exposes the resolved viewport through `data-viewport` so we can
 * assert on it without measuring pixels.
 *
 * The Spotter + the inspector + the column navigation must keep
 * working at every viewport (no overlap, no horizontal scroll on
 * the body, no clipped buttons).
 */
import { test, expect, type Page } from "@playwright/test";

const BREAKPOINTS = [
  { name: "desktop", width: 1280, height: 800, expectedViewport: "desktop" },
  { name: "tablet", width: 1024, height: 800, expectedViewport: "tablet" },
  { name: "small", width: 768, height: 900, expectedViewport: "small" },
] as const;

async function primeApp(page: Page) {
  await page.goto("/");
  await expect(page.getByTestId("shell")).toBeVisible();
  // Drive the app into a state that exercises all three panels.
  await page.keyboard.press("Meta+k");
  await page.getByTestId("spotter-input").fill("build");
  const firstResult = page
    .getByTestId("spotter-results")
    .getByTestId(/^spotter-item-/);
  await firstResult.first().click();
  // The ObjectInspector is only present at desktop/tablet. At
  // small viewport the Miller Columns take the full pane — so
  // the presence of the inspector is breakpoint-dependent. The
  // column-list is always present.
  await expect(
    page
      .getByTestId("miller-columns")
      .or(page.getByTestId("object-inspector")),
  ).toBeVisible();
}

for (const bp of BREAKPOINTS) {
  test.describe(`viewport: ${bp.name} (${bp.width}x${bp.height})`, () => {
    test.use({ viewport: { width: bp.width, height: bp.height } });

    test(`Shell resolves to ${bp.expectedViewport}`, async ({ page }) => {
      await page.goto("/");
      const shell = page.getByTestId("shell");
      await expect(shell).toBeVisible();
      await expect(shell).toHaveAttribute("data-viewport", bp.expectedViewport);
    });

    test("the main flow holds at this viewport", async ({ page }) => {
      await primeApp(page);
      // At desktop + tablet the inspector is in the right pane;
      // at small the columns take the full pane and the inspector
      // is a drill-down (not present in the DOM until navigated).
      if (bp.expectedViewport !== "small") {
        await expect(page.getByTestId("object-inspector-body")).toBeVisible();
      } else {
        // Small viewport — assert the columns are populated and
        // no horizontal overflow happens (asserted in the next test).
        await expect(page.getByTestId("miller-columns")).toBeVisible();
      }
      // At desktop the lens panel is always on; at tablet it is
      // togglable; at small it is hidden behind the toggle. We
      // assert the lens overlay is reachable in tablet mode.
      if (bp.expectedViewport === "tablet") {
        const lensToggle = page.getByRole("button", {
          name: /Open lens panel/i,
        });
        await expect(lensToggle).toBeVisible();
      }
    });

    test("no horizontal overflow on the body", async ({ page }) => {
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
    });
  });
}
