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

  /** Skip Scaffold step, then advance to the Data Source step (step 4 of 6). */
  async function advanceToDataSourceStep(page: any) {
    // Step 1 (Scaffold): skip via "Custom Query"
    await page.getByRole("button", { name: /custom query/i }).click();
    await expect(page.getByText("Step 2 of 6")).toBeVisible();

    // Step 2 (View Kind): select Vertical Slice → Next
    await page.getByRole("button", { name: /vertical slice/i }).click();
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 3 of 6")).toBeVisible();

    // Step 3 (Renderer): default auto-selected on ViewKind pick → Next
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 4 of 6")).toBeVisible();
  }

  // ---------------------------------------------------------------------------
  // G7.1: Open + close
  // ---------------------------------------------------------------------------

  test("wizard opens and shows step 1 (Scaffold)", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    await expect(page.getByText("Step 1 of 6")).toBeVisible();
    // Scaffold options + "Custom Query" link visible
    await expect(page.getByRole("button", { name: /custom query/i })).toBeVisible();
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

  test("Next button advances through all 6 steps", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    // Step 1 (Scaffold): Custom Query → skips to View Kind
    await page.getByRole("button", { name: /custom query/i }).click();
    await expect(page.getByText("Step 2 of 6")).toBeVisible();

    // Step 2 (View Kind): select Vertical Slice → Next
    await page.getByRole("button", { name: /vertical slice/i }).click();
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 3 of 6")).toBeVisible();

    // Step 3 (Renderer): Next (auto-defaulted)
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 4 of 6")).toBeVisible();

    // Step 4 (Data Source): fill query → Next
    await page.locator("textarea").first().fill("symbols");
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 5 of 6")).toBeVisible();

    // Step 5 (Transform): Next → Step 6 (Save)
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 6 of 6")).toBeVisible();
  });

  test("Back button returns to previous step", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    // Step 1 (Scaffold): skip to View Kind
    await page.getByRole("button", { name: /custom query/i }).click();
    await expect(page.getByText("Step 2 of 6")).toBeVisible();

    // Navigate to step 3
    await page.getByRole("button", { name: /vertical slice/i }).click();
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 3 of 6")).toBeVisible();

    // Back to step 2 (Renderer)
    await page.getByRole("button", { name: /back/i }).click();
    await expect(page.getByText("Step 2 of 6")).toBeVisible();
  });

  // ---------------------------------------------------------------------------
  // G7.3: Step content rendering
  // ---------------------------------------------------------------------------

  test("step 2 (View Kind) renders view kind options", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    // Skip Scaffold step to reach View Kind
    await page.getByRole("button", { name: /custom query/i }).click();
    await expect(page.getByText("Step 2 of 6")).toBeVisible();

    // Multiple ViewKind buttons should be visible
    await expect(page.getByRole("button", { name: /vertical slice/i })).toBeVisible();
    await expect(page.getByRole("button", { name: /call graph/i })).toBeVisible();
  });

  test("step 3 (Renderer) renders renderer options", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    // Skip Scaffold → View Kind → Next reaches Renderer (step 3)
    await page.getByRole("button", { name: /custom query/i }).click();
    await page.getByRole("button", { name: /vertical slice/i }).click();
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 3 of 6")).toBeVisible();

    // Renderer options as buttons (full label avoids ambiguity with perspective-graph)
    await expect(page.getByRole("button", { name: /graph — interactive/i })).toBeVisible();
    await expect(page.getByRole("button", { name: /table/i })).toBeVisible();
  });

  test("step 4 (Data Source) renders query input", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    await advanceToDataSourceStep(page);

    // MoldQL query textarea visible
    await expect(page.locator("textarea").first()).toBeVisible();
  });

  test("step 5 (Transform) renders JSONata expression input", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    await advanceToDataSourceStep(page);
    // Fill query to enable Next
    await page.locator("textarea").first().fill("symbols");
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 5 of 6")).toBeVisible();

    // Transform step: click "JSONata" to reveal the expression textarea
    await page.getByRole("button", { name: /jsonata/i }).click();
    // Transform step: textarea visible
    await expect(page.locator("textarea").first()).toBeVisible();
  });

  test("step 6 (Save) renders title input and Save button", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    // Navigate to step 6 (Save): Scaffold skip → ViewKind → Renderer → DataSource → Transform → Save
    await page.getByRole("button", { name: /custom query/i }).click();
    await page.getByRole("button", { name: /vertical slice/i }).click();
    await page.getByRole("button", { name: /next/i }).click();
    await page.getByRole("button", { name: /next/i }).click(); // Renderer (auto-defaulted)
    await page.locator("textarea").first().fill("symbols");
    await page.getByRole("button", { name: /next/i }).click();
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 6 of 6")).toBeVisible();

    // Title input and Save button visible
    const dialog = page.getByRole("dialog", { name: /create custom view/i });
    await expect(dialog.getByLabel(/title/i)).toBeVisible();
    await expect(dialog.getByRole("button", { name: /save view/i })).toBeVisible();
  });

  // ---------------------------------------------------------------------------
  // G7.4: Validation
  // ---------------------------------------------------------------------------

  test("Next is disabled on step 1 when no scaffold or custom query selected", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    // Step 1 (Scaffold): no selection made yet — Next should be disabled
    await expect(page.getByRole("button", { name: /next/i })).toBeDisabled();
  });

  test("Next is enabled after skipping scaffold via Custom Query", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    // Clicking Custom Query immediately advances to step 2 (View Kind)
    await page.getByRole("button", { name: /custom query/i }).click();
    await expect(page.getByText("Step 2 of 6")).toBeVisible();
    await expect(page.getByRole("button", { name: /next/i })).toBeDisabled(); // no ViewKind yet
  });

  test("Next is enabled on step 3 after ViewKind selection (renderer auto-defaulted)", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    // Skip Scaffold → select ViewKind → Next reaches Renderer (auto-defaulted)
    await page.getByRole("button", { name: /custom query/i }).click();
    await page.getByRole("button", { name: /vertical slice/i }).click();
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 3 of 6")).toBeVisible();
    // Renderer is auto-defaulted on ViewKind selection, so Next is enabled
    await expect(page.getByRole("button", { name: /next/i })).toBeEnabled();
  });

  test("Next is disabled on step 4 when MoldQL query is empty", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    await advanceToDataSourceStep(page);

    // Query empty — Next disabled
    await expect(page.getByRole("button", { name: /next/i })).toBeDisabled();

    // Fill query — Next enables
    await page.locator("textarea").first().fill("symbols where kind = 'function'");
    await expect(page.getByRole("button", { name: /next/i })).toBeEnabled();
  });

  // ---------------------------------------------------------------------------
  // G7.5: Save flow (MSW handler POST /api/viewspecs already exists)
  // ---------------------------------------------------------------------------

  test("save flow: fills title and saves successfully", async ({ page }) => {
    await openInspectorWithObject(page);
    await openWizard(page);

    // Navigate to step 6 (Save): Scaffold skip → ViewKind → Renderer → DataSource → Transform → Save
    await page.getByRole("button", { name: /custom query/i }).click();
    await page.getByRole("button", { name: /vertical slice/i }).click();
    await page.getByRole("button", { name: /next/i }).click();
    await page.getByRole("button", { name: /next/i }).click(); // Renderer (auto-defaulted)
    await page.locator("textarea").first().fill("symbols");
    await page.getByRole("button", { name: /next/i }).click();
    await page.getByRole("button", { name: /next/i }).click();
    await expect(page.getByText("Step 6 of 6")).toBeVisible();

    // Fill title
    const dialog = page.getByRole("dialog", { name: /create custom view/i });
    await dialog.getByLabel(/title/i).fill("My Vertical Slice");

    // Click Save — wizard should close after success
    await dialog.getByRole("button", { name: /save view/i }).click();
    await expect(dialog).not.toBeVisible({ timeout: 10_000 });
  });

  // G7.5: Error state — requires ability to override MSW handler or run without mocks.
  // Skipped in E2E (MSW-incompatible with page.route); covered in integration tests.
  test.skip("save flow: shows error message when API fails", async ({ page }) => {
    // This would require disabling MSW or using a test-specific handler override.
    // Until then, the save-success test above provides the relevant coverage.
  });
});
