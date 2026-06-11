/**
 * `viewport.ts` — viewport breakpoint classification for the Shell.
 *
 * The Explorer Shell has four layouts:
 * - `ultrawide` (≥ 1440px): 4-column grid, the 4th column hosts
 *   the live `InteractiveGraph`.
 * - `desktop` (1200 – 1439px): 3-column grid (no live graph).
 * - `tablet`  (900 – 1199px): 2-column grid + lens overlay.
 * - `small`   (< 900px): single-column drill-down.
 *
 * Centralised here so Playwright specs and the Shell share the
 * same boundary logic.
 */
export type ShellViewport = "small" | "tablet" | "desktop" | "ultrawide";

const BREAKPOINTS = {
  tablet: 900,
  desktop: 1200,
  ultrawide: 1440,
} as const;

export function detectViewport(width: number): ShellViewport {
  if (width >= BREAKPOINTS.ultrawide) return "ultrawide";
  if (width >= BREAKPOINTS.desktop) return "desktop";
  if (width >= BREAKPOINTS.tablet) return "tablet";
  return "small";
}
