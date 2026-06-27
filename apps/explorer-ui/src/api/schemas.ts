/**
 * Zod schemas for every backend DTO.
 *
 * Mirrors `crates/cognicode-explorer/src/dto.rs`. The discriminated
 * union for `ViewBlock` covers all 27 known block shapes emitted by
 * the view builders, plus a permissive fallback so future shapes
 * still surface in the UI as "raw JSON" instead of breaking parse.
 *
 * Every schema validates at the API boundary — components downstream
 * only see typed data. The fallback block keeps the system forward-
 * compatible without a new release for every new block kind.
 */
import { z } from "zod";

// ============================================================================
// Enums
// ============================================================================

export const graphStatusSchema = z.enum([
  "missing",
  "stale",
  "ready",
  "indexing",
]);
export type GraphStatus = z.infer<typeof graphStatusSchema>;

export const inspectableObjectTypeSchema = z.enum([
  "workspace",
  "scope",
  "symbol",
  "file",
  "module",
  "evidence",
  "decision_artifact",
  "quality_issue",
  "rule",
  "route",
]);
export type InspectableObjectType = z.infer<typeof inspectableObjectTypeSchema>;

export const relationDirectionSchema = z.enum(["incoming", "outgoing"]);
export type RelationDirection = z.infer<typeof relationDirectionSchema>;

export const findingSeveritySchema = z.enum(["info", "warning", "critical"]);
export type FindingSeverity = z.infer<typeof findingSeveritySchema>;

export const artifactFormatSchema = z.enum(["markdown", "html", "json_replay"]);
export type ArtifactFormat = z.infer<typeof artifactFormatSchema>;

// ============================================================================
// Primitives
// ============================================================================

export const lineRangeSchema = z.object({
  start: z.number().int().nonnegative(),
  end: z.number().int().nonnegative(),
});
export type LineRange = z.infer<typeof lineRangeSchema>;

/**
 * `value` is a free-form JSON blob — the backend stores arbitrary
 * scalars / arrays / objects. The UI narrows per-block when it knows
 * the expected shape (see ViewBlock below).
 */
export const propertySchema = z.object({
  key: z.string(),
  value: z.unknown(),
  value_type: z.string(),
  source: z.string(),
});
export type Property = z.infer<typeof propertySchema>;

// ============================================================================
// Identity & evidence
// ============================================================================

export const objectIdentityEntrySchema = z.object({
  id: z.string(),
  object_type: z.string(),
  natural_key: z.string(),
  first_seen: z.string(),
});
export type ObjectIdentityEntry = z.infer<typeof objectIdentityEntrySchema>;

/**
 * Backend uses `serde_json::Value` for `body` to keep block emission
 * flexible. We type the boundary with `unknown` and narrow per-block
 * via the discriminated union — no schema in this file is more
 * permissive than the wire shape.
 */
export const evidenceBlockSchema = z.object({
  id: z.string(),
  kind: z.string(),
  title: z.string(),
  file: z.string().nullable(),
  line_range: lineRangeSchema.nullable(),
  source_tool_or_query: z.string(),
  confidence: z.number().nullable(),
  freshness: z.string().nullable().optional(),
});
export type EvidenceBlock = z.infer<typeof evidenceBlockSchema>;

export const typedRelationSchema = z.object({
  relation_type: z.string(),
  direction: relationDirectionSchema,
  target_object_id: z.string(),
  target_label: z.string(),
  evidence_ids: z.array(z.string()),
});
export type TypedRelation = z.infer<typeof typedRelationSchema>;

// ============================================================================
// Top-level DTOs
// ============================================================================

export const workspaceSummarySchema = z.object({
  id: z.string(),
  root_path: z.string(),
  graph_status: graphStatusSchema,
  indexed_at: z.string().nullable(),
  symbol_count: z.number().int().nonnegative(),
  relation_count: z.number().int().nonnegative(),
  last_scan_at: z.string().nullable(),
});
export type WorkspaceSummary = z.infer<typeof workspaceSummarySchema>;

export const viewDescriptorSchema = z.object({
  id: z.string(),
  title: z.string(),
  /** Whether this is a built-in view (true) or runtime user-defined view (false). */
  is_builtin: z.boolean().default(true),
  /** Source discriminator for runtime views: "runtime" for user-defined specs. */
  source: z.string().nullable().default(null),
});
export type ViewDescriptor = z.infer<typeof viewDescriptorSchema>;

export const inspectableObjectSummarySchema = z.object({
  id: z.string(),
  object_type: inspectableObjectTypeSchema,
  label: z.string(),
  subtitle: z.string(),
  properties: z.array(propertySchema),
  available_views: z.array(viewDescriptorSchema),
});
export type InspectableObjectSummary = z.infer<
  typeof inspectableObjectSummarySchema
>;

export const spotterResultSchema = z.object({
  object: inspectableObjectSummarySchema,
  score: z.number(),
  match_type: z.string(),
});
export type SpotterResult = z.infer<typeof spotterResultSchema>;

// ============================================================================
// Quality / Findings / Lenses
// ============================================================================

/**
 * `issue_summary` shape — used inside *_issues blocks and as a
 * standalone quality-issue descriptor in the rule detail block.
 */
export const qualityIssueItemSchema = z.object({
  id: z.number().int().positive(),
  rule_id: z.string(),
  severity: z.string(),
  category: z.string(),
  file: z.string(),
  line: z.number().int().nonnegative(),
  message: z.string(),
  status: z.string(),
  object_id: z.string(),
});
export type QualityIssueItem = z.infer<typeof qualityIssueItemSchema>;

/**
 * Quality gate snapshot — surfaced in `file_quality_gate` and
 * `scope_quality_gate` blocks. Mirrors `QualityGateSummary` in Rust.
 */
export const qualityGateSummarySchema = z.object({
  rating: z.string().nullable(),
  total_issues: z.number().int().nonnegative(),
  blockers: z.number().int().nonnegative(),
  criticals: z.number().int().nonnegative(),
  debt_minutes: z.number().int().nonnegative(),
  last_run: z.string().nullable(),
});
export type QualityGateSummary = z.infer<typeof qualityGateSummarySchema>;

export const designFindingSchema = z.object({
  id: z.string(),
  lens_id: z.string(),
  title: z.string(),
  hypothesis: z.string(),
  severity: findingSeveritySchema,
  confidence: z.number().min(0).max(1),
  object_ids: z.array(z.string()),
  evidence_ids: z.array(z.string()),
});
export type DesignFinding = z.infer<typeof designFindingSchema>;

/**
 * Severity used inside quality blocks — broader than `FindingSeverity`.
 * Quality blocks emit a per-rule violation with one of these labels;
 * the dashboard buckets them into the A-E rating.
 */
export const qualitySeveritySchema = z.enum([
  "blocker",
  "critical",
  "major",
  "minor",
  "info",
]);
export type QualitySeverity = z.infer<typeof qualitySeveritySchema>;

export const lensDescriptorSchema = z.object({
  id: z.string(),
  name: z.string(),
  description: z.string(),
  applicable_types: z.array(inspectableObjectTypeSchema),
});
export type LensDescriptor = z.infer<typeof lensDescriptorSchema>;

export const lensResultSchema = z.object({
  lens_id: z.string(),
  findings: z.array(designFindingSchema),
  summary: z.string(),
});
export type LensResult = z.infer<typeof lensResultSchema>;

// ============================================================================
// View blocks — discriminated union
// ============================================================================

/**
 * Helper to constrain the shared `id` + `title` + `body` shell around
 * a typed body. Discriminator is the `id` field.
 */
const blockShell = <TId extends string, TBody extends z.ZodTypeAny>(
  id: TId,
  body: TBody,
) =>
  z.object({
    id: z.literal(id),
    title: z.string(),
    body,
  });

// --- Symbol overview / call graph ---

const identityBodySchema = z.object({
  name: z.string(),
  kind: z.string(),
  file: z.string(),
  line: z.number().int().nonnegative(),
});
export type IdentityBlockBody = z.infer<typeof identityBodySchema>;

const callMetricsBodySchema = z.object({
  fan_in: z.number().int().nonnegative(),
  fan_out: z.number().int().nonnegative(),
});
export type CallMetricsBlockBody = z.infer<typeof callMetricsBodySchema>;

const signatureBodySchema = z.object({
  signature: z.string(),
});
export type SignatureBlockBody = z.infer<typeof signatureBodySchema>;

/**
 * Shared shape for both `callers` and `callees` blocks — a count and
 * a list of `relation_summary` items. Each item is a small ObjectSummary
 * pointing at a related symbol.
 */
const relationItemSchema = z.object({
  object_id: z.string(),
  name: z.string(),
  kind: z.string(),
  file: z.string(),
  line: z.number().int().nonnegative(),
});
export type RelationItem = z.infer<typeof relationItemSchema>;

const callListBodySchema = z.object({
  count: z.number().int().nonnegative(),
  items: z.array(relationItemSchema),
});
export type CallListBlockBody = z.infer<typeof callListBodySchema>;

// --- Source ---

const sourceLineSchema = z.object({
  line: z.number().int().positive(),
  text: z.string(),
});
export type SourceLine = z.infer<typeof sourceLineSchema>;

const sourceSliceBodySchema = z.object({
  file: z.string(),
  line: z.number().int().nonnegative(),
  lines: z.array(sourceLineSchema),
});
export type SourceSliceBlockBody = z.infer<typeof sourceSliceBodySchema>;

// --- Quality (symbol / file / scope) ---

const symbolQualityIdentityBodySchema = z.object({
  file: z.string(),
  line: z.number().int().nonnegative(),
  issue_count: z.number().int().nonnegative(),
});
export type SymbolQualityIdentityBlockBody = z.infer<
  typeof symbolQualityIdentityBodySchema
>;

const fileQualityIdentityBodySchema = z.object({
  path: z.string(),
  issue_count: z.number().int().nonnegative(),
});
export type FileQualityIdentityBlockBody = z.infer<
  typeof fileQualityIdentityBodySchema
>;

const scopeQualityIdentityBodySchema = z.object({
  scope: z.string(),
  issue_count: z.number().int().nonnegative(),
  by_severity: z.record(z.string(), z.number().int().nonnegative()),
});
export type ScopeQualityIdentityBlockBody = z.infer<
  typeof scopeQualityIdentityBodySchema
>;

const qualityGateBlockBodySchema = qualityGateSummarySchema;
export type QualityGateBlockBody = z.infer<typeof qualityGateBlockBodySchema>;

const issuesListBodySchema = z.object({
  count: z.number().int().nonnegative(),
  items: z.array(qualityIssueItemSchema),
});
export type IssuesListBlockBody = z.infer<typeof issuesListBodySchema>;

// --- Quality detail (issue / rule) ---

const issueIdentityBodySchema = z.object({
  id: z.number().int().positive(),
  rule_id: z.string(),
  severity: z.string(),
  category: z.string(),
  status: z.string(),
});
export type IssueIdentityBlockBody = z.infer<typeof issueIdentityBodySchema>;

const issueLocationBodySchema = z.object({
  file: z.string(),
  line: z.number().int().nonnegative(),
});
export type IssueLocationBlockBody = z.infer<typeof issueLocationBodySchema>;

const issueMessageBodySchema = z.object({
  message: z.string(),
});
export type IssueMessageBlockBody = z.infer<typeof issueMessageBodySchema>;

const ruleIdentityBodySchema = z.object({
  rule_id: z.string(),
  description: z.string(),
  open_count: z.number().int().nonnegative(),
});
export type RuleIdentityBlockBody = z.infer<typeof ruleIdentityBodySchema>;

// --- File / Scope ---

const fileIdentityBodySchema = z.object({
  path: z.string(),
  line_count: z.number().int().nonnegative(),
  symbol_count: z.number().int().nonnegative(),
});
export type FileIdentityBlockBody = z.infer<typeof fileIdentityBodySchema>;

const kindsBreakdownBodySchema = z.object({
  breakdown: z.record(z.string(), z.number().int().nonnegative()),
});
export type KindsBreakdownBlockBody = z.infer<typeof kindsBreakdownBodySchema>;

const fileSymbolsItemSchema = z.object({
  name: z.string(),
  kind: z.string(),
  line: z.number().int().nonnegative(),
  object_id: z.string(),
});
export type FileSymbolItem = z.infer<typeof fileSymbolsItemSchema>;

const fileSymbolsBodySchema = z.object({
  count: z.number().int().nonnegative(),
  items: z.array(fileSymbolsItemSchema),
});
export type FileSymbolsBlockBody = z.infer<typeof fileSymbolsBodySchema>;

const scopeIdentityBodySchema = z.object({
  path: z.string(),
  file_count: z.number().int().nonnegative(),
  symbol_count: z.number().int().nonnegative(),
  promotion_ready: z.boolean(),
});
export type ScopeIdentityBlockBody = z.infer<typeof scopeIdentityBodySchema>;

const scopeFilesBodySchema = z.object({
  files: z.array(z.string()),
});
export type ScopeFilesBlockBody = z.infer<typeof scopeFilesBodySchema>;

// --- Scope dependencies / hotspots ---

const crossScopeEntrySchema = z.object({
  scope: z.string(),
  outgoing_count: z.number().int().nonnegative(),
  incoming_count: z.number().int().nonnegative(),
});
export type CrossScopeEntry = z.infer<typeof crossScopeEntrySchema>;

const crossScopeBodySchema = z.object({
  scope: z.string(),
  file_count: z.number().int().nonnegative(),
  symbol_count: z.number().int().nonnegative(),
  entries: z.array(crossScopeEntrySchema),
});
export type CrossScopeBlockBody = z.infer<typeof crossScopeBodySchema>;

const hotspotItemSchema = z.object({
  name: z.string(),
  kind: z.string(),
  file: z.string(),
  line: z.number().int().nonnegative(),
  object_id: z.string(),
});
export type HotspotItem = z.infer<typeof hotspotItemSchema>;

const hotspotsBodySchema = z.object({
  scope: z.string(),
  count: z.number().int().nonnegative(),
  items: z.array(hotspotItemSchema),
});
export type HotspotsBlockBody = z.infer<typeof hotspotsBodySchema>;

// --- Quality dashboard blocks (Phase 10) ---

/**
 * Severity counts for the dashboard. Each bucket is non-negative.
 * Missing buckets are treated as 0 downstream.
 */
const severityCountsSchema = z.object({
  blocker: z.number().int().nonnegative(),
  critical: z.number().int().nonnegative(),
  major: z.number().int().nonnegative(),
  minor: z.number().int().nonnegative(),
  info: z.number().int().nonnegative(),
});
export type SeverityCounts = z.infer<typeof severityCountsSchema>;

/**
 * `quality_summary` — emitted by the Quality Dashboard. Aggregates
 * rating + debt + per-severity counts for the active scope/file.
 * Optional `path` is included for scope/file objects.
 */
const qualitySummaryBodySchema = z.object({
  scope: z.string(),
  rating: z.string().nullable(),
  total_issues: z.number().int().nonnegative(),
  debt_minutes: z.number().int().nonnegative(),
  by_severity: severityCountsSchema,
  last_run: z.string().nullable().optional(),
});
export type QualitySummaryBlockBody = z.infer<typeof qualitySummaryBodySchema>;

/**
 * `quality_issue_detail` — single-issue view with rule reference,
 * location, message, optional remediation tip. Drives the
 * `QualityIssueDetailBlock` renderer.
 */
const qualityIssueDetailBodySchema = z.object({
  id: z.number().int().positive(),
  rule_id: z.string(),
  severity: qualitySeveritySchema,
  category: z.string(),
  status: z.string(),
  file: z.string(),
  line: z.number().int().nonnegative(),
  message: z.string(),
  remediation: z.string().nullable().optional(),
  rule_description: z.string().nullable().optional(),
  rule_url: z.string().nullable().optional(),
  object_id: z.string(),
});
export type QualityIssueDetailBlockBody = z.infer<
  typeof qualityIssueDetailBodySchema
>;

// ============================================================================
// The 27-shape discriminated union + fallback
// ============================================================================

/**
 * `ViewBlock.body` is a `serde_json::Value` on the backend, so we
 * keep `body: z.unknown()` in the shared shell. The narrower per-block
 * schemas above (IdentityBlockBody, etc.) are the types consumers use
 * inside the renderer's switch on `block.id`.
 *
 * The fallback (`unknown` `id`) accepts any future shape — body is
 * `unknown` so we can still render the raw JSON.
 */
export const viewBlockSchema = z.discriminatedUnion("id", [
  // Symbol overview (3)
  blockShell("identity", identityBodySchema),
  blockShell("call_metrics", callMetricsBodySchema),
  blockShell("signature", signatureBodySchema),
  // Call graph (2)
  blockShell("callers", callListBodySchema),
  blockShell("callees", callListBodySchema),
  // Source (1)
  blockShell("source_slice", sourceSliceBodySchema),
  // Symbol quality (2)
  blockShell("symbol_quality_identity", symbolQualityIdentityBodySchema),
  blockShell("symbol_quality_issues", issuesListBodySchema),
  // File quality (3)
  blockShell("file_quality_identity", fileQualityIdentityBodySchema),
  blockShell("file_quality_issues", issuesListBodySchema),
  blockShell("file_quality_gate", qualityGateBlockBodySchema),
  // Scope quality (3)
  blockShell("scope_quality_identity", scopeQualityIdentityBodySchema),
  blockShell("scope_quality_gate", qualityGateBlockBodySchema),
  blockShell("scope_quality_issues", issuesListBodySchema),
  // Issue detail (3)
  blockShell("issue_identity", issueIdentityBodySchema),
  blockShell("issue_location", issueLocationBodySchema),
  blockShell("issue_message", issueMessageBodySchema),
  // Rule detail (2)
  blockShell("rule_identity", ruleIdentityBodySchema),
  blockShell("rule_related", issuesListBodySchema),
  // File overview + symbols (3)
  blockShell("file_identity", fileIdentityBodySchema),
  blockShell("kinds", kindsBreakdownBodySchema),
  blockShell("symbols", fileSymbolsBodySchema),
  // Scope overview (3)
  blockShell("scope_identity", scopeIdentityBodySchema),
  blockShell("scope_kinds", kindsBreakdownBodySchema),
  blockShell("scope_files", scopeFilesBodySchema),
  // Scope dependencies + hotspots (2)
  blockShell("cross_scope", crossScopeBodySchema),
  blockShell("hotspots", hotspotsBodySchema),
  // Quality dashboard (2)
  blockShell("quality_summary", qualitySummaryBodySchema),
  blockShell("quality_issue_detail", qualityIssueDetailBodySchema),
]);
export type ViewBlock = z.infer<typeof viewBlockSchema>;

/**
 * Permissive fallback for any block id we do not recognise yet.
 * The renderer can use this to display raw JSON until a typed
 * renderer is shipped.
 */
export const unknownViewBlockSchema = z.object({
  id: z.string(),
  title: z.string(),
  body: z.unknown(),
});
export type UnknownViewBlock = z.infer<typeof unknownViewBlockSchema>;

/**
 * The boundary schema — try the typed union first, fall back to the
 * permissive shape. The result keeps the original id/title/body so
 * callers can decide how to handle unknown ids downstream.
 */
export const viewBlockAnySchema = z
  .union([viewBlockSchema, unknownViewBlockSchema])
  .transform((block) => block);
export type ViewBlockAny = z.output<typeof viewBlockAnySchema>;

// ============================================================================
// Phase 0: Moldable View Runtime — Domain Vocabulary (ADR-008)
// ============================================================================
//
// Forward-compatible enums: unknown string values are accepted by the
// schema (they pass parse) so that wire-format evolution doesn't break
// clients. The Rust side resolves unknown values to the catch-all
// `Custom` unit variant at deserialization time.

// --- ViewKind — semantic intent of a view ---

/**
 * First-class ViewKind catalog from ADR-008 §First-class ViewKind.
 * Unknown strings are accepted (forward compatibility) — the Rust enum
 * maps them to the `Custom` unit variant on deserialization.
 */
export const viewKindSchema = z.union([
  z.literal("vertical_slice"),
  z.literal("call_graph"),
  z.literal("seam_map"),
  z.literal("dependency_graph"),
  z.literal("source_view"),
  z.literal("data_flow"),
  z.literal("impact_radius"),
  z.literal("diff_view"),
  z.literal("c4_context"),
  z.literal("c4_container"),
  z.literal("c4_component"),
  z.literal("c4_code"),
  z.literal("quality_hotspots"),
  z.literal("evidence_view"),
  z.literal("decision_graph"),
  z.literal("architecture_rationale"),
  z.literal("architecture_drift"),
  z.literal("boundary_map"),
  z.literal("dependency_pressure"),
  z.literal("change_impact_story"),
  z.literal("ownership_map"),
  z.literal("risk_map"),
  z.literal("decision_trace"),
  z.literal("test_slice"),
  z.literal("debug_slice"),
  z.literal("refactor_plan"),
  z.literal("callers_and_implementors"),
  z.literal("usage_examples"),
  z.literal("api_surface"),
  z.literal("dead_code_candidates"),
  z.literal("semantic_search_results"),
  z.literal("doc_code_alignment"),
  z.literal("example_object"),
  z.literal("composed_narrative"),
  z.literal("project_diary"),
  z.literal("concept_map"),
  z.literal("evidence_pack"),
  // Forward-compatibility: any unknown string accepted here; the Rust
  // deserializer maps it to `ViewKind::Custom`.
  z.string(),
]);
export type ViewKind = z.infer<typeof viewKindSchema>;

// --- RendererKind — visual rendering strategy ---

/**
 * First-class RendererKind catalog. Unknown strings are accepted
 * (forward compatibility) — the Rust enum maps them to the `Custom`
 * unit variant on deserialization.
 */
export const rendererKindSchema = z.union([
  z.literal("graph"),
  z.literal("table"),
  z.literal("tree"),
  z.literal("code"),
  z.literal("markdown"),
  z.literal("vega_lite"),
  z.literal("json"),
  z.literal("composite"),
  // Forward-compatibility catch-all
  z.string(),
]);
export type RendererKind = z.infer<typeof rendererKindSchema>;

// --- HierarchyKind — navigable structural projection ---

/**
 * First-class HierarchyKind catalog. Unknown strings are accepted
 * (forward compatibility) — the Rust enum maps them to the `Custom`
 * unit variant on deserialization.
 */
export const hierarchyKindSchema = z.union([
  z.literal("file_tree"),
  z.literal("module_tree"),
  z.literal("type_hierarchy"),
  z.literal("call_hierarchy"),
  z.literal("package_graph"),
  z.literal("c4_hierarchy"),
  // Forward-compatibility catch-all
  z.string(),
]);
export type HierarchyKind = z.infer<typeof hierarchyKindSchema>;

// --- DataSource — where view data comes from ---

/**
 * DataSource union. Moldql is the v1 runtime source; unknown `kind`
 * values fall through to the permissive Other shape (forward compatibility).
 *
 * Note: uses a plain `z.union` (not discriminated) because the
 * forward-compatibility arm has `kind: z.string()` which is not valid
 * in a discriminated union.
 */
export const dataSourceSchema = z.union([
  z.object({
    kind: z.literal("moldql"),
    query: z.string(),
  }),
  // Forward-compatibility: any unknown `kind` is accepted.
  // The entire object is preserved so the payload survives round-trip.
  z.object({
    kind: z.string(),
  }),
]);
export type DataSource = z.infer<typeof dataSourceSchema>;

// --- Transform — how view data is shaped before rendering ---

/**
 * Transform union. Jsonata is the v1 transform language; unknown
 * `kind` values fall through to the permissive Other shape
 * (forward compatibility).
 */
export const transformSchema = z.union([
  z.object({
    kind: z.literal("jsonata"),
    expression: z.string(),
  }),
  // Forward-compatibility catch-all
  z.object({
    kind: z.string(),
  }),
]);
export type Transform = z.infer<typeof transformSchema>;

// --- ViewSpec — the core Moldable View Runtime DTO ---

/**
 * A persisted view specification — the core DTO of the Moldable View Runtime.
 *
 * `id`: server-assigned UUID on persist; client-suggested on create.
 * `title`: non-empty, ≤ 200 chars (validated at the service layer).
 * `applies_to`: narrows which InspectableObjectType the view works on.
 * `view_kind`: semantic intent; `renderer_kind`: visual strategy.
 * `data_source` + `transform`: data supply and reshape (v1 = Moldql + Jsonata).
 * `props`: renderer-specific config; defaults to `{}`.
 * `created_at` / `updated_at`: ISO-8601 UTC; server-assigned.
 */
export const viewSpecSchema = z.object({
  id: z.string().uuid(),
  title: z.string().min(1).max(200),
  applies_to: inspectableObjectTypeSchema,
  view_kind: viewKindSchema,
  data_source: dataSourceSchema,
  transform: transformSchema.nullable().optional(),
  renderer_kind: rendererKindSchema,
  props: z.unknown().default({}),
  created_at: z.string(),
  updated_at: z.string(),
  /** The user who owns this spec. Used for ownership checks and Spotter display. */
  owner: z.string().default(""),
});
export type ViewSpec = z.infer<typeof viewSpecSchema>;

// --- ViewSpecSummary — minimal ViewSpec for Spotter search hits ---

/**
 * ViewSpec summary for Spotter search hits.
 * Minimal info needed to display a hit and open the spec.
 */
export const viewSpecSummarySchema = z.object({
  id: z.string().uuid(),
  title: z.string(),
  view_kind: viewKindSchema,
  applies_to: inspectableObjectTypeSchema,
  owner: z.string(),
  updated_at: z.string(),
});
export type ViewSpecSummary = z.infer<typeof viewSpecSummarySchema>;

/**
 * Discriminated union of all Spotter hit families returned by the backend
 * (`SpotterSearchResult` in `dto.rs`). Each variant carries a `result`
 * payload whose shape depends on the family:
 *
 * - `symbol`         → `SpotterResult` (symbol graph hit)
 * - `file`           → `SpotterResult` (grouped symbol-by-file hit)
 * - `viewspec`       → `ViewSpecSummary` (runtime ViewSpec hit)
 * - `saved_exploration` → `SpotterResult` (saved session hit)
 * - `quality_issue`  → `SpotterResult` (issue-rule hit)
 * - `rule`           → `SpotterResult` (quality rule hit)
 * - `route`          → `SpotterResult` (HTTP route from ingested OpenAPI/gRPC spec)
 *
 * Frontend components switch on `kind` to render each variant. The
 * `useSpotter` hook unwraps `result` before returning so callers always
 * receive the flat `SpotterResult` shape (matching the original MVP
 * contract before multi-family was added in e13-wave-1).
 */
export const spotterSearchResultSchema = z.discriminatedUnion("kind", [
  z.object({
    kind: z.literal("symbol"),
    result: spotterResultSchema,
  }),
  z.object({
    kind: z.literal("file"),
    result: spotterResultSchema,
  }),
  z.object({
    kind: z.literal("viewspec"),
    result: viewSpecSummarySchema,
  }),
  z.object({
    kind: z.literal("saved_exploration"),
    result: spotterResultSchema,
  }),
  z.object({
    kind: z.literal("quality_issue"),
    result: spotterResultSchema,
  }),
  z.object({
    kind: z.literal("rule"),
    result: spotterResultSchema,
  }),
  z.object({
    kind: z.literal("route"),
    result: spotterResultSchema,
  }),
]);
export type SpotterSearchResult = z.infer<typeof spotterSearchResultSchema>;

// ============================================================================
// ContextualView, exploration, artifact
// ============================================================================

export const contextualViewSchema = z.object({
  object_id: z.string(),
  view_id: z.string(),
  title: z.string(),
  /**
   * Semantic intent of this view (ADR-008 §ViewKind). Required for routing
   * in PaneInspector — determines whether to render via GraphViewRenderer
   * or Blocks. Backend stamps this from ViewDescriptor after build().
   * Undefined for legacy payloads that predate this field.
   */
  view_kind: viewKindSchema.optional(),
  blocks: z.array(viewBlockAnySchema),
  relations: z.array(typedRelationSchema),
  evidence: z.array(evidenceBlockSchema),
  findings: z.array(designFindingSchema).default([]),
  /**
   * Visual rendering strategy for this view.
   * Mirrors `RendererKind` from the Moldable View Runtime (ADR-008).
   * Defaults to `"json"` when absent — backward compatible with payloads
   * that predate this field (serde default on the Rust side).
   */
  renderer_kind: rendererKindSchema.default("json"),
});
export type ContextualView = z.infer<typeof contextualViewSchema>;

// ViewportState for graph pan/zoom capture (ADR-040 Wave 3)
const viewportStateSchema = z.object({
  x: z.number(),
  y: z.number(),
  scale: z.number(),
});
export type ViewportStateDto = z.infer<typeof viewportStateSchema>;

// PaneSnapshot for session restore (ADR-040 Wave 3)
const paneSnapshotSchema = z.object({
  pane_id: z.string(),
  object_id: z.string(),
  view_id: z.string(),
  scroll_y: z.number(),
  viewport: viewportStateSchema.nullable(),
});
export type PaneSnapshotDto = z.infer<typeof paneSnapshotSchema>;

// ExplorationEvent for session
const explorationEventSchema = z.object({
  object_id: z.string(),
  view_id: z.string().nullable(),
  query: z.string().nullable(),
  ts: z.string(),
});
export type ExplorationEventDto = z.infer<typeof explorationEventSchema>;

// ExplorationSession for session restore (ADR-040 Wave 3)
export const explorationSessionSchema = z.object({
  id: z.string(),
  workspace_id: z.string(),
  events: z.array(explorationEventSchema),
  navigation_mode: z.string(),
  panes: z.array(paneSnapshotSchema),
  created_at: z.string(),
});
export type ExplorationSessionDto = z.infer<typeof explorationSessionSchema>;

// SaveExplorationSessionRequest (ADR-040 Wave 3)
// eslint-disable-next-line @typescript-eslint/no-unused-vars -- schema only used for type inference
const saveExplorationSessionRequestSchema = z.object({
  workspace_id: z.string(),
  events: z.array(explorationEventSchema),
  navigation_mode: z.string(),
  panes: z.array(paneSnapshotSchema),
});
export type SaveExplorationSessionRequestDto = z.infer<typeof saveExplorationSessionRequestSchema>;

export const openWorkspaceRequestSchema = z.object({
  root_path: z.string(),
});
export type OpenWorkspaceRequest = z.infer<typeof openWorkspaceRequestSchema>;

export const indexWorkspaceRequestSchema = z.object({
  strategy: z.string().nullable().optional(),
});
export type IndexWorkspaceRequest = z.infer<typeof indexWorkspaceRequestSchema>;

export const generateArtifactRequestSchema = z.object({
  format: artifactFormatSchema,
});
export type GenerateArtifactRequest = z.infer<
  typeof generateArtifactRequestSchema
>;

export const decisionArtifactSummarySchema = z.object({
  id: z.string(),
  format: artifactFormatSchema,
  title: z.string(),
  content: z.string(),
});
export type DecisionArtifactSummary = z.infer<
  typeof decisionArtifactSummarySchema
>;

// ============================================================================
// Health
// ============================================================================

export const healthResponseSchema = z.object({
  status: z.string(),
  service: z.string(),
});
export type HealthResponse = z.infer<typeof healthResponseSchema>;

// ============================================================================
// `cognicode_ask` response (contextual-help)
// ============================================================================

/**
 * Response shape for `POST /api/ask` — the natural-language router
 * used by the contextual-help suggestion strip. The backend currently
 * returns the typed `McpResultEnvelope` from `cognicode_ask`; we keep
 * this schema permissive (it is not the primary type) but a single
 * known shape gives us runtime validation at the boundary.
 */
export const askResponseSchema = z.object({
  status: z.string(),
  primary_result: z.unknown().nullable().optional(),
  supporting: z.array(z.unknown()).optional(),
  suggested_follow_ups: z
    .array(
      z.object({
        id: z.string(),
        label: z.string(),
        tool: z.string(),
        params: z.record(z.string(), z.string()),
      }),
    )
    .optional(),
});
export type AskResponse = z.infer<typeof askResponseSchema>;

// ============================================================================
// Subgraph (visualization-stack Phase 1)
// ============================================================================

/**
 * Node `style_class` taxonomy — strict mirror of the Rust
 * `style_class_for` helper. Unknown buckets fail parse so the
 * front-end can never silently mis-style a node.
 *
 * Multimodal (T17) — 4 new buckets appended for the Generic Graph
 * Layer nodes (decision / doc / issue / evidence). Dashed form
 * (`node-decision`, etc.) so the cytoscape stylesheet can match a
 * single attribute selector and the kind label never collides
 * with the code-only taxonomy (`function` / `module` / `external`).
 */
export const graphNodeStyleClassSchema = z.enum([
  "function",
  "module",
  "external",
  "node-decision",
  "node-doc",
  "node-issue",
  "node-evidence",
  "node-component",
  "node-container",
  "node-system",
  "node-code",
  // ---- Landing Page (E4 ADR-039) ----
  "entry-point",
  "hot",
  "god",
]);
export type GraphNodeStyleClass = z.infer<typeof graphNodeStyleClassSchema>;

/**
 * Edge `style_class` taxonomy — strict mirror of the Rust
 * `edge_style_class_for` helper. Same rationale as the node variant.
 *
 * Multimodal (T17) — 4 new buckets for the Generic Graph Layer
 * edges (cites / justifies / resolves / corroborated).
 */
export const graphEdgeStyleClassSchema = z.enum([
  "edge.calls",
  "edge.implements",
  "edge.uses",
  "edge-cites",
  "edge-justifies",
  "edge-resolves",
  "edge-corroborated",
  "edge-part-of",
  "edge-deployed-as",
  "edge-in-system",
]);
export type GraphEdgeStyleClass = z.infer<typeof graphEdgeStyleClassSchema>;

// ============================================================================
// T17 — multimodal node / edge kind enums (Generic Graph Layer).
// ============================================================================
//
// These are the discriminator values from the Rust domain model:
//   `cognicode_core::domain::value_objects::NodeKind`
//   `cognicode_core::domain::value_objects::EdgeKind`
//
// `SymbolKind` is the 22-variant code-only enum that `NodeKind::Symbol`
// wraps, but the `NodeKind` enum on the wire is either a multimodal
// tag ("decision" / "doc" / "issue" / "evidence") or a bare "symbol"
// for any code node. The `SymbolKind` payload is not surfaced on the
// wire at this layer; if a future feature needs it, add a new wire
// field rather than overloading `NodeKind`.

/**
 * `NodeKind` wire enum — multimodal (Generic Graph Layer).
 * The Rust `NodeKind::Symbol(SymbolKind)` variant collapses to the
 * `"symbol"` string at the wire boundary; the inner `SymbolKind`
 * is not exposed here.
 */
export const nodeKindSchema = z.enum([
  "symbol",
  "decision",
  "doc",
  "issue",
  "evidence",
  "route", // e15.5: OpenAPI/gRPC/GraphQL/trRPC route nodes
]);
export type NodeKind = z.infer<typeof nodeKindSchema>;

/**
 * `EdgeKind` wire enum — multimodal (Generic Graph Layer). The Rust
 * `EdgeKind::Dependency(DependencyType)` variant collapses to
 * `"dependency"`; the inner `DependencyType` is not exposed here.
 */
export const edgeKindSchema = z.enum([
  "dependency",
  "cites",
  "justifies",
  "resolves",
  "corroborated_by",
  // e15.5: protocol cross-service edges
  "http_calls",
  "graphql_calls",
  "grpc_calls",
  "trpc_calls",
]);
export type EdgeKind = z.infer<typeof edgeKindSchema>;

/**
 * Multimodal node DTO — mirrors the `GraphNode` aggregate from the
 * Rust side. The `id` is a `NodeId` (e.g. `decision:adrs/0007.md#adr-7`),
 * `label` is the human display string, `kind` is the wire `NodeKind`
 * (see [`nodeKindSchema`]), `source_path` is the file the node was
 * extracted from (Markdown / ADR / issue tracker), and `metadata` is
 * a free-form JSON blob (status, dates, body, etc.).
 */
export const multimodalNodeSchema = z.object({
  id: z.string(),
  label: z.string(),
  kind: nodeKindSchema,
  source_path: z.string().nullable().optional(),
  metadata: z.record(z.string(), z.unknown()).default({}),
});
export type MultimodalNode = z.infer<typeof multimodalNodeSchema>;

/**
 * Multimodal edge DTO — mirrors the `GraphEdge` aggregate. `source`
 * and `target` are `NodeId` strings; `kind` is the wire `EdgeKind`
 * (see [`edgeKindSchema`]); `provenance` and `confidence` mirror
 * the existing `TypedRelation` shape on the API.
 */
export const multimodalEdgeSchema = z.object({
  source: z.string(),
  target: z.string(),
  kind: edgeKindSchema,
  provenance: z.string().optional(),
  confidence: z.number().min(0).max(1).optional(),
});
export type MultimodalEdge = z.infer<typeof multimodalEdgeSchema>;

/**
 * A single `graph_search` hit — what the MCP `graph_search` tool
 * (T22) returns. The normalised `score` is the
 * `0.6 * FTS5-rank + 0.4 * kind-bonus` formula from the design
 * (T22 IB check), and `raw_rank` is the raw FTS5 rank so callers
 * can re-derive their own score if needed (lossy compression
 * flagged in `design.md` §Information Bottleneck Check).
 */
export const graphSearchResultSchema = z.object({
  node: multimodalNodeSchema,
  score: z.number(),
  raw_rank: z.number(),
});
export type GraphSearchResult = z.infer<typeof graphSearchResultSchema>;

/**
 * Top-level envelope for `graph_search`. `total_count` is the
 * total number of matches in the index (NOT the size of the
 * current page); `next_cursor` is `null` on the last page.
 */
export const graphSearchResponseSchema = z.object({
  results: z.array(graphSearchResultSchema),
  total_count: z.number().int().nonnegative(),
  next_cursor: z.string().nullable(),
});
export type GraphSearchResponse = z.infer<typeof graphSearchResponseSchema>;

/**
 * One node in a sub-graph response. `id` matches the canonical MVP
 * id; `style_class` is the cytoscape-taxonomy bucket the backend
 * already derived — the front-end does NOT re-classify.
 */
export const graphNodeSchema = z.object({
  id: z.string(),
  label: z.string(),
  kind: z.string(),
  file: z.string().optional(),
  line: z.number().int().nonnegative().optional(),
  style_class: graphNodeStyleClassSchema,
});
export type GraphNode = z.infer<typeof graphNodeSchema>;

/**
 * One edge. `source` and `target` reference `graphNode.id` by
 * equality — the adapter assumes the response is internally
 * consistent (the backend guarantees this even after truncation).
 */
export const graphEdgeSchema = z.object({
  source: z.string(),
  target: z.string(),
  relation: z.string(),
  style_class: graphEdgeStyleClassSchema,
});
export type GraphEdge = z.infer<typeof graphEdgeSchema>;

/**
 * Top-level response for `GET /api/graph/:id/subgraph`.
 *
 * `truncated_reason` is `Some("node_cap")` whenever `truncated` is
 * `true`. We type it as `optional + nullable` so the absence case
 * (most responses) round-trips cleanly.
 *
 * `corroboration_scores` is an optional map of `"source->target"` → score.
 * Populated by the rationale endpoint; empty otherwise.
 */
export const subgraphResponseSchema = z.object({
  root: z.string(),
  nodes: z.array(graphNodeSchema),
  edges: z.array(graphEdgeSchema),
  truncated: z.boolean(),
  truncated_reason: z.string().nullable().optional(),
  corroboration_scores: z.record(z.string(), z.number().min(0).max(1)).default({}),
});
export type SubgraphResponse = z.infer<typeof subgraphResponseSchema>;

/**
 * Payload for a loaded rationale view, returned by the named-view
 * system when `view_load` is called on a rationale lens.
 */
export const rationaleViewPayloadSchema = z.object({
  subgraph: subgraphResponseSchema,
  corroboration_scores: z.record(z.string(), z.number().min(0).max(1)),
  source_count: z.number().int().nonnegative(),
});
export type RationaleViewPayload = z.infer<typeof rationaleViewPayloadSchema>;

// ============================================================================
// Contextual Graph — Contextual Views (Phase 1 of visualization-stack)
// ============================================================================
//
// Mirrors the Rust `ContextualGraphResponse` family in
// `crates/cognicode-explorer/src/dto.rs`. The four sections
// (`focusNode`, `parent`, `children`, `sameLevel`) reuse the
// `GraphNode` / `GraphEdge` shapes (ADR-CX-2).

/**
 * The `parent` section — the file the focus lives in, plus the
 * `lives_in` edge connecting them. Null when the focus is an orphan.
 */
export const parentSectionSchema = z.object({
  node: graphNodeSchema,
  edge: graphEdgeSchema,
});
export type ParentSection = z.infer<typeof parentSectionSchema>;

/**
 * The `children` section — the siblings of the focus in its file,
 * with the `lives_in` edges pointing at the focus. Null when the
 * focus is an orphan.
 */
export const childrenSectionSchema = z.object({
  nodes: z.array(graphNodeSchema),
  edges: z.array(graphEdgeSchema),
});
export type ChildrenSection = z.infer<typeof childrenSectionSchema>;

/**
 * The `sameLevel` section — the BFS of callers + callees around the
 * focus, bounded by `max_nodes` (combined with children).
 */
export const sameLevelSectionSchema = z.object({
  nodes: z.array(graphNodeSchema),
  edges: z.array(graphEdgeSchema),
});
export type SameLevelSection = z.infer<typeof sameLevelSectionSchema>;

/**
 * Top-level response for `GET /api/graph/:id/contextual`.
 *
 * `truncatedReason` is `null` (or absent) when nothing was clipped;
 * `"max_nodes_exceeded"` when the children / same-level combined
 * set was trimmed to fit the cap.
 */
export const contextualGraphResponseSchema = z.object({
  focusNode: graphNodeSchema,
  parent: parentSectionSchema.nullable(),
  children: childrenSectionSchema.nullable(),
  sameLevel: sameLevelSectionSchema,
  level: z.string(),
  truncated: z.boolean(),
  // e11: renamed from `truncationReason` → `truncatedReason` (aligned with LandingPayload).
  // Backend serializes `truncatedReason`; accepts `truncationReason` alias on input for
  // wire-compatible migration (until next MAJOR).
  truncatedReason: z.string().nullable().optional(),
});
export type ContextualGraphResponse = z.infer<
  typeof contextualGraphResponseSchema
>;

// ============================================================================
// Landing Page — E4 ADR-039
// ============================================================================

/**
 * A god node entry — a high PageRank symbol with its score.
 * Mirrors `cognicode_explorer::dto::GodNodeEntry`.
 */
export const godNodeEntrySchema = z.object({
  id: z.string(),
  label: z.string(),
  score: z.number(),
});
export type GodNodeEntry = z.infer<typeof godNodeEntrySchema>;

/**
 * Response payload for `GET /api/workspaces/:id/landing`.
 * Bundles everything the landing page needs in a single round-trip.
 * Mirrors `cognicode_explorer::dto::LandingPayload`.
 */
export const landingPayloadSchema = z.object({
  workspace: workspaceSummarySchema,
  nodes: z.array(graphNodeSchema),
  edges: z.array(graphEdgeSchema),
  entry_points: z.array(inspectableObjectSummarySchema),
  hot_paths: z.array(inspectableObjectSummarySchema),
  god_nodes: z.array(godNodeEntrySchema),
  suggested_questions: z.array(z.string()),
  graph_status: graphStatusSchema,
  truncated: z.boolean().optional(),
  truncated_reason: z.string().nullable().optional(),
});
export type LandingPayload = z.infer<typeof landingPayloadSchema>;

// ============================================================================
// Architecture View — E5 ADR-039 (Perspective Toggle Graph ↔ C4)
// ============================================================================

/**
 * Response payload for `GET /api/workspaces/:id/architecture`.
 * Reuses `SubgraphResponse` — nodes use `kind = "component"` and
 * `style_class = "node-component"`, edges use `relation = "part_of"`.
 */
export const architecturePayloadSchema = subgraphResponseSchema;
export type ArchitecturePayload = z.infer<typeof architecturePayloadSchema>;

// ============================================================================
// Convenience aliases
// ============================================================================

/** Per-block body types keyed by block id. Useful in switch statements. */
export type ViewBlockBodyById = {
  identity: IdentityBlockBody;
  call_metrics: CallMetricsBlockBody;
  signature: SignatureBlockBody;
  callers: CallListBlockBody;
  callees: CallListBlockBody;
  source_slice: SourceSliceBlockBody;
  symbol_quality_identity: SymbolQualityIdentityBlockBody;
  symbol_quality_issues: IssuesListBlockBody;
  file_quality_identity: FileQualityIdentityBlockBody;
  file_quality_issues: IssuesListBlockBody;
  file_quality_gate: QualityGateBlockBody;
  scope_quality_identity: ScopeQualityIdentityBlockBody;
  scope_quality_gate: QualityGateBlockBody;
  scope_quality_issues: IssuesListBlockBody;
  issue_identity: IssueIdentityBlockBody;
  issue_location: IssueLocationBlockBody;
  issue_message: IssueMessageBlockBody;
  rule_identity: RuleIdentityBlockBody;
  rule_related: IssuesListBlockBody;
  file_identity: FileIdentityBlockBody;
  kinds: KindsBreakdownBlockBody;
  symbols: FileSymbolsBlockBody;
  scope_identity: ScopeIdentityBlockBody;
  scope_kinds: KindsBreakdownBlockBody;
  scope_files: ScopeFilesBlockBody;
  cross_scope: CrossScopeBlockBody;
  hotspots: HotspotsBlockBody;
  quality_summary: QualitySummaryBlockBody;
  quality_issue_detail: QualityIssueDetailBlockBody;
};
