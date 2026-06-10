use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSummary {
    pub id: String,
    pub root_path: String,
    pub graph_status: GraphStatus,
    pub indexed_at: Option<String>,
    pub symbol_count: usize,
    pub relation_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphStatus {
    Missing,
    Stale,
    Ready,
    Indexing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotterResult {
    pub object: InspectableObjectSummary,
    pub score: f32,
    pub match_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectableObjectSummary {
    pub id: String,
    pub object_type: InspectableObjectType,
    pub label: String,
    pub subtitle: String,
    pub properties: Vec<Property>,
    pub available_views: Vec<ViewDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectableObjectType {
    Workspace,
    Scope,
    Symbol,
    File,
    Module,
    Evidence,
    DecisionArtifact,
    /// A single quality issue, addressed by its primary key.
    /// Surface name is `quality_issue` (snake_case) so MCP/JSON callers
    /// can distinguish it from a regular `file` / `symbol`.
    QualityIssue,
    /// A quality rule, addressed by its rule id.
    Rule,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewDescriptor {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Property {
    pub key: String,
    pub value: serde_json::Value,
    pub value_type: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedRelation {
    pub relation_type: String,
    pub direction: RelationDirection,
    pub target_object_id: String,
    pub target_label: String,
    pub evidence_ids: Vec<String>,
    /// How the edge was obtained (e.g. `"Extracted"`, `"Inferred"`,
    /// `"Ambiguous"`). `#[serde(default)]` keeps the field backward
    /// compatible — payloads produced by pre-change builders (no field)
    /// deserialize cleanly with `None`, and JSON consumers that never
    /// read the field keep working. Populated by the call-graph view
    /// builder when the underlying repository is metadata-aware;
    /// `None` for mocks and other adapters that don't implement
    /// [`crate::ports::MetadataAwareRepository`].
    #[serde(default)]
    pub provenance: Option<String>,
    /// Edge trust score in `[0.0, 1.0]`, sourced from the per-edge
    /// `confidence` assigned by `ConfidenceRules` at edge creation.
    /// `#[serde(default)]` keeps backward compatibility with payloads
    /// that pre-date this field. Clamping/normalization is the
    /// producer's responsibility — values flow through as recorded.
    #[serde(default)]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationDirection {
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceBlock {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub file: Option<String>,
    pub line_range: Option<LineRange>,
    pub source_tool_or_query: String,
    pub confidence: Option<f32>,
    /// Freshness signal: `"fresh"`, `"stale"`, `"unknown"`, or `None` for legacy
    /// payloads. `#[serde(default)]` keeps the field backward compatible —
    /// clients that never send or read it see `None`/nothing and keep working.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<String>,
    /// How the underlying edge was obtained (e.g. `"Extracted"`,
    /// `"Inferred"`, `"Ambiguous"`). Mirrors the
    /// [`TypedRelation::provenance`] field for consistency — when a
    /// builder populates the per-edge confidence, it also populates
    /// the source provenance here. `None` when the view builder could
    /// not resolve a metadata-aware repository (mock / legacy path).
    /// `#[serde(default, skip_serializing_if = "Option::is_none")]`
    /// mirrors the `freshness` pattern: missing on the wire for
    /// pre-change payloads, `null` in JSON for populated-but-empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<String>,
}

/// One resolved inspectable object, as captured in an [`ExplorationPath`].
///
/// `id` is the canonical MVP id (`symbol:{file}:{name}:{line}`).
/// `natural_key` is the equivalent `SymbolId` form (`{file}:{name}:{line}`) —
/// the shape consumed by the call graph repository.
/// `first_seen` is the timestamp of the path that first surfaced the object;
/// Phase 1C sets it to the exploration's `created_at`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectIdentityEntry {
    pub id: String,
    pub object_type: String,
    pub natural_key: String,
    pub first_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextualView {
    pub object_id: String,
    pub view_id: String,
    pub title: String,
    pub blocks: Vec<ViewBlock>,
    pub relations: Vec<TypedRelation>,
    pub evidence: Vec<EvidenceBlock>,
    /// Design findings produced by a lens when this view is the result of
    /// `apply_lens`. Empty for non-lens views. The field is `#[serde(default)]`
    /// so old fixtures (Phase 1/2/3) that don't carry it deserialize cleanly
    /// with an empty `Vec` — backward compatibility is the contract.
    #[serde(default)]
    pub findings: Vec<DesignFinding>,
}

// ============================================================================
// Phase 4 — Design Lenses
// ============================================================================

/// Severity hint for a `DesignFinding`. The roadmap frames lens output as
/// "hypotheses, not verdicts" — this enum is the only scalar classification
/// callers can rely on. Order matters: `Critical > Warning > Info` when
/// callers cap or sort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Info,
    Warning,
    Critical,
}

impl FindingSeverity {
    /// Numeric rank for sorting: `Critical = 0`, `Warning = 1`, `Info = 2`.
    /// Lower is "more important" — sorting ASC puts Critical first.
    pub fn rank(self) -> u8 {
        match self {
            Self::Critical => 0,
            Self::Warning => 1,
            Self::Info => 2,
        }
    }
}

/// One structured observation produced by a lens.
///
/// `DesignFinding` is a HYPOTHESIS, not a verdict. The `hypothesis` text
/// should frame the observation as a question or a tentative interpretation
/// (e.g. "This function may be a hotspot — 12 callers across 5 modules").
/// Callers should never display the hypothesis as a definitive claim.
///
/// `confidence` is in `[0.0, 1.0]`. Lenses use data availability to score
/// it: presence of quality findings raises confidence; their absence lowers
/// it but never below `0.0`.
///
/// `object_ids` and `evidence_ids` are stable references to the inspectable
/// objects and evidence blocks the lens considered. Callers can cross-link
/// to existing context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignFinding {
    pub id: String,
    pub lens_id: String,
    pub title: String,
    pub hypothesis: String,
    pub severity: FindingSeverity,
    pub confidence: f32,
    pub object_ids: Vec<String>,
    pub evidence_ids: Vec<String>,
}

/// Lightweight metadata for a registered lens, surfaced via
/// `GET /api/objects/{id}/lenses`. `applicable_types` tells the caller which
/// `InspectableObjectType`s the lens can run against — the service filters
/// by this before returning descriptors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LensDescriptor {
    pub id: String,
    pub name: String,
    pub description: String,
    pub applicable_types: Vec<InspectableObjectType>,
}

/// Output of `apply_lens` — the lens's own shape, separate from
/// `ContextualView`. The API returns this directly.
///
/// `summary` is a one-line human description of what the lens did (e.g.
/// "Detected 3 cross-scope relations and 1 high fan-out symbol").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LensResult {
    pub lens_id: String,
    pub findings: Vec<DesignFinding>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewBlock {
    pub id: String,
    pub title: String,
    pub body: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationPath {
    pub id: String,
    pub workspace_id: String,
    pub columns: Vec<ExplorationColumn>,
    /// Resolved objects the user touched in this path. The field is
    /// `#[serde(default)]` so persisted/legacy paths without it still
    /// deserialize cleanly — they get an empty `Vec`.
    #[serde(default)]
    pub objects: Vec<ObjectIdentityEntry>,
    pub lens: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationColumn {
    pub object_id: String,
    pub active_view: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenWorkspaceRequest {
    pub root_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexWorkspaceRequest {
    pub strategy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveExplorationRequest {
    pub workspace_id: String,
    pub columns: Vec<ExplorationColumn>,
    pub lens: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateArtifactRequest {
    pub format: ArtifactFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactFormat {
    Markdown,
    Html,
    JsonReplay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionArtifactSummary {
    pub id: String,
    pub format: ArtifactFormat,
    pub title: String,
    pub content: String,
}

// ============================================================================
// Phase 6 — MoldQL
// ============================================================================

/// Wire-side DTO mirroring [`crate::moldql::MoldQLResult`]. Mirrored so
/// the API and MCP layers can serialise the result without taking a
/// dependency on the executor's internal type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoldQLResultDto {
    /// The query string the executor ran (echoed back for verification).
    pub query: String,
    /// Matched items, in executor order.
    pub items: Vec<MoldQLItemDto>,
    /// Total number of items matched.
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoldQLItemDto {
    /// MVP id of the matched object.
    pub object_id: String,
    /// Object type tag.
    pub object_type: InspectableObjectType,
    /// Human-readable label.
    pub label: String,
    /// Optional lens output (`<lens_id>: <summary>`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl From<crate::moldql::MoldQLItem> for MoldQLItemDto {
    fn from(item: crate::moldql::MoldQLItem) -> Self {
        Self {
            object_id: item.object_id,
            object_type: item.object_type,
            label: item.label,
            detail: item.detail,
        }
    }
}

impl From<crate::moldql::MoldQLResult> for MoldQLResultDto {
    fn from(r: crate::moldql::MoldQLResult) -> Self {
        Self {
            query: r.query,
            total: r.total,
            items: r.items.into_iter().map(MoldQLItemDto::from).collect(),
        }
    }
}

// ============================================================================
// Named Views — Phase-3 add to the explorer surface
// ============================================================================
//
// A NamedView is the persisted shape of a saved graph projection. The
// four-tuple `(level, lens, focus_node, max_depth)` is the minimum
// information needed to re-invoke the `contextual_view` pipeline;
// everything else (`name`, `description`, `workspace_id`, `owner`,
// `created_at`) is metadata. The descriptor variant is the list
// response — it carries the same fields, but `description` is
// truncated in the service layer to keep payloads lean.

/// Full persistence shape of a saved named view. Returned by
/// `view_save` (the `id` and `created_at` are server-assigned on
/// insert) and by `view_load` (re-fetched from PG).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NamedView {
    pub id: String,
    pub workspace_id: String,
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub level: String,
    pub lens: String,
    pub focus_node: String,
    pub max_depth: i32,
    pub created_at: String,
}

/// List response shape for `view_list`. The descriptor carries the
/// same fields as [`NamedView`] — the truncation in the service
/// layer keeps `description` ≤ 201 chars (200 + `…`) when the
/// stored text is longer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NamedViewDescriptor {
    pub id: String,
    pub workspace_id: String,
    pub owner: String,
    pub name: String,
    pub description: Option<String>,
    pub level: String,
    pub lens: String,
    pub focus_node: String,
    pub max_depth: i32,
    pub created_at: String,
}

/// Truncate a free-form description for the list path. Returns
/// `Some(s)` unchanged when `s.chars().count() <= max`; otherwise
/// returns `Some(s)` with the original UTF-8 trimmed to `max`
/// boundary chars and `"…"` (U+2026) appended. Returns `None`
/// when `s` is `None`.
///
/// The truncation is character-aware (counted via `chars().count()`)
/// so multibyte sequences are never split mid-codepoint.
pub fn truncate_description(s: Option<String>, max: usize) -> Option<String> {
    let s = s?;
    if s.chars().count() <= max {
        return Some(s);
    }
    let truncated: String = s.chars().take(max).collect();
    Some(format!("{truncated}\u{2026}"))
}

// ============================================================================
// Subgraph — visualization-stack Phase 1
// ============================================================================

/// One node in a sub-graph projection. Carries a `style_class` derived
/// from the underlying `SymbolKind` so the front-end cytoscape
/// stylesheet can map it without re-classifying on every render.
///
/// The shape is a strict wire contract — `subgraphResponseSchema` on
/// the front-end mirrors these fields. Any change here must be paired
/// with a `zod` schema update and a new test in
/// `crates/cognicode-explorer/src/api_graph_tests.rs`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphNode {
    /// Canonical id of the symbol. Same value used as
    /// `edge.source`/`edge.target` so the front-end can wire edges
    /// to nodes by equality.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Kind of node (function, module, …). The original kind from
    /// the repository — the `style_class` is the *derived* bucket.
    pub kind: String,
    /// Optional file the symbol lives in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Optional 1-based line number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Cytoscape-friendly bucket. One of `function`, `module`,
    /// `external` (see `api::style_class_for`).
    pub style_class: String,
}

/// One edge in a sub-graph projection. Source/target reference
/// `GraphNode::id` values by equality.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphEdge {
    /// Source node id (matches a `GraphNode::id`).
    pub source: String,
    /// Target node id (matches a `GraphNode::id`).
    pub target: String,
    /// Original relation label from the repository.
    pub relation: String,
    /// Cytoscape-friendly bucket. One of `edge.calls`,
    /// `edge.implements`, `edge.uses` (see `api::edge_style_class_for`).
    pub style_class: String,
}

/// Response payload of `GET /api/graph/:id/subgraph`.
///
/// `truncated_reason` is `Some("node_cap")` whenever `truncated` is
/// `true`. We use `#[serde(skip_serializing_if = "Option::is_none")]`
/// so the field is absent on the wire when there's no truncation —
/// cleaner clients and smaller payloads.
///
/// `corroboration_scores` is an optional map of edge composite key
/// (`"source->target"`) → score in `[0.0, 1.0]`. It is populated by
/// the rationale endpoint; other endpoints leave it empty.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubgraphResponse {
    /// Echo of the requested `:id` so the client can pair request and
    /// response without re-encoding.
    pub root: String,
    /// Reachable nodes (root included). Every `edge.source` and
    /// `edge.target` MUST point at one of these ids — the handler
    /// filters out dangling edges after truncation.
    pub nodes: Vec<GraphNode>,
    /// Reachable edges.
    pub edges: Vec<GraphEdge>,
    /// `true` when the reachable set exceeded `max_nodes` and the
    /// handler truncated it.
    pub truncated: bool,
    /// Reason for truncation, when `truncated` is `true`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncated_reason: Option<String>,
    /// Per-edge corroboration scores, keyed by `"source->target"`.
    /// Populated by the rationale endpoint; empty otherwise.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub corroboration_scores: HashMap<String, f64>,
}

/// Payload for a loaded rationale view. Wraps the subgraph response
/// with summary corroboration metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RationaleViewPayload {
    /// The rationale sub-graph (nodes + edges + per-edge scores).
    pub subgraph: SubgraphResponse,
    /// Per-edge corroboration scores (same as `subgraph.corroboration_scores`).
    /// Duplicated for top-level convenience.
    pub corroboration_scores: HashMap<String, f64>,
    /// Total number of edges in the rationale sub-graph.
    pub source_count: u32,
}

// ============================================================================
// Contextual Graph — visualization-stack Phase 2 (Contextual Views)
// ============================================================================
//
// Bundled response for `GET /api/graph/:id/contextual`.
// Reuses `GraphNode` / `GraphEdge` (ADR-CX-2). Name disambiguates from
// the text-based `ContextualView` DTO higher up (ADR-CX-5).
//
// The four sections compose a file-level contextual projection:
// - `focus_node`  : the resolved symbol
// - `parent`      : the containing file via `lives_in` (null = orphan)
// - `children`    : sibling symbols in the same file (null = orphan)
// - `same_level`  : BFS of callers + callees up to `depth` hops
//
// Truncation contract: `truncated=true` means `len(children.nodes) +
// len(same_level.nodes)` would have exceeded `max_nodes` and we
// trimmed it; `truncation_reason` carries the cause for the UI.

/// Response payload of `GET /api/graph/:id/contextual`.
///
/// All four sections (focus, parent, children, same_level) reuse the
/// existing `GraphNode` / `GraphEdge` types — no new node/edge
/// schemas are introduced (ADR-CX-2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ContextualGraphResponse {
    /// The resolved symbol the user clicked on.
    pub focus_node: GraphNode,
    /// Containing file via `lives_in` (null when the symbol is an
    /// orphan — no `lives_in` edge).
    pub parent: Option<ParentSection>,
    /// Sibling symbols in the same file, with the focus node removed
    /// (null when the symbol is an orphan).
    pub children: Option<ChildrenSection>,
    /// Same-level call neighbours (callers + callees BFS up to
    /// `depth` hops), bounded by `max_nodes`.
    pub same_level: SameLevelSection,
    /// Reserved for future C4 levels; always `"file"` in Phase 1.
    pub level: String,
    /// `true` when the BFS/children combined set exceeded
    /// `max_nodes` and the service trimmed it.
    pub truncated: bool,
    /// Reason for truncation, when `truncated` is `true`. Typically
    /// `"max_nodes_exceeded"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation_reason: Option<String>,
}

/// `parent` section — the file the focus symbol lives in, plus the
/// `lives_in` edge connecting them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ParentSection {
    pub node: GraphNode,
    pub edge: GraphEdge,
}

/// `children` section — the sibling symbols in the same file as the
/// focus, with the `lives_in` edges connecting each sibling to the
/// focus.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChildrenSection {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// `same_level` section — the BFS of callers and callees around the
/// focus, bounded by `max_nodes` (combined with children).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SameLevelSection {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[cfg(test)]
mod named_view_tests {
    use super::*;

    #[test]
    fn named_view_serde_roundtrip() {
        let v = NamedView {
            id: "11111111-1111-1111-1111-111111111111".to_string(),
            workspace_id: "w1".to_string(),
            owner: "u1".to_string(),
            name: "hotspots".to_string(),
            description: Some("a saved view".to_string()),
            level: "function".to_string(),
            lens: "callgraph".to_string(),
            focus_node: "crate::foo::bar".to_string(),
            max_depth: 3,
            created_at: "2026-06-09T00:00:00Z".to_string(),
        };
        let s = serde_json::to_string(&v).expect("serialize");
        let back: NamedView = serde_json::from_str(&s).expect("deserialize");
        assert_eq!(v, back);
    }

    #[test]
    fn truncate_description_preserves_short_text() {
        let s = Some("hello".to_string());
        assert_eq!(truncate_description(s, 200), Some("hello".to_string()));
    }

    #[test]
    fn truncate_description_truncates_long_text_with_ellipsis() {
        let long: String = "a".repeat(1500);
        let out = truncate_description(Some(long.clone()), 200).expect("some");
        // 200 chars + the ellipsis (1 char) = 201 total chars
        assert_eq!(out.chars().count(), 201);
        assert!(out.ends_with('\u{2026}'));
        // The non-ellipsis prefix is the first 200 chars of the input.
        let prefix: String = out.chars().take(200).collect();
        let expected_prefix: String = long.chars().take(200).collect();
        assert_eq!(prefix, expected_prefix);
    }

    #[test]
    fn truncate_description_none_passthrough() {
        assert_eq!(truncate_description(None, 200), None);
    }
}
