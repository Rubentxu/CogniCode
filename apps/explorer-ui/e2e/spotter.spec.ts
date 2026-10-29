/**
 * E2E spotter tests.
 *
 * Verifies the Spotter (cmd-K palette) end-to-end behavior: open, search,
 * result groups, escape-to-close, empty state.
 *
 * All tests rely on MSW handlers (VITE_USE_MOCKS=true).
 * The Spotter component lives at apps/explorer-ui/src/components/Spotter.tsx.
 *
 * Phase 4 scenarios (5 tests):
 *   P4.1 Open via Cmd+K
 *   P4.2 Open via search button
 *   P4.3 Results grouped by kind
 *   P4.4 Close with Escape
 *   P4.5 Empty state when no results
 */
import { test, expect } from "@playwright/test";

import {
  GLOBAL_SHORTCUT_ATTRIBUTE,
  GLOBAL_SHORTCUT_READY,
} from "../src/components/Spotter/globalShortcut";
import { waitForGlobalShortcut } from "./utils/globalShortcut";

test.describe("Phase 4: Spotter (5 tests)", () => {
  test("P4.1 Open via Cmd+K", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // The application reports the shortcut as registered.
    await waitForGlobalShortcut(page);

    // Press Cmd+K
    await page.keyboard.press("Meta+k");

    // The Spotter modal opens
    await expect(page.getByTestId("spotter")).toBeVisible({ timeout: 5_000 });
    await expect(page.getByTestId("spotter-input")).toBeFocused();
  });

  test("P4.2 Open via search button", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Click the Spotter trigger button in the header
    const trigger = page.getByTestId("spotter-trigger");
    await expect(trigger).toBeVisible();
    await trigger.click();

    // The Spotter modal opens
    await expect(page.getByTestId("spotter")).toBeVisible({ timeout: 5_000 });
  });

  test("P4.3 Results grouped by kind", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Open the Spotter
    await waitForGlobalShortcut(page);
    await page.keyboard.press("Meta+k");
    await expect(page.getByTestId("spotter")).toBeVisible({ timeout: 5_000 });

    // Type a query that matches multiple kinds
    const input = page.getByTestId("spotter-input");
    await input.fill("build");

    // Wait for results
    const results = page.getByTestId("spotter-results");
    await expect(results).toBeVisible({ timeout: 5_000 });

    // At least one result item
    const items = page.locator("[data-testid^='spotter-item-']");
    await expect(items.first()).toBeVisible({ timeout: 5_000 });
    expect(await items.count()).toBeGreaterThan(0);

    // The count badge shows the total
    const count = page.getByTestId("spotter-count");
    await expect(count).toBeVisible();
  });

  test("P4.4 Close with Escape", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Open the Spotter
    await waitForGlobalShortcut(page);
    await page.keyboard.press("Meta+k");
    await expect(page.getByTestId("spotter")).toBeVisible({ timeout: 5_000 });

    // Press Escape
    await page.keyboard.press("Escape");

    // The Spotter closes
    await expect(page.getByTestId("spotter")).toBeHidden({ timeout: 5_000 });
  });

  test("P4.5 Empty state when no results", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // Open the Spotter
    await waitForGlobalShortcut(page);
    await page.keyboard.press("Meta+k");
    await expect(page.getByTestId("spotter")).toBeVisible({ timeout: 5_000 });

    // Type a query that won't match anything
    const input = page.getByTestId("spotter-input");
    await input.fill("zzzzzzzzz_no_match_xyz_qqq");

    // Wait for the empty state
    await expect(page.getByTestId("spotter-empty")).toBeVisible({ timeout: 5_000 });

    // No result items are shown
    const items = page.locator("[data-testid^='spotter-item-']");
    expect(await items.count()).toBe(0);
  });
});

/**
 * The availability signal of the global shortcut.
 *
 * The suite used to wait for the keyboard listener with a fixed
 * clock, which is a guess about how long the application takes. The
 * application now publishes a fact — an attribute on `<html>` that is
 * present exactly while the listener is registered — and the suite
 * waits for that fact.
 */
test.describe("Global shortcut availability signal", () => {
  test("the signal is published and the shortcut then opens the palette", async ({
    page,
  }) => {
    await page.goto("/");
    await expect(page.getByTestId("shell")).toBeVisible({ timeout: 10_000 });

    // The application reports the shortcut as registered.
    await waitForGlobalShortcut(page);
    await expect(page.locator("html")).toHaveAttribute(
      GLOBAL_SHORTCUT_ATTRIBUTE,
      GLOBAL_SHORTCUT_READY,
    );

    // And the shortcut really answers.
    await page.keyboard.press("Meta+k");
    await expect(page.getByTestId("spotter")).toBeVisible({ timeout: 5_000 });
  });

  test("the wait fails when the signal arrives later than the deadline", async ({
    page,
  }) => {
    // The application boots late: the signal cannot exist before the
    // deadline the wait is given, so the wait must fail — because the
    // signal is missing, not because a clock ran out.
    await page.route("**/src/main.tsx", async (route) => {
      await new Promise((resolve) => setTimeout(resolve, 3000));
      await route.continue();
    });

    await page.goto("/", { waitUntil: "commit" });

    await expect(waitForGlobalShortcut(page, 1_000)).rejects.toThrow();

    // The signal was genuinely absent — the wait did not pass on a
    // missing assertion.
    expect(
      await page.locator("html").getAttribute(GLOBAL_SHORTCUT_ATTRIBUTE),
    ).toBeNull();

    // The same page satisfies the same wait once the deadline allows
    // for the late arrival.
    await waitForGlobalShortcut(page, 15_000);
  });
});
