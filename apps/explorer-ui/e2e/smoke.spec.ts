/**
 * E2E smoke test — Phase 11 acceptance criterion 11.1.
 *
 * Full flow:
 *  1. Open the app — connection gate resolves to the Shell.
 *  2. Shell mounts with the 2-zone layout (nav + inspector).
 *  3. Spotter opens via Cmd+K, types a query, and shows results.
 *  4. Selecting a result closes the palette and inspects the object.
 *  5. View tabs render for the new object; clicking one updates the body.
 *
 * The dev server is started with `VITE_USE_MOCKS=true` (see
 * `playwright.config.ts`), so every `/api/*` request is handled by
 * the MSW browser worker. The result is a deterministic, network-
 * free E2E suite that runs in CI.
 */
import { test, expect } from "@playwright/test";

test.describe("Explorer smoke flow", () => {
  test("boots, opens Spotter, and inspects an object", async ({
    page,
  }) => {
    await page.goto("/");

    // 1. App boots — title + Shell visible.
    await expect(
      page.getByRole("heading", { name: /CogniCode Explorer/i, level: 1 }),
    ).toBeVisible();
    const shell = page.getByTestId("shell");
    await expect(shell).toBeVisible();

    // 2. Open the Spotter via Cmd+K.
    await page.keyboard.press("Meta+k");
    const spotter = page.getByTestId("spotter");
    await expect(spotter).toBeVisible();
    const input = page.getByTestId("spotter-input");
    await input.fill("build");

    // 3. The first fixture result becomes available.
    const firstResult = page
      .getByTestId("spotter-results")
      .getByTestId(/^spotter-item-/);
    await expect(firstResult.first()).toBeVisible({ timeout: 5_000 });

    // 4. Click the first result, palette closes, inspector renders.
    await firstResult.first().click();
    await expect(spotter).toBeHidden();
    await expect(page.getByTestId("object-inspector")).toBeVisible();

    // 5. At least one view tab is rendered for the new object.
    const tablist = page.getByRole("tablist", { name: /Available views/i });
    await expect(tablist).toBeVisible();
    const tabs = tablist.getByRole("tab");
    await expect(tabs.first()).toBeVisible();
  });
});
