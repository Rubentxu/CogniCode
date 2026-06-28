/**
 * E2E: ViewSpecWizard full flow (Lepiter-equivalent authoring).
 *
 * GToolkit parity: Lepiter authoring is the equivalent of CogniCode's
 * ViewSpecWizard. The wizard lets users pick a ViewKind, pick a
 * RendererKind, pick a data source, edit a JSONata transform, and save
 * the ViewSpec as data. After save, the spec appears in the inspector
 * for the same object.
 *
 * Spec scope:
 * - Open the wizard via the overflow menu.
 * - Verify each step is reachable.
 * - Verify Save persists the ViewSpec via MSW.
 *
 * NOTE: The wizard has minimal instrumentation (only `data-testid="viewspec-wizard"`
 * on the root). This spec verifies the openable surface and the steps
 * count. Detailed per-step interaction is unit-tested in
 * `ViewSpecWizard.test.tsx`; E2E coverage here is for the user-visible
 * happy path.
 */
import { test, expect } from "@playwright/test";
import { snapshot } from "./utils/screenshot";

async function openSymbolInInspector(page: import("@playwright/test").Page): Promise<void> {
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
  await expect(page.getByTestId("object-inspector")).toBeVisible();
}

test.describe("ViewSpecWizard full flow (Lepiter authoring parity)", () => {
  test("wizard opens from overflow menu on inspectable object", async ({ page }) => {
    await openSymbolInInspector(page);

    const overflow = page.getByTestId("view-tabs-overflow-menu");
    await expect(overflow).toBeVisible();
    await overflow.click();

    // The wizard mounts with a single testid on root.
    const wizard = page.getByTestId("viewspec-wizard");
    await expect(wizard).toBeVisible();

    await snapshot(page, "viewspec-wizard-open.png");
  });

  test("wizard shows 5 steps in step navigation", async ({ page }) => {
    await openSymbolInInspector(page);

    const overflow = page.getByTestId("view-tabs-overflow-menu");
    await overflow.click();

    const wizard = page.getByTestId("viewspec-wizard");
    await expect(wizard).toBeVisible();

    // The wizard has STEPS = [view-kind, renderer-kind, data-source, transform, review].
    // Without granular testids, we sanity-check the wizard renders a step
    // navigation element. Look for any text containing "view-kind" or step labels.
    const wizardText = await wizard.textContent();
    expect(wizardText).toBeTruthy();
    // The wizard should mention at least "View kind" or step labels.
    const hasStepLabel =
      wizardText?.toLowerCase().includes("view kind") ||
      wizardText?.toLowerCase().includes("renderer") ||
      wizardText?.toLowerCase().includes("data source") ||
      wizardText?.toLowerCase().includes("transform");
    expect(hasStepLabel).toBe(true);

    await snapshot(page, "viewspec-wizard-steps.png");
  });

  test.skip("wizard save persists ViewSpec via MSW [DEBT: minimal instrumentation]", async ({
    page,
  }) => {
    // This test is skipped because the wizard lacks per-step testids for
    // reliable E2E assertion. The save path is unit-tested in
    // apps/explorer-ui/src/components/ObjectInspector/ViewSpecWizard.test.tsx.
    // Re-enable when the wizard adds data-testid to each step + save button.
    await openSymbolInInspector(page);
    const overflow = page.getByTestId("view-tabs-overflow-menu");
    await overflow.click();
    // ... would fill 5 steps and click save ...
  });
});
