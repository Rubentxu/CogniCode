use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// Ports for InspectionTarget and ViewContext
use crate::ports::quality_repository::{QualityIssue, QualityRepository};
use crate::ports::source_reader::SourceReader;
use crate::ports::symbol_repository::{ResolvedSymbol, SymbolRepository};
use cognicode_core::domain::traits::GraphQueryPort;

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

/// A ViewSpec summary for Spotter search results.
/// Includes the minimal set of fields needed to display a hit and open the spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewSpecSummary {
    pub id: String,
    pub title: String,
    pub view_kind: ViewKind,
    pub applies_to: InspectableObjectType,
    pub owner: String,
    pub updated_at: String,
}

/// Discriminated union of symbol/file hits and ViewSpec hits for Spotter results.
/// The frontend can switch on `kind` to render each variant appropriately.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "result")]
pub enum SpotterSearchResult {
    /// A symbol or file hit from the code graph.
    #[serde(rename = "symbol")]
    Symbol(SpotterResult),
    /// A runtime ViewSpec hit from the store.
    #[serde(rename = "viewspec")]
    ViewSpec(ViewSpecSummary),
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ViewDescriptor {
    pub id: String,
    pub title: String,
    /// Whether this is a built-in view (`true`) or a runtime user-defined view (`false`).
    /// Phase 4+: included so the frontend can badge runtime views.
    /// Default `true` when absent (backward compat).
    #[serde(default)]
    pub is_builtin: bool,
    /// Source discriminator for runtime views. `"runtime"` for user-defined specs;
    /// absent (or `null`) for built-ins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
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
    /// [`cognicode_core::domain::traits::GraphQueryPort`].
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    /// Visual rendering strategy for this view. `#[serde(default)]` preserves
    /// backward compatibility with payloads that predate this field — they
    /// deserialize with `RendererKind::Json`, the most common fallback.
    #[serde(default)]
    pub renderer_kind: RendererKind,
}

// ============================================================================
// Phase 1 — View Seam Consolidation: InspectionTarget + ViewContext
// ============================================================================
//
// InspectionTarget carries the pre-resolved object data passed to ViewExecutor::build().
// The service resolves identity BEFORE calling build(); capabilities MUST NOT re-resolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InspectionTarget {
    Symbol(ResolvedSymbol),
    File {
        path: String,
        symbols: Vec<ResolvedSymbol>,
    },
    Scope {
        path: String,
        files: Vec<String>,
        symbols: Vec<ResolvedSymbol>,
    },
    Issue(QualityIssue),
    Rule { rule_id: String },
}

/// Context passed to ViewExecutor::build(). The service populates all
/// fields before calling build(); capabilities MUST NOT re-resolve identity.
pub struct ViewContext<'a> {
    pub target: &'a InspectionTarget,
    pub repo: &'a dyn SymbolRepository,
    pub reader: &'a dyn SourceReader,
    pub quality: Option<&'a dyn QualityRepository>,
    /// Optional graph query port for traversal and navigation queries.
    /// `None` when no call graph is wired.
    pub graph_query: Option<&'a dyn GraphQueryPort>,
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
    /// Navigation mode for reconstruction (added jun-15, ADR-016).
    /// `#[serde(default)]` so paths saved before this field existed
    /// are treated as column mode.
    #[serde(default = "default_navigation_mode")]
    pub navigation_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationColumn {
    pub object_id: String,
    pub active_view: Option<String>,
}

// ============================================================================
// ExplorationSession — semantic exploration history (ADR-016 Fase 3)
// ============================================================================

/// A single step in the user's exploration: which object they looked
/// at, which view they used, and when. `query` stores the user's
/// natural-language question if this step came from the Ask panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationEvent {
    pub object_id: String,
    pub view_id: Option<String>,
    pub query: Option<String>,
    pub ts: String, // ISO-8601
}

/// Ordered log of exploration events. The `navigation_mode` field
/// records whether the user was in column or pane-stack mode when
/// the session was saved.
///
/// Unlike `ExplorationPath` (which models a linear drill-down),
/// `ExplorationSession` models the raw sequence of object
/// inspections. A pane-stack navigation can be reconstructed from
/// the session by grouping consecutive events with the same
/// `object_id` into a pane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationSession {
    pub id: String,
    pub workspace_id: String,
    pub events: Vec<ExplorationEvent>,
    /// Mode for reconstruction: "column" or "pane-stack".
    /// `#[serde(default)]` so v1 sessions without this field are
    /// treated as column mode.
    #[serde(default = "default_navigation_mode")]
    pub navigation_mode: String,
    pub created_at: String,
}

fn default_navigation_mode() -> String { "column".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveExplorationSessionRequest {
    pub workspace_id: String,
    pub events: Vec<ExplorationEvent>,
    pub navigation_mode: String,
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
    /// Navigation mode for reconstruction (added jun-15, ADR-016).
    /// Defaults to "column" when omitted by old clients.
    #[serde(default = "default_navigation_mode")]
    pub navigation_mode: String,
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
///
/// **Deprecated** in favor of [`ViewSpec`] (ADR-008 Phase 2).
/// Use [`NamedView::to_view_spec`] to migrate existing rows.
#[deprecated(since = "0.6.0", note = "Use ViewSpec / view_spec_* instead")]
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

impl NamedView {
    /// Convert this `NamedView` to a [`ViewSpec`].
    ///
    /// Maps the four-tuple `(level, lens, focus_node, max_depth)` to
    /// the equivalent `ViewSpec` fields. The `data_source` is set to
    /// `DataSource::Moldql { query: "" }` (filled in later via the
    /// authoring wizard). The `renderer_kind` defaults to `RendererKind::Json`.
    ///
    /// Level → `InspectableObjectType` map:
    /// `function|method` → `Symbol`; `file` → `File`; `module|scope` →
    /// `Scope`; `system` → `Workspace`; unknown → `Symbol` (safe default).
    ///
    /// Lens → `ViewKind` map:
    /// `callgraph` → `CallGraph`; `overview` → `Custom("overview")`;
    /// `quality` → `QualityHotspots`; unknown → `Custom(lens)`.
    ///
    /// # Example
    ///
    /// ```
    /// use cognicode_explorer::dto::{NamedView, ViewKind, DataSource, RendererKind};
    ///
    /// let nv = NamedView {
    ///     id: "test-id".to_string(),
    ///     workspace_id: "ws1".to_string(),
    ///     owner: "alice".to_string(),
    ///     name: "My View".to_string(),
    ///     description: None,
    ///     level: "function".to_string(),
    ///     lens: "callgraph".to_string(),
    ///     focus_node: "crate::foo".to_string(),
    ///     max_depth: 3,
    ///     created_at: "2024-01-01T00:00:00Z".to_string(),
    /// };
    /// let spec = nv.to_view_spec();
    /// assert_eq!(spec.title, "My View");
    /// assert_eq!(spec.view_kind, ViewKind::CallGraph);
    /// assert!(matches!(spec.data_source, DataSource::Moldql { .. }));
    /// assert_eq!(spec.renderer_kind, RendererKind::Json);
    /// ```
    #[must_use]
    pub fn to_view_spec(&self) -> ViewSpec {
        let applies_to = level_to_inspectable_object_type(&self.level);
        let view_kind = lens_to_view_kind(&self.lens);
        let props = serde_json::json!({
            "focus_node": &self.focus_node,
            "max_depth": self.max_depth,
        });

        ViewSpec {
            id: self.id.clone(),
            title: self.name.clone(),
            applies_to,
            view_kind,
            data_source: DataSource::Moldql {
                query: String::new(),
            },
            transform: None,
            renderer_kind: RendererKind::Json,
            props,
            created_at: self.created_at.clone(),
            updated_at: self.created_at.clone(),
            owner: self.owner.clone(),
        }
    }
}

/// Convert a `NamedView.level` string to an `InspectableObjectType`.
fn level_to_inspectable_object_type(level: &str) -> InspectableObjectType {
    match level {
        "function" | "method" => InspectableObjectType::Symbol,
        "file" => InspectableObjectType::File,
        "module" | "scope" => InspectableObjectType::Scope,
        "system" => InspectableObjectType::Workspace,
        _ => InspectableObjectType::Symbol, // safe default
    }
}

/// Convert a `NamedView.lens` string to a `ViewKind`.
fn lens_to_view_kind(lens: &str) -> ViewKind {
    match lens {
        "callgraph" => ViewKind::CallGraph,
        "overview" => ViewKind::Custom("overview".to_string()),
        "quality" => ViewKind::QualityHotspots,
        _ => ViewKind::Custom(lens.to_string()),
    }
}

/// List response shape for `view_list`. The descriptor carries the
/// same fields as [`NamedView`] — the truncation in the service
/// layer keeps `description` ≤ 201 chars (200 + `…`) when the
/// stored text is longer.
///
/// **Deprecated** in favor of [`ViewSpec`] (ADR-008 Phase 2).
#[deprecated(since = "0.6.0", note = "Use ViewSpec / view_spec_* instead")]
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
// Landing Page — E4 ADR-039
// ============================================================================

/// Response payload for `GET /api/workspaces/:id/landing`.
/// Bundles everything the landing page needs in a single round-trip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LandingPayload {
    /// The workspace summary (always present once workspace is open).
    pub workspace: WorkspaceSummary,
    /// Root-level graph nodes (entry points + hot + god nodes).
    pub nodes: Vec<GraphNode>,
    /// Edges connecting the root-level nodes.
    pub edges: Vec<GraphEdge>,
    /// Top-level entry points for this workspace.
    pub entry_points: Vec<InspectableObjectSummary>,
    /// Hot paths for this workspace.
    pub hot_paths: Vec<InspectableObjectSummary>,
    /// God nodes (high PageRank symbols).
    pub god_nodes: Vec<GodNodeEntry>,
    /// Suggested questions for the ask panel.
    pub suggested_questions: Vec<String>,
    /// Current graph status (even when missing/indexing).
    pub graph_status: GraphStatus,
}

/// A god node entry — a high PageRank symbol with its score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GodNodeEntry {
    pub id: String,
    pub label: String,
    pub score: f64,
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

    // --- NamedView::to_view_spec ---

    #[test]
    fn to_view_spec_callgraph_roundtrip() {
        let nv = NamedView {
            id: "11111111-1111-1111-1111-111111111111".to_string(),
            workspace_id: "ws1".to_string(),
            owner: "alice".to_string(),
            name: "hotspots".to_string(),
            description: None,
            level: "function".to_string(),
            lens: "callgraph".to_string(),
            focus_node: "crate::foo::bar".to_string(),
            max_depth: 3,
            created_at: "2026-06-09T00:00:00Z".to_string(),
        };
        let spec = nv.to_view_spec();
        assert_eq!(spec.id, nv.id);
        assert_eq!(spec.title, nv.name);
        assert_eq!(spec.applies_to, InspectableObjectType::Symbol);
        assert_eq!(spec.view_kind, ViewKind::CallGraph);
        assert!(matches!(spec.data_source, DataSource::Moldql { query } if query.is_empty()));
        assert_eq!(spec.renderer_kind, RendererKind::Json);
        let props = serde_json::json!({
            "focus_node": "crate::foo::bar",
            "max_depth": 3,
        });
        assert_eq!(spec.props, props);
    }

    #[test]
    fn to_view_spec_unknown_lens_becomes_custom() {
        let nv = NamedView {
            id: "22222222-2222-2222-2222-222222222222".to_string(),
            workspace_id: "ws1".to_string(),
            owner: "alice".to_string(),
            name: "experimental".to_string(),
            description: None,
            level: "function".to_string(),
            lens: "experimental_lens".to_string(),
            focus_node: "crate::foo".to_string(),
            max_depth: 1,
            created_at: "2026-06-09T00:00:00Z".to_string(),
        };
        let spec = nv.to_view_spec();
        assert!(matches!(spec.view_kind, ViewKind::Custom(s) if s == "experimental_lens"));
    }

    #[test]
    fn to_view_spec_file_level_maps_to_file() {
        let nv = NamedView {
            id: "33333333-3333-3333-3333-333333333333".to_string(),
            workspace_id: "ws1".to_string(),
            owner: "alice".to_string(),
            name: "file view".to_string(),
            description: None,
            level: "file".to_string(),
            lens: "overview".to_string(),
            focus_node: "src/main.rs".to_string(),
            max_depth: 0,
            created_at: "2026-06-09T00:00:00Z".to_string(),
        };
        let spec = nv.to_view_spec();
        assert_eq!(spec.applies_to, InspectableObjectType::File);
    }

    #[test]
    fn to_view_spec_module_level_maps_to_scope() {
        let nv = NamedView {
            id: "44444444-4444-4444-4444-444444444444".to_string(),
            workspace_id: "ws1".to_string(),
            owner: "alice".to_string(),
            name: "module view".to_string(),
            description: None,
            level: "module".to_string(),
            lens: "overview".to_string(),
            focus_node: "crate::foo".to_string(),
            max_depth: 0,
            created_at: "2026-06-09T00:00:00Z".to_string(),
        };
        let spec = nv.to_view_spec();
        assert_eq!(spec.applies_to, InspectableObjectType::Scope);
    }

    #[test]
    fn to_view_spec_system_level_maps_to_workspace() {
        let nv = NamedView {
            id: "55555555-5555-5555-5555-555555555555".to_string(),
            workspace_id: "ws1".to_string(),
            owner: "alice".to_string(),
            name: "system view".to_string(),
            description: None,
            level: "system".to_string(),
            lens: "overview".to_string(),
            focus_node: "/".to_string(),
            max_depth: 0,
            created_at: "2026-06-09T00:00:00Z".to_string(),
        };
        let spec = nv.to_view_spec();
        assert_eq!(spec.applies_to, InspectableObjectType::Workspace);
    }

    #[test]
    fn to_view_spec_unknown_level_defaults_to_symbol() {
        let nv = NamedView {
            id: "66666666-6666-6666-6666-666666666666".to_string(),
            workspace_id: "ws1".to_string(),
            owner: "alice".to_string(),
            name: "unknown level".to_string(),
            description: None,
            level: "totally_unknown".to_string(),
            lens: "overview".to_string(),
            focus_node: "crate::foo".to_string(),
            max_depth: 0,
            created_at: "2026-06-09T00:00:00Z".to_string(),
        };
        let spec = nv.to_view_spec();
        assert_eq!(spec.applies_to, InspectableObjectType::Symbol);
    }

    #[test]
    fn to_view_spec_quality_lens_maps_to_quality_hotspots() {
        let nv = NamedView {
            id: "77777777-7777-7777-7777-777777777777".to_string(),
            workspace_id: "ws1".to_string(),
            owner: "alice".to_string(),
            name: "quality view".to_string(),
            description: None,
            level: "function".to_string(),
            lens: "quality".to_string(),
            focus_node: "crate::foo".to_string(),
            max_depth: 0,
            created_at: "2026-06-09T00:00:00Z".to_string(),
        };
        let spec = nv.to_view_spec();
        assert_eq!(spec.view_kind, ViewKind::QualityHotspots);
    }
}

// ============================================================================
// Phase 0: Moldable View Runtime — Domain Vocabulary
// ============================================================================
//
// ViewKind, RendererKind, HierarchyKind, ViewSpec DTO, DataSource,
// Transform, and validation — Phase 0 of the Moldable View Runtime
// roadmap (ADR-008). Zero behaviour change; purely additive vocabulary.

// ============================================================================
// ViewSpec errors
// ============================================================================

/// Validation errors returned by [`ViewSpec::validate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewSpecError {
    /// `title` was empty.
    EmptyTitle,
    /// `title` exceeded 200 characters.
    TitleTooLong,
    /// `applies_to` resolved to an unknown [`InspectableObjectType`].
    UnknownAppliesTo,
    /// `data_source` was a Moldql variant with an empty query string.
    EmptyQuery,
    /// `id` was not a valid UUID.
    InvalidUuid,
}

impl std::fmt::Display for ViewSpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewSpecError::EmptyTitle => write!(f, "title must not be empty"),
            ViewSpecError::TitleTooLong => write!(f, "title must not exceed 200 characters"),
            ViewSpecError::UnknownAppliesTo => write!(f, "applies_to is not a valid object type"),
            ViewSpecError::EmptyQuery => write!(f, "data_source query must not be empty"),
            ViewSpecError::InvalidUuid => write!(f, "id must be a valid UUID"),
        }
    }
}

impl std::error::Error for ViewSpecError {}

impl From<ViewSpecError> for crate::error::ExplorerError {
    fn from(err: ViewSpecError) -> Self {
        crate::error::ExplorerError::InvalidInput(err.to_string())
    }
}

/// One first-class ViewKind — the semantic intent of a view.
///
/// Variants cover the full catalog from ADR-008 §First-class ViewKind.
/// The `Custom(String)` variant absorbs any future / user-defined value
/// without breaking deserialisation, preserving the original tag string
/// for round-trip fidelity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ViewKind {
    // Core
    VerticalSlice,
    CallGraph,
    SeamMap,
    DependencyGraph,
    SourceView,
    DataFlow,
    ImpactRadius,
    DiffView,
    // C4
    C4Context,
    C4Container,
    C4Component,
    C4Code,
    // Quality
    QualityHotspots,
    EvidenceView,
    DecisionGraph,
    // Architecture
    ArchitectureRationale,
    ArchitectureDrift,
    BoundaryMap,
    DependencyPressure,
    ChangeImpactStory,
    OwnershipMap,
    RiskMap,
    DecisionTrace,
    // Development
    TestSlice,
    DebugSlice,
    RefactorPlan,
    CallersAndImplementors,
    UsageExamples,
    ApiSurface,
    DeadCodeCandidates,
    SemanticSearchResults,
    // Living doc
    DocCodeAlignment,
    ExampleObject,
    ComposedNarrative,
    ProjectDiary,
    ConceptMap,
    EvidencePack,
    /// Forward-compatibility arm: any unknown tag is captured here,
    /// preserving the original string for round-trip serialization.
    Custom(String),
}

impl Serialize for ViewKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ViewKind::VerticalSlice => serializer.serialize_str("vertical_slice"),
            ViewKind::CallGraph => serializer.serialize_str("call_graph"),
            ViewKind::SeamMap => serializer.serialize_str("seam_map"),
            ViewKind::DependencyGraph => serializer.serialize_str("dependency_graph"),
            ViewKind::SourceView => serializer.serialize_str("source_view"),
            ViewKind::DataFlow => serializer.serialize_str("data_flow"),
            ViewKind::ImpactRadius => serializer.serialize_str("impact_radius"),
            ViewKind::DiffView => serializer.serialize_str("diff_view"),
            ViewKind::C4Context => serializer.serialize_str("c4_context"),
            ViewKind::C4Container => serializer.serialize_str("c4_container"),
            ViewKind::C4Component => serializer.serialize_str("c4_component"),
            ViewKind::C4Code => serializer.serialize_str("c4_code"),
            ViewKind::QualityHotspots => serializer.serialize_str("quality_hotspots"),
            ViewKind::EvidenceView => serializer.serialize_str("evidence_view"),
            ViewKind::DecisionGraph => serializer.serialize_str("decision_graph"),
            ViewKind::ArchitectureRationale => serializer.serialize_str("architecture_rationale"),
            ViewKind::ArchitectureDrift => serializer.serialize_str("architecture_drift"),
            ViewKind::BoundaryMap => serializer.serialize_str("boundary_map"),
            ViewKind::DependencyPressure => serializer.serialize_str("dependency_pressure"),
            ViewKind::ChangeImpactStory => serializer.serialize_str("change_impact_story"),
            ViewKind::OwnershipMap => serializer.serialize_str("ownership_map"),
            ViewKind::RiskMap => serializer.serialize_str("risk_map"),
            ViewKind::DecisionTrace => serializer.serialize_str("decision_trace"),
            ViewKind::TestSlice => serializer.serialize_str("test_slice"),
            ViewKind::DebugSlice => serializer.serialize_str("debug_slice"),
            ViewKind::RefactorPlan => serializer.serialize_str("refactor_plan"),
            ViewKind::CallersAndImplementors => serializer.serialize_str("callers_and_implementors"),
            ViewKind::UsageExamples => serializer.serialize_str("usage_examples"),
            ViewKind::ApiSurface => serializer.serialize_str("api_surface"),
            ViewKind::DeadCodeCandidates => serializer.serialize_str("dead_code_candidates"),
            ViewKind::SemanticSearchResults => serializer.serialize_str("semantic_search_results"),
            ViewKind::DocCodeAlignment => serializer.serialize_str("doc_code_alignment"),
            ViewKind::ExampleObject => serializer.serialize_str("example_object"),
            ViewKind::ComposedNarrative => serializer.serialize_str("composed_narrative"),
            ViewKind::ProjectDiary => serializer.serialize_str("project_diary"),
            ViewKind::ConceptMap => serializer.serialize_str("concept_map"),
            ViewKind::EvidencePack => serializer.serialize_str("evidence_pack"),
            ViewKind::Custom(s) => serializer.serialize_str(s),
        }
    }
}

impl<'de> Deserialize<'de> for ViewKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "vertical_slice" => Ok(ViewKind::VerticalSlice),
            "call_graph" => Ok(ViewKind::CallGraph),
            "seam_map" => Ok(ViewKind::SeamMap),
            "dependency_graph" => Ok(ViewKind::DependencyGraph),
            "source_view" => Ok(ViewKind::SourceView),
            "data_flow" => Ok(ViewKind::DataFlow),
            "impact_radius" => Ok(ViewKind::ImpactRadius),
            "diff_view" => Ok(ViewKind::DiffView),
            "c4_context" => Ok(ViewKind::C4Context),
            "c4_container" => Ok(ViewKind::C4Container),
            "c4_component" => Ok(ViewKind::C4Component),
            "c4_code" => Ok(ViewKind::C4Code),
            "quality_hotspots" => Ok(ViewKind::QualityHotspots),
            "evidence_view" => Ok(ViewKind::EvidenceView),
            "decision_graph" => Ok(ViewKind::DecisionGraph),
            "architecture_rationale" => Ok(ViewKind::ArchitectureRationale),
            "architecture_drift" => Ok(ViewKind::ArchitectureDrift),
            "boundary_map" => Ok(ViewKind::BoundaryMap),
            "dependency_pressure" => Ok(ViewKind::DependencyPressure),
            "change_impact_story" => Ok(ViewKind::ChangeImpactStory),
            "ownership_map" => Ok(ViewKind::OwnershipMap),
            "risk_map" => Ok(ViewKind::RiskMap),
            "decision_trace" => Ok(ViewKind::DecisionTrace),
            "test_slice" => Ok(ViewKind::TestSlice),
            "debug_slice" => Ok(ViewKind::DebugSlice),
            "refactor_plan" => Ok(ViewKind::RefactorPlan),
            "callers_and_implementors" => Ok(ViewKind::CallersAndImplementors),
            "usage_examples" => Ok(ViewKind::UsageExamples),
            "api_surface" => Ok(ViewKind::ApiSurface),
            "dead_code_candidates" => Ok(ViewKind::DeadCodeCandidates),
            "semantic_search_results" => Ok(ViewKind::SemanticSearchResults),
            "doc_code_alignment" => Ok(ViewKind::DocCodeAlignment),
            "example_object" => Ok(ViewKind::ExampleObject),
            "composed_narrative" => Ok(ViewKind::ComposedNarrative),
            "project_diary" => Ok(ViewKind::ProjectDiary),
            "concept_map" => Ok(ViewKind::ConceptMap),
            "evidence_pack" => Ok(ViewKind::EvidencePack),
            "custom" => Ok(ViewKind::Custom("custom".to_string())),
            other => Ok(ViewKind::Custom(other.to_string())),
        }
    }
}

/// One first-class RendererKind — the visual rendering strategy.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RendererKind {
    Graph,
    Table,
    Tree,
    Code,
    Markdown,
    VegaLite,
    Json,
    Composite,
    /// Forward-compatibility arm: catches any unknown renderer id,
    /// preserving the original string for round-trip serialization.
    Custom(String),
}

impl Default for RendererKind {
    fn default() -> Self {
        RendererKind::Json
    }
}

impl Serialize for RendererKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            RendererKind::Graph => serializer.serialize_str("graph"),
            RendererKind::Table => serializer.serialize_str("table"),
            RendererKind::Tree => serializer.serialize_str("tree"),
            RendererKind::Code => serializer.serialize_str("code"),
            RendererKind::Markdown => serializer.serialize_str("markdown"),
            RendererKind::VegaLite => serializer.serialize_str("vega_lite"),
            RendererKind::Json => serializer.serialize_str("json"),
            RendererKind::Composite => serializer.serialize_str("composite"),
            RendererKind::Custom(s) => serializer.serialize_str(s),
        }
    }
}

impl<'de> Deserialize<'de> for RendererKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "graph" => Ok(RendererKind::Graph),
            "table" => Ok(RendererKind::Table),
            "tree" => Ok(RendererKind::Tree),
            "code" => Ok(RendererKind::Code),
            "markdown" => Ok(RendererKind::Markdown),
            "vega_lite" => Ok(RendererKind::VegaLite),
            "json" => Ok(RendererKind::Json),
            "composite" => Ok(RendererKind::Composite),
            "custom" => Ok(RendererKind::Custom("custom".to_string())),
            other => Ok(RendererKind::Custom(other.to_string())),
        }
    }
}

/// One first-class HierarchyKind — a navigable structural projection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HierarchyKind {
    FileTree,
    ModuleTree,
    TypeHierarchy,
    CallHierarchy,
    PackageGraph,
    C4Hierarchy,
    /// Forward-compatibility arm: catches any unknown hierarchy id,
    /// preserving the original string for round-trip serialization.
    Custom(String),
}

impl Serialize for HierarchyKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            HierarchyKind::FileTree => serializer.serialize_str("file_tree"),
            HierarchyKind::ModuleTree => serializer.serialize_str("module_tree"),
            HierarchyKind::TypeHierarchy => serializer.serialize_str("type_hierarchy"),
            HierarchyKind::CallHierarchy => serializer.serialize_str("call_hierarchy"),
            HierarchyKind::PackageGraph => serializer.serialize_str("package_graph"),
            HierarchyKind::C4Hierarchy => serializer.serialize_str("c4_hierarchy"),
            HierarchyKind::Custom(s) => serializer.serialize_str(s),
        }
    }
}

impl<'de> Deserialize<'de> for HierarchyKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "file_tree" => Ok(HierarchyKind::FileTree),
            "module_tree" => Ok(HierarchyKind::ModuleTree),
            "type_hierarchy" => Ok(HierarchyKind::TypeHierarchy),
            "call_hierarchy" => Ok(HierarchyKind::CallHierarchy),
            "package_graph" => Ok(HierarchyKind::PackageGraph),
            "c4_hierarchy" => Ok(HierarchyKind::C4Hierarchy),
            "custom" => Ok(HierarchyKind::Custom("custom".to_string())),
            other => Ok(HierarchyKind::Custom(other.to_string())),
        }
    }
}

/// Where the data for a [`ViewSpec`] comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DataSource {
    /// A MoldQL query string.
    Moldql { query: String },
    /// Forward-compatibility arm: catches any unknown `kind` value.
    /// The `kind` field is still present in the JSON but is not
    /// captured (serde `other` is a unit-variant catch-all).
    #[serde(other)]
    Other,
}

/// How the data from [`DataSource`] is transformed before rendering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Transform {
    /// No transform — the MoldQL result passes through unchanged.
    /// This is distinct from `transform: None` (Option) which means
    /// "not specified". `Transform::None` represents an explicit user choice
    /// of "no transform" in the authoring wizard (Phase 4).
    None,
    /// A JSONata expression string.
    Jsonata { expression: String },
    /// Forward-compatibility arm: catches any unknown `kind` value.
    #[serde(other)]
    Other,
}

/// A persisted view specification — the core DTO of the Moldable View Runtime.
///
/// `id` is server-assigned UUID on persist; client-suggested on create.
/// `applies_to` narrows which [`InspectableObjectType`] the view works on.
/// `view_kind` names the semantic intent; `renderer_kind` selects the visual
/// strategy; `data_source` + `transform` supply and reshape the data.
/// `owner` is the user who created this spec.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ViewSpec {
    pub id: String,
    pub title: String,
    pub applies_to: InspectableObjectType,
    pub view_kind: ViewKind,
    pub data_source: DataSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<Transform>,
    pub renderer_kind: RendererKind,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub props: serde_json::Value,
    /// ISO-8601 UTC; server-assigned on insert; `#[serde(default)]` allows
    /// clients to omit it on create.
    #[serde(default)]
    pub created_at: String,
    /// ISO-8601 UTC; server-assigned on update; `#[serde(default)]` allows
    /// clients to omit it on create.
    #[serde(default)]
    pub updated_at: String,
    /// The user who owns this spec. Used for ownership checks and Spotter display.
    #[serde(default)]
    pub owner: String,
}

impl ViewSpec {
    /// Validate this spec. Returns `Ok(())` if the spec is well-formed;
    /// `Err(ViewSpecError)` describing the first problem found.
    pub fn validate(&self) -> Result<(), ViewSpecError> {
        if self.title.is_empty() {
            return Err(ViewSpecError::EmptyTitle);
        }
        if self.title.chars().count() > 200 {
            return Err(ViewSpecError::TitleTooLong);
        }
        if let DataSource::Moldql { query } = &self.data_source {
            if query.trim().is_empty() {
                return Err(ViewSpecError::EmptyQuery);
            }
        }
        // Validate id is a valid UUID (basic format check without adding uuid dep)
        if !is_valid_uuid_format(&self.id) {
            return Err(ViewSpecError::InvalidUuid);
        }
        Ok(())
    }
}

/// Basic UUID v4 format validator.
///
/// Checks that the string is exactly 36 characters, with hyphens
/// at positions 8, 13, 18, and 23, and hexadecimal digits elsewhere.
/// This is sufficient for validating user-supplied IDs without pulling
/// in the full `uuid` crate.
fn is_valid_uuid_format(s: &str) -> bool {
    if s.len() != 36 {
        return false;
    }
    let bytes = s.as_bytes();
    // Check hyphen positions: 8, 13, 18, 23
    bytes[8] == b'-'
        && bytes[13] == b'-'
        && bytes[18] == b'-'
        && bytes[23] == b'-'
        // Check all hex digits
        && bytes.iter().enumerate().all(|(i, &b)| {
            if i == 8 || i == 13 || i == 18 || i == 23 {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}

// ============================================================================
// C4 Architecture Drift Detection (E6)
// ============================================================================

/// Kind of architecture drift detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftKind {
    /// Container exists in expected architecture but not in inferred.
    MissingContainer,
    /// Container exists in inferred architecture but not in expected.
    ExtraContainer,
    /// Container exists in both but sub_kind differs.
    WrongSubKind,
}

/// One drift finding comparing expected vs inferred C4 architecture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftFinding {
    /// Kind of drift detected.
    pub kind: DriftKind,
    /// Container name in the expected architecture (or "—" if not expected).
    pub expected: String,
    /// Container name in the inferred architecture (or "—" if not inferred).
    pub actual: String,
    /// Severity: "warning" for Missing/Extra, "info" for WrongSubKind.
    pub severity: String,
    /// Human-readable explanation.
    pub detail: String,
}

/// Report comparing expected C4 architecture against inferred architecture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftReport {
    /// All drift findings.
    pub findings: Vec<DriftFinding>,
    /// Human-readable summary.
    pub summary: String,
    /// Count of missing containers.
    pub missing_containers: usize,
    /// Count of extra containers.
    pub extra_containers: usize,
    /// Count of wrong sub_kind findings.
    pub wrong_sub_kinds: usize,
}

impl Default for DriftReport {
    fn default() -> Self {
        Self {
            findings: Vec::new(),
            summary: "No architecture drift detected".to_string(),
            missing_containers: 0,
            extra_containers: 0,
            wrong_sub_kinds: 0,
        }
    }
}

/// One container entry from `.cognicode/expected-architecture.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedContainer {
    pub name: String,
    pub sub_kind: String,
    #[serde(default)]
    pub purpose: String,
}

/// Parsed form of `.cognicode/expected-architecture.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedArchitecture {
    pub containers: Vec<ExpectedContainer>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod view_spec_tests {
    use super::*;

    // --- ViewKind serde round-trip ---

    #[test]
    fn view_kind_known_variant_round_trips() {
        for vk in [
            ViewKind::CallGraph,
            ViewKind::VerticalSlice,
            ViewKind::SourceView,
            ViewKind::QualityHotspots,
        ] {
            let json = serde_json::to_string(&vk).expect("serialize");
            let back: ViewKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(vk, back, "ViewKind::{vk:?} must round-trip");
        }
    }

    #[test]
    fn view_kind_snake_case_on_wire() {
        let vk = ViewKind::CallGraph;
        let json = serde_json::to_string(&vk).expect("serialize");
        assert_eq!(json, "\"call_graph\"", "CallGraph serialises as snake_case");
    }

    #[test]
    fn view_kind_unknown_string_deserialises_to_custom() {
        let json = r#""future_view""#;
        let back: ViewKind = serde_json::from_str(json).expect("deserialize");
        // Unknown variant tag deserializes to Custom with original string preserved
        assert_eq!(back, ViewKind::Custom("future_view".to_string()));
    }

    #[test]
    fn view_kind_custom_round_trips() {
        let vk = ViewKind::Custom("custom".to_string());
        let json = serde_json::to_string(&vk).expect("serialize");
        assert_eq!(json, "\"custom\"", "Custom serialises as original string");
        let back: ViewKind = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(vk, back);
    }

    // --- Unknown-string preservation tests (C1 fix) ---

    #[test]
    fn view_kind_unknown_round_trip_preserves_string() {
        // Deserialize an unknown wire tag
        let json = r#""future_view_kind""#;
        let back: ViewKind = serde_json::from_str(json).expect("deserialize");
        // Re-serialize and verify the original string is preserved
        let reserialized = serde_json::to_string(&back).expect("serialize");
        assert_eq!(reserialized, json, "unknown tag must round-trip as original string");
    }

    #[test]
    fn view_kind_unknown_with_underscores_round_trip() {
        let json = r#""custom_view_v2_alpha""#;
        let back: ViewKind = serde_json::from_str(json).expect("deserialize");
        let reserialized = serde_json::to_string(&back).expect("serialize");
        assert_eq!(reserialized, json);
    }

    #[test]
    fn renderer_kind_unknown_round_trip_preserves_string() {
        let json = r#""future_renderer_v3""#;
        let back: RendererKind = serde_json::from_str(json).expect("deserialize");
        let reserialized = serde_json::to_string(&back).expect("serialize");
        assert_eq!(reserialized, json, "unknown renderer tag must round-trip as original string");
    }

    #[test]
    fn hierarchy_kind_unknown_round_trip_preserves_string() {
        let json = r#""experimental_hierarchy_kind""#;
        let back: HierarchyKind = serde_json::from_str(json).expect("deserialize");
        let reserialized = serde_json::to_string(&back).expect("serialize");
        assert_eq!(reserialized, json, "unknown hierarchy tag must round-trip as original string");
    }

    // --- RendererKind serde round-trip ---

    #[test]
    fn renderer_kind_known_variant_round_trips() {
        for rk in [
            RendererKind::Graph,
            RendererKind::Table,
            RendererKind::Code,
            RendererKind::Json,
            RendererKind::Composite,
        ] {
            let json = serde_json::to_string(&rk).expect("serialize");
            let back: RendererKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(rk, back, "RendererKind::{rk:?} must round-trip");
        }
    }

    #[test]
    fn renderer_kind_json_round_trip() {
        let rk = RendererKind::Json;
        let json = serde_json::to_string(&rk).expect("serialize");
        let back: RendererKind = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(rk, back);
    }

    #[test]
    fn renderer_kind_unknown_string_deserialises_to_custom() {
        let json = r#""future_renderer""#;
        let back: RendererKind = serde_json::from_str(json).expect("deserialize");
        assert_eq!(back, RendererKind::Custom("future_renderer".to_string()));
    }

    // --- HierarchyKind serde round-trip ---

    #[test]
    fn hierarchy_kind_known_variant_round_trips() {
        for hk in [
            HierarchyKind::FileTree,
            HierarchyKind::ModuleTree,
            HierarchyKind::CallHierarchy,
            HierarchyKind::C4Hierarchy,
        ] {
            let json = serde_json::to_string(&hk).expect("serialize");
            let back: HierarchyKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(hk, back, "HierarchyKind::{hk:?} must round-trip");
        }
    }

    #[test]
    fn hierarchy_kind_unknown_string_deserialises_to_custom() {
        let json = r#""experimental_x""#;
        let back: HierarchyKind = serde_json::from_str(json).expect("deserialize");
        assert_eq!(back, HierarchyKind::Custom("experimental_x".to_string()));
    }

    // --- DataSource serde ---

    #[test]
    fn data_source_moldql_round_trips() {
        let ds = DataSource::Moldql {
            query: "symbols where fan_out > 5".to_string(),
        };
        let json = serde_json::to_string(&ds).expect("serialize");
        let back: DataSource = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(ds, back);
    }

    #[test]
    fn data_source_unknown_kind_is_permissive() {
        // Simulates a future data source that we don't know yet.
        // Unknown `kind` values are caught by the `Other` unit variant.
        let json = r#"{"kind": "graphql", "endpoint": "http://example.com/graphql"}"#;
        let back: DataSource = serde_json::from_str(json).expect("deserialize");
        assert_eq!(back, DataSource::Other);
    }

    // --- Transform serde ---

    #[test]
    fn transform_jsonata_round_trips() {
        let t = Transform::Jsonata {
            expression: "$.data".to_string(),
        };
        let json = serde_json::to_string(&t).expect("serialize");
        let back: Transform = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(t, back);
    }

    #[test]
    fn transform_none_round_trips() {
        // Transform::None represents explicit "no transform" choice (Phase 4).
        let t = Transform::None;
        let json = serde_json::to_string(&t).expect("serialize");
        assert_eq!(json, r#"{"kind":"none"}"#);
        let back: Transform = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(t, back);
    }

    #[test]
    fn transform_other_catches_unknown_variant() {
        // Unknown transform kinds fall through to Other (forward compat).
        let json = r#"{"kind":"future_transform","value":123}"#;
        let back: Transform = serde_json::from_str(json).expect("deserialize");
        assert_eq!(back, Transform::Other);
    }

    // --- ViewSpec serde round-trip ---

    #[test]
    fn view_spec_minimal_round_trip() {
        let vs = ViewSpec {
            id: "a1b2c3d4-e5f6-4789-a123-456789abcdef".to_string(),
            title: "Hot Symbols".to_string(),
            applies_to: InspectableObjectType::Symbol,
            view_kind: ViewKind::QualityHotspots,
            data_source: DataSource::Moldql {
                query: "symbols where fan_out > 5".to_string(),
            },
            transform: None,
            renderer_kind: RendererKind::Table,
            props: serde_json::Value::Object(serde_json::Map::new()),
            created_at: "2026-06-12T00:00:00Z".to_string(),
            updated_at: "2026-06-12T00:00:00Z".to_string(),
            owner: "test-user".to_string(),
        };
        let json = serde_json::to_string(&vs).expect("serialize");
        let back: ViewSpec = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(vs, back);
        // Verify wire format
        let v: serde_json::Value = serde_json::from_str(&json).expect("parse");
        assert_eq!(v["view_kind"], "quality_hotspots");
        assert_eq!(v["renderer_kind"], "table");
    }

    // --- ViewSpec validation ---

    #[test]
    fn validate_rejects_empty_title() {
        let vs = ViewSpec {
            id: "a1b2c3d4-e5f6-4789-a123-456789abcdef".to_string(),
            title: "".to_string(),
            applies_to: InspectableObjectType::Symbol,
            view_kind: ViewKind::CallGraph,
            data_source: DataSource::Moldql {
                query: "symbols".to_string(),
            },
            transform: None,
            renderer_kind: RendererKind::Graph,
            props: serde_json::Value::Null,
            created_at: "2026-06-12T00:00:00Z".to_string(),
            updated_at: "2026-06-12T00:00:00Z".to_string(),
            owner: "test-user".to_string(),
        };
        let err = vs.validate().expect_err("validate must fail");
        assert_eq!(err, ViewSpecError::EmptyTitle);
    }

    #[test]
    fn validate_rejects_title_too_long() {
        let vs = ViewSpec {
            id: "a1b2c3d4-e5f6-4789-a123-456789abcdef".to_string(),
            title: "a".repeat(201),
            applies_to: InspectableObjectType::Symbol,
            view_kind: ViewKind::CallGraph,
            data_source: DataSource::Moldql {
                query: "symbols".to_string(),
            },
            transform: None,
            renderer_kind: RendererKind::Graph,
            props: serde_json::Value::Null,
            created_at: "2026-06-12T00:00:00Z".to_string(),
            updated_at: "2026-06-12T00:00:00Z".to_string(),
            owner: "test-user".to_string(),
        };
        let err = vs.validate().expect_err("validate must fail");
        assert_eq!(err, ViewSpecError::TitleTooLong);
    }

    #[test]
    fn validate_rejects_empty_query() {
        let vs = ViewSpec {
            id: "a1b2c3d4-e5f6-4789-a123-456789abcdef".to_string(),
            title: "Valid".to_string(),
            applies_to: InspectableObjectType::Symbol,
            view_kind: ViewKind::CallGraph,
            data_source: DataSource::Moldql {
                query: "   ".to_string(),
            },
            transform: None,
            renderer_kind: RendererKind::Graph,
            props: serde_json::Value::Null,
            created_at: "2026-06-12T00:00:00Z".to_string(),
            updated_at: "2026-06-12T00:00:00Z".to_string(),
            owner: "test-user".to_string(),
        };
        let err = vs.validate().expect_err("validate must fail");
        assert_eq!(err, ViewSpecError::EmptyQuery);
    }

    #[test]
    fn validate_rejects_invalid_uuid() {
        let vs = ViewSpec {
            id: "not-a-uuid".to_string(),
            title: "Valid".to_string(),
            applies_to: InspectableObjectType::Symbol,
            view_kind: ViewKind::CallGraph,
            data_source: DataSource::Moldql {
                query: "symbols".to_string(),
            },
            transform: None,
            renderer_kind: RendererKind::Graph,
            props: serde_json::Value::Null,
            created_at: "2026-06-12T00:00:00Z".to_string(),
            updated_at: "2026-06-12T00:00:00Z".to_string(),
            owner: "test-user".to_string(),
        };
        let err = vs.validate().expect_err("validate must fail");
        assert_eq!(err, ViewSpecError::InvalidUuid);
    }

    #[test]
    fn validate_accepts_valid_view_spec() {
        let vs = ViewSpec {
            id: "a1b2c3d4-e5f6-4789-a123-456789abcdef".to_string(),
            title: "Hot Symbols".to_string(),
            applies_to: InspectableObjectType::Symbol,
            view_kind: ViewKind::QualityHotspots,
            data_source: DataSource::Moldql {
                query: "symbols where fan_out > 5".to_string(),
            },
            transform: None,
            renderer_kind: RendererKind::Table,
            props: serde_json::json!({"max_nodes": 50}),
            created_at: "2026-06-12T00:00:00Z".to_string(),
            updated_at: "2026-06-12T00:00:00Z".to_string(),
            owner: "test-user".to_string(),
        };
        vs.validate().expect("valid ViewSpec must pass");
    }

    // --- ViewSpecError Display ---

    #[test]
    fn view_spec_error_display() {
        assert_eq!(ViewSpecError::EmptyTitle.to_string(), "title must not be empty");
        assert_eq!(
            ViewSpecError::TitleTooLong.to_string(),
            "title must not exceed 200 characters"
        );
        assert_eq!(
            ViewSpecError::UnknownAppliesTo.to_string(),
            "applies_to is not a valid object type"
        );
        assert_eq!(
            ViewSpecError::EmptyQuery.to_string(),
            "data_source query must not be empty"
        );
        assert_eq!(ViewSpecError::InvalidUuid.to_string(), "id must be a valid UUID");
    }

    // --- ViewSpecError -> ExplorerError conversion ---

    #[test]
    fn view_spec_error_converts_to_explorer_error() {
        let err: crate::error::ExplorerError = ViewSpecError::EmptyTitle.into();
        assert!(matches!(err, crate::error::ExplorerError::InvalidInput(_)));
    }
}

#[cfg(test)]
mod exploration_session_tests {
    use super::*;

    #[test]
    fn exploration_session_serde_roundtrip() {
        let session = ExplorationSession {
            id: "session:1".into(),
            workspace_id: "ws1".into(),
            events: vec![ExplorationEvent {
                object_id: "symbol:a".into(),
                view_id: Some("overview".into()),
                query: None,
                ts: "2026-06-15T00:00:00Z".into(),
            }],
            navigation_mode: "pane-stack".into(),
            created_at: "2026-06-15T00:00:00Z".into(),
        };
        let json = serde_json::to_string(&session).unwrap();
        let deser: ExplorationSession = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.id, session.id);
        assert_eq!(deser.navigation_mode, "pane-stack");
        assert_eq!(deser.events.len(), 1);
        assert_eq!(deser.events[0].object_id, "symbol:a");
    }

    #[test]
    fn exploration_path_old_format_no_navigation_mode() {
        // Old JSON from before ADR-016: no navigation_mode field.
        let json = r#"{
            "id": "exploration:1",
            "workspace_id": "ws1",
            "columns": [{"object_id": "symbol:a", "active_view": "overview"}],
            "lens": null,
            "created_at": "2026-06-15T00:00:00Z"
        }"#;
        let path: ExplorationPath = serde_json::from_str(json).unwrap();
        assert_eq!(path.navigation_mode, "column"); // default
    }

    #[test]
    fn exploration_session_requires_at_least_one_event() {
        let request = SaveExplorationSessionRequest {
            workspace_id: "ws1".into(),
            events: vec![],
            navigation_mode: "column".into(),
        };
        assert!(request.events.is_empty());
    }

    #[test]
    fn exploration_event_query_is_optional() {
        let json = r#"{"object_id":"a","view_id":null,"query":null,"ts":"2026-06-15T00:00:00Z"}"#;
        let ev: ExplorationEvent = serde_json::from_str(json).unwrap();
        assert_eq!(ev.object_id, "a");
        assert!(ev.query.is_none());
    }
}
