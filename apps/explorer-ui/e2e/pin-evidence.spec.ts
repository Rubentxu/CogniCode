import { test, expect } from "@playwright/test";

test.describe("Pin Evidence — E21-2", () => {
  test("pins evidence to an investigation", async ({ page }) => {
    // NOTE: This test assumes the backend is seeded with:
    // 1. A workspace with ingested graph
    // 2. At least one investigation in "active" or "draft" status
    // Use MSW mocks for deterministic tests.

    await page.goto("/");

    // Wait for the workspace to load
    await expect(page.getByTestId("loading-shell")).not.toBeVisible({ timeout: 10000 });

    // Navigate to a symbol to have an object to pin
    await page.getByTestId("spotter-trigger").click();
    const spotterInput = page.getByTestId("spotter-input");
    await spotterInput.fill("UserService");
    await page.getByRole("option").first().click();

    // Wait for the pane to render
    await expect(page.getByTestId("object-inspector")).toBeVisible({ timeout: 10000 });

    // Click the Pin button
    const pinButton = page.getByTestId("pin-evidence-button");
    await expect(pinButton).toBeVisible();
    await pinButton.click();

    // Modal should appear
    const modal = page.getByTestId("pin-evidence-modal");
    await expect(modal).toBeVisible();

    // Verify object context is shown
    const objectIdField = page.locator('[data-testid="pin-evidence-modal-panel"] p[title]');
    await expect(objectIdField).toBeVisible();
    const objectId = await objectIdField.getAttribute("title");
    expect(objectId).toMatch(/symbol:.+/);

    // Investigation dropdown should be visible
    const investigationSelect = page.getByTestId("pin-evidence-investigation");
    await expect(investigationSelect).toBeVisible();

    // Select an investigation (if not pre-selected)
    const selectedOption = await investigationSelect.inputValue();
    if (!selectedOption) {
      await investigationSelect.selectOption({ index: 0 });
    }

    // Type a note
    const noteTextarea = page.getByTestId("pin-evidence-note");
    await noteTextarea.fill("This function is critical for user authentication flow");

    // Submit button should be enabled
    const submitButton = page.getByTestId("pin-evidence-submit");
    await expect(submitButton).not.toBeDisabled();
    await submitButton.click();

    // Success state should appear
    const successMessage = page.getByTestId("pin-evidence-success");
    await expect(successMessage).toBeVisible({ timeout: 5000 });
    await expect(successMessage).toContainText("Evidence pinned successfully");

    // Modal should auto-close after success display
    await expect(modal).not.toBeVisible({ timeout: 2000 });
  });

  test("disables submit when note is empty", async ({ page }) => {
    await page.goto("/");

    await expect(page.getByTestId("loading-shell")).not.toBeVisible({ timeout: 10000 });

    await page.getByTestId("spotter-trigger").click();
    const spotterInput = page.getByTestId("spotter-input");
    await spotterInput.fill("UserService");
    await page.getByRole("option").first().click();

    await expect(page.getByTestId("object-inspector")).toBeVisible({ timeout: 10000 });

    // Open modal
    await page.getByTestId("pin-evidence-button").click();
    await expect(page.getByTestId("pin-evidence-modal")).toBeVisible();

    // Submit button should be disabled with empty note
    const submitButton = page.getByTestId("pin-evidence-submit");
    await expect(submitButton).toBeDisabled();

    // Type note - button should enable
    const noteTextarea = page.getByTestId("pin-evidence-note");
    await noteTextarea.fill("Test note");
    await expect(submitButton).not.toBeDisabled();

    // Clear note - button should disable again
    await noteTextarea.fill("");
    await expect(submitButton).toBeDisabled();
  });

  test("shows error message when no active investigations", async ({ page }) => {
    // NOTE: Requires MSW mock that returns empty investigations list

    await page.goto("/");

    await expect(page.getByTestId("loading-shell")).not.toBeVisible({ timeout: 10000 });

    await page.getByTestId("spotter-trigger").click();
    const spotterInput = page.getByTestId("spotter-input");
    await spotterInput.fill("UserService");
    await page.getByRole("option").first().click();

    await expect(page.getByTestId("object-inspector")).toBeVisible({ timeout: 10000 });

    // Open modal
    await page.getByTestId("pin-evidence-button").click();
    await expect(page.getByTestId("pin-evidence-modal")).toBeVisible();

    // Should show error message about no investigations
    const errorMessage = page.getByText("No active or draft investigations found");
    await expect(errorMessage).toBeVisible();
  });

  test("closes on Escape key", async ({ page }) => {
    await page.goto("/");

    await expect(page.getByTestId("loading-shell")).not.toBeVisible({ timeout: 10000 });

    await page.getByTestId("spotter-trigger").click();
    const spotterInput = page.getByTestId("spotter-input");
    await spotterInput.fill("UserService");
    await page.getByRole("option").first().click();

    await expect(page.getByTestId("object-inspector")).toBeVisible({ timeout: 10000 });

    // Open modal
    await page.getByTestId("pin-evidence-button").click();
    await expect(page.getByTestId("pin-evidence-modal")).toBeVisible();

    // Press Escape
    await page.keyboard.press("Escape");

    // Modal should close
    await expect(page.getByTestId("pin-evidence-modal")).not.toBeVisible();
  });

  test("closes on backdrop click", async ({ page }) => {
    await page.goto("/");

    await expect(page.getByTestId("loading-shell")).not.toBeVisible({ timeout: 10000 });

    await page.getByTestId("spotter-trigger").click();
    const spotterInput = page.getByTestId("spotter-input");
    await spotterInput.fill("UserService");
    await page.getByRole("option").first().click();

    await expect(page.getByTestId("object-inspector")).toBeVisible({ timeout: 10000 });

    // Open modal
    await page.getByTestId("pin-evidence-button").click();
    await expect(page.getByTestId("pin-evidence-modal")).toBeVisible();

    // Click backdrop (the modal overlay, not the panel)
    const modalBackdrop = page.getByTestId("pin-evidence-modal");
    const boundingBox = await modalBackdrop.boundingBox();
    if (boundingBox) {
      await page.mouse.click(boundingBox.x + 10, boundingBox.y + 10);
    }

    // Modal should close
    await expect(page.getByTestId("pin-evidence-modal")).not.toBeVisible();
  });
});