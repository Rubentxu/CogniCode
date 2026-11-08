/**
 * `kindDefaultView` — picks the best default viewId for a given object kind.
 *
 * Used by Spotter to decide which view to open when the user picks a
 * result via Enter / click / chip. Mappings per E18-2 spec:
 *
 *   - symbol             → call-graph (drill into calls)
 *   - route              → vertical_slice (full flow)
 *   - file|scope         → overview (broad picture first)
 *   - doc                → doc_code_alignment (E23: Doc/Code alignment)
 *   - decision_artifact  → doc_code_alignment (E23: Doc/Code alignment)
 *   - evidence           → evidence (pin it)
 *   - adr                → adr-source (ADR markdown source)
 *   - workspace|module   → overview
 *   - quality_issue|rule → quality
 *   - investigation      → overview (default; no dedicated view kind yet)
 *   - saved_exploration  → overview
 *
 * Falls back to "overview" for unknown kinds.
 */
import type { InspectableObjectType } from "./types";

const KIND_TO_DEFAULT_VIEW: Readonly<Record<InspectableObjectType, string>> = {
  symbol: "call-graph",
  route: "vertical_slice",
  file: "overview",
  scope: "seam_map",
  doc: "doc_code_alignment",
  decision_artifact: "doc_code_alignment",
  evidence: "evidence",
  workspace: "overview",
  module: "seam_map",
  quality_issue: "quality",
  rule: "quality",
  investigation: "overview",
  adr: "adr-source",
  saved_exploration: "overview",
};

/**
 * Pure lookup — no React state needed. Call at render time.
 *
 * Accepts a plain string because the wire `SpotterResult.object_type` is
 * not narrowed to the enum; anything outside the table falls back.
 */
export function kindDefaultView(kind: string | null | undefined): string {
  if (!kind || !Object.hasOwn(KIND_TO_DEFAULT_VIEW, kind)) return "overview";
  return KIND_TO_DEFAULT_VIEW[kind as InspectableObjectType];
}
