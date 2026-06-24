/**
 * `LensResultView` — renders the result of an applied lens using the
 * best visualization for the data type.
 *
 * For `lens.hotspots` it shows a D3 treemap; for `lens.dead-code`
 * it shows a D3 sunburst; for all other lenses it returns `null`
 * so the caller can render the default grouped findings view.
 *
 * NOTE: The backend currently returns `LensResult` with a flat
 * `DesignFinding[]`. The D3 components expect hierarchical
 * `TreemapData` / `SunburstData` shapes. Until the backend supplies
 * those shapes, this component uses representative mock fixtures
 * so the wiring is in place and tested. The fixtures are scoped to
 * this component so the data-shape mismatch is isolated.
 */
import type { JSX } from "react";
import { HotspotTreemap } from "../AnalyticsViews/HotspotTreemap";
import { DeadCodeSunburst } from "../AnalyticsViews/DeadCodeSunburst";
import type { DesignFinding } from "../../api/types";
import type { SunburstData, SunburstLeaf, TreemapData, TreemapLeaf } from "../AnalyticsViews/types";

// -----------------------------------------------------------------------------
// Mock fixtures — replaced by real backend data in a follow-up PR
// -----------------------------------------------------------------------------

/**
 * Mock data for the hotspots treemap. Each cell has a `value`
 * (call count) and a `complexity` score in 0..1 (colour ramp input).
 */
const HOTSPOT_TREEMAP_FIXTURE: TreemapData = {
  name: "crates/cognicode-explorer/src",
  children: [
    { name: "build_overview", value: 140, complexity: 0.92 },
    { name: "spotter", value: 96, complexity: 0.74 },
    { name: "save_exploration", value: 72, complexity: 0.58 },
    { name: "resolve_path", value: 60, complexity: 0.46 },
    { name: "render_block", value: 48, complexity: 0.31 },
    { name: "fetch_object", value: 36, complexity: 0.22 },
    { name: "merge_blocks", value: 24, complexity: 0.14 },
    { name: "format_label", value: 18, complexity: 0.08 },
  ] satisfies TreemapLeaf[],
};

/**
 * Mock data for the dead-code sunburst. `alive === false` segments
 * render in the warm palette so dead code visually pops.
 */
const DEAD_CODE_SUNBURST_FIXTURE: SunburstData = {
  name: "crates/cognicode-explorer/src",
  children: [
    { name: "api", size: 120, alive: true },
    { name: "db", size: 90, alive: true },
    { name: "lib", size: 80, alive: true },
    { name: "view_block", size: 64, alive: true },
    { name: "legacy_parser", size: 48, alive: false },
    { name: "compat_shim", size: 32, alive: false },
    { name: "unused_format", size: 20, alive: false },
    { name: "orphan_helper", size: 12, alive: false },
  ] satisfies SunburstLeaf[],
};

// -----------------------------------------------------------------------------
// LensResultView
// -----------------------------------------------------------------------------

export interface LensResultViewProps {
  lensId: string;
  result: { findings: DesignFinding[]; summary: string } | null;
}

export function LensResultView({
  lensId,
  result,
}: LensResultViewProps): JSX.Element | null {
  // Apply the active scope as the root label so the user can tell
  // cells apart when navigating between scopes.
  const rootLabel = result?.summary ?? "scope";

  if (lensId === "lens.hotspots") {
    const data: TreemapData = {
      ...HOTSPOT_TREEMAP_FIXTURE,
      name: rootLabel,
    };
    return <HotspotTreemap data={data} />;
  }

  if (lensId === "lens.dead-code") {
    const data: SunburstData = {
      ...DEAD_CODE_SUNBURST_FIXTURE,
      name: rootLabel,
    };
    return <DeadCodeSunburst data={data} />;
  }

  // Fallback: the caller (`LensFindingsView`) renders grouped findings.
  return null;
}
