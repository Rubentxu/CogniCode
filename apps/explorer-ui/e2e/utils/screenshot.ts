/**
 * Screenshot helper — central `snapshot(page, name)` for E2E coverage.
 *
 * All E2E specs in this cycle (e17-e2e-coverage-audit) use this helper so
 * screenshot options stay consistent: full-page, animations disabled,
 * small pixel-diff tolerance for CI runner variance.
 *
 * Why these options:
 *
 * - `fullPage: true` — captures the entire scrollable area, not just the
 *   viewport. Useful for long landing pages and pane stacks.
 * - `animations: "disabled"` — pauses CSS transitions/animations so the
 *   screenshot is deterministic. Without this, CI runners drift
 *   ~5-10px between runs due to RAF timing.
 * - `maxDiffPixels: 50` — tolerates minor anti-aliasing differences
 *   between runners without losing strictness for real regressions.
 * - `caret: "hide"` — hides text caret so typing-in-progress doesn't
 *   change the screenshot between runs.
 *
 * Usage:
 *
 * ```ts
 * import { snapshot } from "./utils/screenshot";
 *
 * test("renders overview", async ({ page }) => {
 *   await page.goto("/");
 *   await snapshot(page, "overview.png");
 * });
 * ```
 */
import { expect, type Page, type PageScreenshotOptions } from "@playwright/test";

const DEFAULT_OPTS: PageScreenshotOptions = {
  animations: "disabled",
  caret: "hide",
  fullPage: true,
  maxDiffPixels: 50,
};

/**
 * Capture a screenshot with consistent options.
 *
 * The file is written to `apps/explorer-ui/e2e/<spec-name>.spec.ts-snapshots/<name>.png`
 * and used as the baseline for subsequent runs.
 */
export async function snapshot(page: Page, name: string): Promise<void> {
  await expect(page).toHaveScreenshot(name, DEFAULT_OPTS);
}

/**
 * Capture a screenshot with extra options override.
 *
 * Use this when you need to deviate from defaults — for example, to
 * capture a single element rather than the full page.
 */
export async function snapshotWith(
  page: Page,
  name: string,
  overrides: Partial<PageScreenshotOptions>,
): Promise<void> {
  await expect(page).toHaveScreenshot(name, { ...DEFAULT_OPTS, ...overrides });
}
