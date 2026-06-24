/**
 * `z.infer<typeof X>` re-exports for every schema in `schemas.ts`.
 *
 * The rule is: import `type X` from here, never re-derive from
 * `z.infer` in feature code. Keeps the public type surface obvious
 * and lets the schema module evolve without rippling.
 */
export type {
  GraphStatus,
  InspectableObjectType,
  RelationDirection,
  FindingSeverity,
  ArtifactFormat,
  LineRange,
  Property,
  ObjectIdentityEntry,
  EvidenceBlock,
  TypedRelation,
  WorkspaceSummary,
  ViewDescriptor,
  InspectableObjectSummary,
  SpotterResult,
  QualityIssueItem,
  QualityGateSummary,
  DesignFinding,
  QualitySeverity,
  SeverityCounts,
  QualitySummaryBlockBody,
  QualityIssueDetailBlockBody,
  LensDescriptor,
  LensResult,
  IdentityBlockBody,
  CallMetricsBlockBody,
  SignatureBlockBody,
  RelationItem,
  CallListBlockBody,
  SourceLine,
  SourceSliceBlockBody,
  SymbolQualityIdentityBlockBody,
  FileQualityIdentityBlockBody,
  ScopeQualityIdentityBlockBody,
  QualityGateBlockBody,
  IssuesListBlockBody,
  IssueIdentityBlockBody,
  IssueLocationBlockBody,
  IssueMessageBlockBody,
  RuleIdentityBlockBody,
  FileIdentityBlockBody,
  KindsBreakdownBlockBody,
  FileSymbolItem,
  FileSymbolsBlockBody,
  ScopeIdentityBlockBody,
  ScopeFilesBlockBody,
  CrossScopeEntry,
  CrossScopeBlockBody,
  HotspotItem,
  HotspotsBlockBody,
  ViewBlock,
  UnknownViewBlock,
  ViewBlockAny,
  ViewBlockBodyById,
  ContextualView,
  OpenWorkspaceRequest,
  IndexWorkspaceRequest,
  ExplorationSessionDto,
  GenerateArtifactRequest,
  DecisionArtifactSummary,
  HealthResponse,
  AskResponse,
  GraphNode,
  GraphEdge,
  GraphNodeStyleClass,
  GraphEdgeStyleClass,
  SubgraphResponse,
  RationaleViewPayload,
  ContextualGraphResponse,
  ParentSection,
  ChildrenSection,
  SameLevelSection,
  // ---- multimodal (T17) ----
  NodeKind,
  EdgeKind,
  MultimodalNode,
  MultimodalEdge,
  GraphSearchResult,
  GraphSearchResponse,
  // ---- Landing Page (E4 ADR-039) ----
  GodNodeEntry,
  LandingPayload,
  // ---- Architecture View — E5 ADR-039 ----
  ArchitecturePayload,
} from "./schemas";

// ---- WASM graph algorithm outputs (ADR-047) ----
//
// These match the Rust `PageRankOutput` and `GodNodesOutput` structs
// defined in `crates/cognicode-graph-algos/src/protocol.rs` and exposed
// by the WASM module at `crates/cognicode-graph-wasm/src/lib.rs`.
//
// IMPORTANT: The WASM `GodNodeEntry` shape is { id, score } — NOT the same
// as the backend `GodNodeEntry` from schemas.ts (which has an extra `label`
// field). The WASM types are used internally by `useGraphAlgorithms`; the
// backend types flow through the public API via `LandingPayload`.

/**
 * PageRank scores keyed by node id.
 * Matches Rust `PageRankOutput` from `protocol.rs`.
 */
export interface WasmPageRankOutput {
  scores: Record<string, number>;
}

/**
 * A god node — a symbol with PageRank above the configured percentile threshold.
 * Matches Rust `GodNodeEntry` from `protocol.rs`.
 *
 * Note: This is the WASM output shape { id, score }. The backend API
 * `GodNodeEntry` in schemas.ts additionally includes a `label` field.
 */
export interface WasmGodNodeEntry {
  id: string;
  score: number;
}

/**
 * Output shape for `god_nodes` from the WASM module.
 * Matches Rust `GodNodesOutput` from `protocol.rs`.
 */
export interface WasmGodNodesOutput {
  nodes: WasmGodNodeEntry[];
}

// ---- Community detection types (ADR-048) ----
// These match the Rust protocol types in `crates/cognicode-graph-wasm/src/protocol.rs`.

/**
 * A single community — list of node IDs.
 * Matches Rust `Community` from `protocol.rs`.
 */
export interface Community {
  node_ids: string[];
}

/**
 * Output shape for `communities` from the WASM module.
 * Matches Rust `CommunitiesOutput` from `protocol.rs`.
 */
export interface CommunitiesOutput {
  communities: Community[];
}

/**
 * A god node within a specific community.
 * Matches Rust `CommunityGodNode` from `protocol.rs`.
 */
export interface CommunityGodNode {
  community_index: number;
  id: string;
  score: number;
}

/**
 * Output shape for `community_god_nodes` from the WASM module.
 * Matches Rust `CommunityGodNodesOutput` from `protocol.rs`.
 */
export interface CommunityGodNodesOutput {
  nodes: CommunityGodNode[];
}

/**
 * A surprising cross-community edge.
 * Matches Rust `SurprisingEdge` from `protocol.rs`.
 */
export interface SurprisingEdge {
  source_id: string;
  target_id: string;
  score: number;
}

/**
 * Output shape for `surprising_connections` from the WASM module.
 * Matches Rust `SurprisingConnectionsOutput` from `protocol.rs`.
 */
export interface SurprisingConnectionsOutput {
  edges: SurprisingEdge[];
}

/**
 * Options for `communities` (Label Propagation).
 */
export interface CommunitiesOptions {
  maxIterations?: number; // default 100
}

/**
 * Options for `community_god_nodes`.
 */
export interface CommunityGodNodesOptions {
  percentile?: number; // default 0.95
}

/**
 * Options for `surprising_connections`.
 */
export interface SurprisingConnectionsOptions {
  limit?: number; // default 10
}

export {
  graphStatusSchema,
  inspectableObjectTypeSchema,
  relationDirectionSchema,
  findingSeveritySchema,
  artifactFormatSchema,
  lineRangeSchema,
  propertySchema,
  objectIdentityEntrySchema,
  evidenceBlockSchema,
  typedRelationSchema,
  workspaceSummarySchema,
  viewDescriptorSchema,
  inspectableObjectSummarySchema,
  spotterResultSchema,
  qualityIssueItemSchema,
  qualityGateSummarySchema,
  designFindingSchema,
  qualitySeveritySchema,
  lensDescriptorSchema,
  lensResultSchema,
  contextualViewSchema,
  openWorkspaceRequestSchema,
  indexWorkspaceRequestSchema,
  explorationSessionSchema,
  generateArtifactRequestSchema,
  decisionArtifactSummarySchema,
  healthResponseSchema,
  askResponseSchema,
  viewBlockSchema,
  viewBlockAnySchema,
  unknownViewBlockSchema,
  graphNodeSchema,
  graphEdgeSchema,
  graphNodeStyleClassSchema,
  graphEdgeStyleClassSchema,
  subgraphResponseSchema,
  rationaleViewPayloadSchema,
  contextualGraphResponseSchema,
  parentSectionSchema,
  childrenSectionSchema,
  sameLevelSectionSchema,
  // ---- multimodal (T17) ----
  nodeKindSchema,
  edgeKindSchema,
  multimodalNodeSchema,
  multimodalEdgeSchema,
  graphSearchResultSchema,
  graphSearchResponseSchema,
  // ---- Landing Page (E4 ADR-039) ----
  godNodeEntrySchema,
  landingPayloadSchema,
  // ---- Architecture View — E5 ADR-039 ----
  architecturePayloadSchema,
} from "./schemas";
