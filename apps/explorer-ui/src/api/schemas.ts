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
});
export type WorkspaceSummary = z.infer<typeof workspaceSummarySchema>;

export const viewDescriptorSchema = z.object({
  id: z.string(),
  title: z.string(),
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
// ContextualView, exploration, artifact
// ============================================================================

export const contextualViewSchema = z.object({
  object_id: z.string(),
  view_id: z.string(),
  title: z.string(),
  blocks: z.array(viewBlockAnySchema),
  relations: z.array(typedRelationSchema),
  evidence: z.array(evidenceBlockSchema),
  findings: z.array(designFindingSchema).default([]),
});
export type ContextualView = z.infer<typeof contextualViewSchema>;

export const explorationColumnSchema = z.object({
  object_id: z.string(),
  active_view: z.string().nullable(),
  kind: z.string().default("symbol"),
});
export type ExplorationColumn = z.infer<typeof explorationColumnSchema>;

export const explorationPathSchema = z.object({
  id: z.string(),
  workspace_id: z.string(),
  columns: z.array(explorationColumnSchema),
  objects: z.array(objectIdentityEntrySchema).default([]),
  lens: z.string().nullable(),
  created_at: z.string(),
});
export type ExplorationPath = z.infer<typeof explorationPathSchema>;

export const openWorkspaceRequestSchema = z.object({
  root_path: z.string(),
});
export type OpenWorkspaceRequest = z.infer<typeof openWorkspaceRequestSchema>;

export const indexWorkspaceRequestSchema = z.object({
  strategy: z.string().nullable().optional(),
});
export type IndexWorkspaceRequest = z.infer<typeof indexWorkspaceRequestSchema>;

export const saveExplorationRequestSchema = z.object({
  workspace_id: z.string(),
  columns: z.array(explorationColumnSchema),
  lens: z.string().nullable(),
});
export type SaveExplorationRequest = z.infer<
  typeof saveExplorationRequestSchema
>;

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
