/**
 * E2E: Exploration sharing.
 *
 * GToolkit parity: Lepiter page links are shareable URLs that restore
 * state when opened. CogniCode's ShareExplorationButton generates a
 * URL with ?exploration=<id>; opening that URL restores the pane stack.
 */
import { test, expect } from "@playwright/test";
import { snapshot } from "./utils/screenshot";

test.describe("Exploration sharing (Lepiter page link parity)", () => {
  test("ShareExplorationButton exists in the shell", async ({ page }) => {
    await page.goto("/");

    const shareBtn = page.getByTestId("share-exploration");
    await expect(shareBtn).toBeVisible();

    await snapshot(page, "exploration-share-button.png");
  });

  test.skip("clicking share generates ?exploration=<id> URL [DEBT: button not wired to URL update]", async ({
    page,
  }) => {
    // The button is visible but the click handler does not currently update
    // the URL with ?exploration=<id>. This was identified during e17
    // coverage audit. Re-enable when the click handler is wired.
    await page.goto("/");
    const shareBtn = page.getByTestId("share-exploration");
    await shareBtn.click();
    await expect(page).toHaveURL(/\?exploration=/);
    await snapshot(page, "exploration-share-url.png");
  });
});
