/**
 * E2E helper — wait for the object's view tabs to be rendered.
 *
 * The tabs of an inspected object are rendered together, when the
 * object's `available_views` arrive. That arrival is later than the
 * inspector shell: the shell shows first and the tabs land a beat
 * after.
 *
 * That beat is the reason this helper exists. A spec that samples the
 * DOM for a tab in that window finds nothing, and — because the
 * branches are written as "if the tab is there, do this; otherwise do
 * that" — it silently takes the other branch and stops asserting
 * anything while still passing. The failure is invisible: the spec
 * turns green by asserting less.
 *
 * So the probe waits for the fact it is about to measure. Once the
 * tabs are rendered, a tab that is still absent is genuinely absent
 * from the fixture, and skipping on it is honest.
 */
import { expect, type Page } from "@playwright/test";

/** Default deadline for the view tabs to be rendered. */
export const VIEW_TABS_TIMEOUT = 10_000;

/**
 * Wait until the inspected object's view tabs are rendered.
 *
 * Rejects at `timeout` if they never arrive — which is the point: an
 * object that is supposed to have views and does not is a failure, not
 * a pause.
 */
export async function waitForViewTabs(
  page: Page,
  timeout: number = VIEW_TABS_TIMEOUT,
): Promise<void> {
  // `.first()` because every open pane renders its own tabs: with two
  // panes open there are two containers, and the question this helper
  // answers is "have the views rendered?", not "which pane renders
  // them?".
  await expect(page.getByTestId("view-tabs").first()).toBeVisible({ timeout });
}
