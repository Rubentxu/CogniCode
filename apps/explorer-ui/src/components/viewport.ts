/**
 * `viewport.ts` — viewport breakpoint classification for the Shell.
 *
 * The Explorer Shell has three layouts:
 * - `desktop` (≥ 1200px): 3-column grid
 * - `tablet`  (900 – 1199px): 2-column grid + lens overlay
 * - `small`   (< 900px): single-column drill-down
 *
 * Centralised here so Playwright specs and the Shell share the
 * same boundary logic.
 */
export type ShellViewport = "small" | "tablet" | "desktop";

const BREAKPOINTS = {
  tablet: 900,
  desktop: 1200,
} as const;

export function detectViewport(width: number): ShellViewport {
  if (width >= BREAKPOINTS.desktop) return "desktop";
  if (width >= BREAKPOINTS.tablet) return "tablet";
  return "small";
}
