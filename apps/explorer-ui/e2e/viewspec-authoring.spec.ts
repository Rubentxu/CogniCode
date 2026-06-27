/**
 * E2E tests for ViewSpec authoring wizard (G7).
 *
 * Covers:
 * - Wizard opens via "+ Custom View" button in ViewTabs
 * - All 5 steps visible in the wizard
 * - Step content renders for each step
 * - Next button advances through steps (after selecting required fields)
 * - Back button returns to previous step
 * - Validation: Next disabled without required fields
 *
 * Note: Full save (POST/PUT /api/viewspecs) requires MSW handlers
 * not yet implemented — save flow is tested up to step navigation.
 */
import { test, expect } from "@playwright/test";

test.describe("ViewSpec Wizard (G7)", () => {
  // ---------------------------------------------------------------------------
  // Helpers
  // ---------------------------------------------------------------------------

  /** Open the app and select a symbol so the wizard trigger appears. */
  async function openInspectorWithObject(page: any) {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible();
    await page.waitForTimeout(1500);

    const modifier = process.platform === "linux" ? "Control" : "Meta";
    await page.keyboard.press(`${modifier}+k`);
    const input = page.getByTestId("spotter-input");
    await expect(input).toBeVisible({ timeout: 5_000 });
    await input.fill("build");
    await page.waitForTimeout(800);

    const firstResult = page
      .getByTestId("spotter-results")
      .getByTestId(/^spotter-item-/);
    await expect(firstResult.first()).toBeVisible({ timeout: 5_000 });
    await firstResult.first().click();

    await expect(page.getByTestId("object-inspector")).toBeVisible();
  }

  /** Open the wizard via the "+ Custom View" button. */
  async function openWizard(page: any) {
    await page.getByTestId("view-tabs-overflow-menu").click();
    await expect(
      page.getByRole("dialog", { name: /create custom view/i })
    ).toBeVisible({ timeout: 5_000 });
  }

  /** Navigate steps 1-2 by clicking ViewKind + Renderer. */
  async function advanceToStep3(page: any) {
    // Step 1: select a ViewKind
    await page.getByRole("button", { name: /vertical slice/i }).click();
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 2 of 5")).toBeVisible();

    // Step 2: select a RendererKind (use full label to avoid ambiguity with perspective-graph)
    await page.getByRole("button", { name: /graph — interactive/i }).click();
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 3 of 5")).toBeVisible();
  }

  // ---------------------------------------------------------------------------
  // G7.1: Open + close
  // ---------------------------------------------------------------------------

  test("wizard opens and shows step 1 (View Kind)", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    await expect(page.getByText("Step 1 of 5")).toBeVisible();
    // View kind options render as buttons
    await expect(page.getByRole("button", { name: /vertical slice/i })).toBeVisible();
  });

  test("wizard closes via ✕ button", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    const dialog = page.getByRole("dialog", { name: /create custom view/i });
    await dialog.getByRole("button", { name: /close/i }).click();
    await expect(dialog).not.toBeVisible();
  });

  // ---------------------------------------------------------------------------
  // G7.2: Step navigation
  // ---------------------------------------------------------------------------

  test("Next button advances through all 5 steps", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    // Step 1: select ViewKind → Next
    await page.getByRole("button", { name: /vertical slice/i }).click();
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 2 of 5")).toBeVisible();

    // Step 2: select RendererKind → Next
    await page.getByRole("button", { name: /graph — interactive/i }).click();
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 3 of 5")).toBeVisible();

    // Step 3: fill query → Next
    await page.locator("textarea").first().fill("symbols");
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 4 of 5")).toBeVisible();

    // Step 4: Next → Step 5
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 5 of 5")).toBeVisible();
  });

  test("Back button returns to previous step", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    // Navigate to step 2
    await page.getByRole("button", { name: /vertical slice/i }).click();
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 2 of 5")).toBeVisible();

    // Back to step 1
    await page.getByRole("button", { name: /back/i }).click();
    await expect(page.getByText("Step 1 of 5")).toBeVisible();
  });

  // ---------------------------------------------------------------------------
  // G7.3: Step content rendering
  // ---------------------------------------------------------------------------

  test("step 1 (View Kind) renders view kind options", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    // Multiple ViewKind buttons should be visible
    await expect(page.getByRole("button", { name: /vertical slice/i })).toBeVisible();
    await expect(page.getByRole("button", { name: /call graph/i })).toBeVisible();
  });

  test("step 2 (Renderer) renders renderer options", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    // Step 1 → 2
    await page.getByRole("button", { name: /vertical slice/i }).click();
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 2 of 5")).toBeVisible();

    // Renderer options as buttons (full label avoids ambiguity with perspective-graph)
    await expect(page.getByRole("button", { name: /graph — interactive/i })).toBeVisible();
    await expect(page.getByRole("button", { name: /table/i })).toBeVisible();
  });

  test("step 3 (Data Source) renders query input", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    await advanceToStep3(page);

    // MoldQL query textarea visible
    await expect(page.locator("textarea").first()).toBeVisible();
  });

  test("step 4 (Transform) renders JSONata expression input", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    await advanceToStep3(page);
    // Fill query to enable Next
    await page.locator("textarea").first().fill("symbols");
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 4 of 5")).toBeVisible();

    // Transform step: click "JSONata" to reveal the expression textarea
    await page.getByRole("button", { name: /jsonata/i }).click();
    // Transform step: textarea visible
    await expect(page.locator("textarea").first()).toBeVisible();
  });

  test("step 5 (Save) renders title input and Save button", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    // Navigate to step 5
    await page.getByRole("button", { name: /vertical slice/i }).click();
    await page.getByRole("button", { name: /next/i }).click();
    await page.getByRole("button", { name: /graph — interactive/i }).click();
    await page.getByRole("button", { name: /next/i }).click();
    await page.locator("textarea").first().fill("symbols");
    await page.getByRole("button", { name: /next/i }).click();
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 5 of 5")).toBeVisible();

    // Title input and Save button visible
    const dialog = page.getByRole("dialog", { name: /create custom view/i });
    await expect(dialog.getByLabel(/title/i)).toBeVisible();
    await expect(dialog.getByRole("button", { name: /save view/i })).toBeVisible();
  });

  // ---------------------------------------------------------------------------
  // G7.4: Validation
  // ---------------------------------------------------------------------------

  test("Next is disabled on step 1 when no view kind is selected", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    // No ViewKind selected — Next should be disabled
    await expect(page.getByRole("button", { name: /next/i })).toBeDisabled();
  });

  test("Next is enabled on step 2 after ViewKind selection (renderer auto-defaulted)", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    // Select ViewKind → Next → step 2
    // Renderer is auto-defaulted on ViewKind selection, so Next is enabled
    await page.getByRole("button", { name: /vertical slice/i }).click();
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 2 of 5")).toBeVisible();
    await expect(page.getByRole("button", { name: /next/i })).toBeEnabled();
  });

  test("Next is disabled on step 3 when MoldQL query is empty", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    await advanceToStep3(page);

    // Query empty — Next disabled
    await expect(page.getByRole("button", { name: /next/i })).toBeDisabled();

    // Fill query — Next enables
    await page.locator("textarea").first().fill("symbols where kind = 'function'");
    await expect(page.getByRole("button", { name: /next/i })).toBeEnabled();
  });
});
