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
