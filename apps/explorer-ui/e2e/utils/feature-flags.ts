/**
 * Feature flags — query whether a feature is supported.
 *
 * Used by E2E specs to skip cleanly when a feature is documented as debt
 * (not yet implemented) instead of failing the test.
 *
 * Flags come from three sources:
 *
 * 1. **ADR-002 capability debt list** — `composed_narrative`, `project_diary`,
 *    `example_object`, `concept_map`, `evidence_pack`, `risk_map`,
 *    `decision_trace`, `doc_code_alignment`.
 * 2. **Cycle-specific debt notes** — e.g. LensPanel uses mock fixtures.
 * 3. **Feature inventory gaps** — features without an implementation path.
 *
 * Usage:
 *
 * ```ts
 * import { isFeatureSupported } from "./utils/feature-flags";
 *
 * test.skip(!isFeatureSupported("composed_narrative"), "debt: catalog-only ViewKind");
 * ```
 */
import { test } from "@playwright/test";

export type FeatureFlag =
  // Moldable Development patterns (ADR-002 Phase 3)
  | "composed_narrative"
  | "project_diary"
  | "example_object"
  // Catalog-only ViewKinds (ADR-002 §3 capability debt)
  | "concept_map"
  | "evidence_pack"
  | "risk_map"
  | "decision_trace"
  | "doc_code_alignment"
  // Cycle-specific debt
  | "lens_real_data"
  | "contextual_editor_lsp"
  | "playgrounds"
  | "driller_shortcuts";

const DEBT_FEATURES = new Set<FeatureFlag>([
  "composed_narrative",
  "project_diary",
  "example_object",
  "concept_map",
  "evidence_pack",
  "risk_map",
  "decision_trace",
  "doc_code_alignment",
  "lens_real_data",
  "contextual_editor_lsp",
  "playgrounds",
  "driller_shortcuts",
]);

/**
 * Returns true if the feature is currently implemented (not in debt).
 */
export function isFeatureSupported(name: FeatureFlag): boolean {
  return !DEBT_FEATURES.has(name);
}

/**
 * Skip a test if a feature is in debt. Use as a guard in test bodies:
 *
 * ```ts
 * test("composed_narrative view renders", async ({ page }) => {
 *   skipIfDebt("composed_narrative", "ADR-002 Phase 3 — narrative runtime not yet implemented");
 *   // ... test body ...
 * });
 * ```
 */
export function skipIfDebt(name: FeatureFlag, reason: string): void {
  if (!isFeatureSupported(name)) {
    test.skip(true, `[DEBT] ${name}: ${reason}`);
  }
}
