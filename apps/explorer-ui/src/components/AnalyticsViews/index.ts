/**
 * Public surface for the D3-powered analytic views.
 *
 * `HotspotTreemap` and `DeadCodeSunburst` are mounted by
 * `LensPanel` when the user picks the hotspots or dead-code lens.
 * Other consumers should treat them as plain presentational
 * widgets — they emit no events and own no state.
 */
export { HotspotTreemap } from "./HotspotTreemap";
export type { HotspotTreemapProps } from "./HotspotTreemap";
export { DeadCodeSunburst } from "./DeadCodeSunburst";
export type { DeadCodeSunburstProps } from "./DeadCodeSunburst";
export type { TreemapData, SunburstData } from "./types";
