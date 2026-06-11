//! Domain port for resolving symbols and their call graph relations.
//!
//! Implementations adapt a concrete `CallGraph` (or a database store) into
//! a stable, explorer-focused interface. The service depends on this trait
//! and not on `CallGraph` directly — that keeps view builders pure and
//! trivial to test with mocks.

use cognicode_core::domain::aggregates::SymbolId;
use cognicode_core::domain::value_objects::{DependencyType, Provenance, SymbolKind};

use crate::error::ExplorerResult;

/// A symbol that has been resolved to a known graph node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSymbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub file: String,
    pub line: u32,
    pub signature: Option<String>,
}

/// Compact view of a single incoming or outgoing relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationTarget {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub file: String,
    pub line: u32,
    pub signature: Option<String>,
}

impl From<&ResolvedSymbol> for RelationTarget {
    fn from(s: &ResolvedSymbol) -> Self {
        Self {
            id: s.id.clone(),
            name: s.name.clone(),
            kind: s.kind,
            file: s.file.clone(),
            line: s.line,
            signature: s.signature.clone(),
        }
    }
}

impl From<ResolvedSymbol> for RelationTarget {
    fn from(s: ResolvedSymbol) -> Self {
        Self::from(&s)
    }
}

/// Aggregate counts of a loaded call graph.
///
/// Returned by [`SymbolRepository::graph_stats`] so the workspace summary
/// can be populated with real values once a graph is indexed. The two
/// counts are always retrieved together so they remain coherent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GraphStats {
    pub symbol_count: usize,
    pub relation_count: usize,
}

/// Read-only port for looking up symbols and their direct call relations.
///
/// The trait surface is intentionally metadata-free: `callees` and `callers`
/// return plain `RelationTarget` values. Consumers that need edge trust
/// information (provenance, confidence) should depend on the
/// [`MetadataAwareRepository`] sub-trait instead. This keeps the base
/// port usable for view builders and mock implementations that do not
/// need to surface the metadata introduced in Phase 1.
pub trait SymbolRepository: Send + Sync {
    /// Resolve a fully-qualified `SymbolId`. Returns `None` when the id is
    /// not present in the underlying graph.
    fn resolve(&self, id: &SymbolId) -> ExplorerResult<Option<ResolvedSymbol>>;

    /// Return the direct callers (incoming edges) of `id`.
    fn callers(&self, id: &SymbolId) -> Vec<RelationTarget>;

    /// Return the direct callees (outgoing edges) of `id`.
    fn callees(&self, id: &SymbolId) -> Vec<RelationTarget>;

    /// Number of direct incoming edges.
    fn fan_in(&self, id: &SymbolId) -> usize;

    /// Number of direct outgoing edges.
    fn fan_out(&self, id: &SymbolId) -> usize;

    /// Look up all symbols whose base name matches `name` (case-insensitive,
    /// exact match). Returns an empty `Vec` when nothing matches — never an
    /// error. Used by the spotter search pipeline.
    fn find_symbols_by_name(&self, name: &str) -> ExplorerResult<Vec<ResolvedSymbol>>;

    /// Return every symbol whose `location().file()` equals `file` exactly
    /// (no path normalisation, no prefix match). Returns an empty `Vec`
    /// when nothing matches — never an error. Used by the file scope of
    /// the explorer to render the "symbols in this file" view.
    fn find_symbols_by_file(&self, file: &str) -> ExplorerResult<Vec<ResolvedSymbol>>;

    /// Return a sorted, deduplicated list of the parent directories of
    /// every indexed symbol's file. Mirrors [`CallGraph::modules`] —
    /// scope identities in Phase 2 are derived from this set.
    fn module_list(&self) -> Vec<String>;

    /// Return every indexed symbol in the graph. Used by scope inspection
    /// to compute the scope's member files and member symbols. The result
    /// is unordered; callers that need a stable order must sort. Linear in
    /// the number of symbols — acceptable for the MVP (sub-10k symbols).
    fn all_symbols(&self) -> ExplorerResult<Vec<ResolvedSymbol>>;

    /// Return the current size of the underlying graph. Used by the
    /// workspace summary to report real symbol/edge counts.
    fn graph_stats(&self) -> GraphStats;

    /// Downcast hook so consumers holding a `&dyn SymbolRepository` can
    /// reach the [`MetadataAwareRepository`] surface without an
    /// `Any`-based downcast at every call site.
    ///
    /// Default implementation returns `None` — only adapters that
    /// actually carry edge trust metadata (currently
    /// [`crate::adapters::CallGraphRepository`]) override it. Mock
    /// repositories inherit the `None` default automatically; no
    /// per-mock wiring is required.
    ///
    /// This is the seam the call-graph / scope-dependency view
    /// builders use to populate `TypedRelation::provenance` and
    /// `TypedRelation::confidence`. Adapters that grow metadata
    /// support in the future only need to override this method.
    fn as_metadata_aware(&self) -> Option<&dyn MetadataAwareRepository> {
        None
    }
}

/// A relation target enriched with edge metadata (provenance, confidence).
///
/// Returned by [`MetadataAwareRepository::callees_with_metadata`] and
/// [`MetadataAwareRepository::dependencies_with_metadata`]. Carries the
/// full [`RelationTarget`] (no N+1 lookups required for display data) plus
/// the `(Provenance, confidence)` tuple assigned by
/// `ConfidenceRules` at edge creation time.
#[derive(Debug, Clone, PartialEq)]
pub struct RelationTargetWithMetadata {
    pub target: RelationTarget,
    pub dependency_type: DependencyType,
    pub provenance: Provenance,
    pub confidence: f64,
}

/// A full graph edge enriched with edge metadata (provenance, confidence).
///
/// Returned by [`MetadataAwareRepository::edges_with_metadata`]. Carries
/// the source [`SymbolId`], the resolved target [`RelationTarget`], the
/// edge [`DependencyType`], and the `(Provenance, confidence)` tuple.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeWithMetadata {
    pub source: SymbolId,
    pub target: RelationTarget,
    pub dependency_type: DependencyType,
    pub provenance: Provenance,
    pub confidence: f64,
}

/// Opt-in sub-trait for repository consumers that need edge trust
/// information (provenance, confidence).
///
/// Implemented on top of [`SymbolRepository`]: a `dyn SymbolRepository`
/// reference does not expose these methods. Downcast (or an explicit
/// `as_metadata_aware` helper on the concrete adapter) is required to
/// reach the metadata-aware surface. This is intentional — the base
/// trait serves a broader consumer set that has no need for trust
/// metadata.
pub trait MetadataAwareRepository: SymbolRepository {
    /// Return the direct callees of `id` along with their `(Provenance,
    /// confidence)` metadata.
    fn callees_with_metadata(&self, id: &SymbolId) -> Vec<RelationTargetWithMetadata>;

    /// Return the outgoing dependencies of `id` along with their
    /// `(Provenance, confidence)` metadata. Synonym for
    /// [`Self::callees_with_metadata`] kept for parity with the
    /// underlying `CallGraph::dependencies_with_metadata` method.
    fn dependencies_with_metadata(&self, id: &SymbolId) -> Vec<RelationTargetWithMetadata>;

    /// Return every edge in the underlying graph with full metadata.
    fn edges_with_metadata(&self) -> Vec<EdgeWithMetadata>;
}
