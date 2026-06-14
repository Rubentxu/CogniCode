//! Graph navigation port — ISP-segregated from SymbolRepository identity methods.
//!
//! [`GraphQueryPort`] provides graph traversal and navigation queries separated
//! from the identity-resolution methods of [`SymbolRepository`].
//! This separation follows ADR-010 Phase 4: identity methods stay on
//! [`SymbolRepository`](cognicode_explorer::ports::SymbolRepository), while
//! navigation/traversal methods live here.
//!
//! [`CallGraphRepository`](cognicode_explorer::adapters::CallGraphRepository)
//! implements this trait on the same `Arc<CallGraph>` backing store.

use crate::domain::aggregates::{CallEntry, SymbolId};
use crate::domain::value_objects::{DependencyType, Provenance, SymbolKind};

/// Compact view of a single incoming or outgoing relation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationTarget {
    /// The symbol id.
    pub id: SymbolId,
    /// The symbol name.
    pub name: String,
    /// The symbol kind.
    pub kind: SymbolKind,
    /// The file where the symbol is defined.
    pub file: String,
    /// The line number.
    pub line: u32,
    /// The signature, if available.
    pub signature: Option<String>,
}

/// A relation target enriched with edge metadata (provenance, confidence).
///
/// Returned by [`GraphQueryPort::callees_with_metadata`] and
/// [`GraphQueryPort::dependencies_with_metadata`]. Carries the
/// full [`RelationTarget`] (no N+1 lookups required for display data) plus
/// the `(Provenance, confidence)` tuple assigned by
/// `ConfidenceRules` at edge creation time.
#[derive(Debug, Clone, PartialEq)]
pub struct RelationTargetWithMetadata {
    /// The resolved relation target.
    pub target: RelationTarget,
    /// The kind of dependency (Calls, Imports, etc.).
    pub dependency_type: DependencyType,
    /// How this call relationship was established.
    pub provenance: Provenance,
    /// Confidence score in `[0.0, 1.0]`.
    pub confidence: f64,
}

/// A full graph edge enriched with edge metadata (provenance, confidence).
///
/// Returned by [`GraphQueryPort::edges_with_metadata`]. Carries
/// the source [`SymbolId`], the resolved target [`RelationTarget`], the
/// edge [`DependencyType`], and the `(Provenance, confidence)` tuple.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeWithMetadata {
    /// The source symbol id.
    pub source: SymbolId,
    /// The resolved relation target.
    pub target: RelationTarget,
    /// The kind of dependency.
    pub dependency_type: DependencyType,
    /// How this call relationship was established.
    pub provenance: Provenance,
    /// Confidence score in `[0.0, 1.0]`.
    pub confidence: f64,
}

/// Caller with metadata — caller symbol ID plus edge provenance and confidence.
#[derive(Debug, Clone, PartialEq)]
pub struct CallerWithMetadata {
    /// The symbol that calls the target.
    pub caller_id: SymbolId,
    /// How this call relationship was established.
    pub provenance: Provenance,
    /// Confidence score in `[0.0, 1.0]`.
    pub confidence: f64,
}

/// Callee with metadata — callee symbol ID, dependency type, provenance, confidence.
#[derive(Debug, Clone, PartialEq)]
pub struct CalleeWithMetadata {
    /// The symbol being called.
    pub callee_id: SymbolId,
    /// The kind of dependency (Calls, Imports, etc.).
    pub dependency_type: DependencyType,
    /// How this call relationship was established.
    pub provenance: Provenance,
    /// Confidence score in `[0.0, 1.0]`.
    pub confidence: f64,
}

/// Graph navigation and traversal queries.
///
/// Separated from [`SymbolRepository`](cognicode_explorer::ports::SymbolRepository)
/// per ADR-010 Phase 4. CallGraphRepository implements BOTH
/// SymbolRepository (identity) AND GraphQueryPort (navigation) on the
/// same `Arc<CallGraph>` backing store.
///
/// This trait unifies both navigation methods (callers/callees with metadata)
/// and the basic identity-resolved navigation methods (callers/callees returning
/// `RelationTarget`). The `MetadataAwareRepository` sub-trait has been removed;
/// all metadata-aware and basic navigation now lives here.
pub trait GraphQueryPort: Send + Sync {
    /// Return the direct callers (incoming edges) of `id` with resolved symbol
    /// identity. Returns `Vec<RelationTarget>` — callers that cannot be resolved
    /// are silently omitted.
    fn callers(&self, id: &SymbolId) -> Vec<RelationTarget>;

    /// Return the direct callees (outgoing edges) of `id` with resolved symbol
    /// identity. Returns `Vec<RelationTarget>` — callees that cannot be resolved
    /// are silently omitted.
    fn callees(&self, id: &SymbolId) -> Vec<RelationTarget>;

    /// Number of direct incoming edges.
    fn fan_in(&self, id: &SymbolId) -> usize;

    /// Number of direct outgoing edges.
    fn fan_out(&self, id: &SymbolId) -> usize;

    /// Callers with provenance and confidence metadata.
    ///
    /// Unlike `Self::callers` which returns resolved `RelationTarget`,
    /// this returns raw caller IDs with edge metadata.
    fn callers_with_metadata(&self, id: &SymbolId) -> Vec<CallerWithMetadata>;

    /// Callees with provenance and confidence metadata.
    ///
    /// Unlike `Self::callees` which returns resolved `RelationTarget`,
    /// this returns raw callee IDs with edge metadata.
    fn callees_with_metadata(&self, id: &SymbolId) -> Vec<CalleeWithMetadata>;

    /// Return the outgoing dependencies of `id` along with their
    /// `(Provenance, confidence)` metadata. Synonym for
    /// [`Self::callees_with_metadata`] kept for parity with the
    /// underlying `CallGraph::dependencies_with_metadata` method.
    fn dependencies_with_metadata(&self, id: &SymbolId) -> Vec<RelationTargetWithMetadata>;

    /// BFS traversal of callees up to `max_depth`.
    fn traverse_callees(&self, id: &SymbolId, max_depth: u8) -> Vec<CallEntry>;

    /// BFS traversal of callers up to `max_depth`.
    fn traverse_callers(&self, id: &SymbolId, max_depth: u8) -> Vec<CallEntry>;
}
