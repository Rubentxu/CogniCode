/**
 * `kindDefaultView` — picks the best default viewId for a given object kind.
 *
 * Used by Spotter to decide which view to open when the user picks a
 * result via Enter / click / chip. Mappings per E18-2 spec:
 *
 *   - symbol             → call-graph (drill into calls)
 *   - route|use_case|event → vertical_slice (full flow)
 *   - file|scope         → overview (broad picture first)
 *   - decision_artifact|evidence → evidence (pin it)
 *   - other (workspace, module, quality_issue, rule) → overview
 *
 * Falls back to "overview" for unknown kinds.
 */
import type { InspectableObjectType } from "./types";

const KIND_TO_DEFAULT_VIEW: Readonly<Record<InspectableObjectType, string>> = {
  symbol: "call-graph",
  route: "vertical_slice",
  use_case: "vertical_slice",
  event: "vertical_slice",
  file: "overview",
  scope: "overview",
  decision_artifact: "evidence",
  evidence: "evidence",
  workspace: "overview",
  module: "overview",
  quality_issue: "quality",
  rule: "quality",
};

/**
 * Pure lookup — no React state needed. Call at render time.
 */
export function kindDefaultView(kind: InspectableObjectType | null | undefined): string {
  if (!kind) return "overview";
  return KIND_TO_DEFAULT_VIEW[kind] ?? "overview";
}
