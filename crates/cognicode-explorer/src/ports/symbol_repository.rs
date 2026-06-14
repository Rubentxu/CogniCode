//! Domain port for resolving symbols and their call graph relations.
//!
//! Implementations adapt a concrete `CallGraph` (or a database store) into
//! a stable, explorer-focused interface. The service depends on this trait
//! and not on `CallGraph` directly — that keeps view builders pure and
//! trivial to test with mocks.

use cognicode_core::domain::aggregates::SymbolId;
use cognicode_core::domain::value_objects::SymbolKind;

use crate::error::ExplorerResult;

// Re-export RelationTarget from graph_query_port for backwards compatibility.
pub use cognicode_core::domain::traits::graph_query_port::RelationTarget;

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

impl From<ResolvedSymbol> for RelationTarget {
    fn from(sym: ResolvedSymbol) -> Self {
        RelationTarget {
            id: sym.id,
            name: sym.name,
            kind: sym.kind,
            file: sym.file,
            line: sym.line,
            signature: sym.signature,
        }
    }
}

impl From<&ResolvedSymbol> for RelationTarget {
    fn from(sym: &ResolvedSymbol) -> Self {
        RelationTarget {
            id: sym.id.clone(),
            name: sym.name.clone(),
            kind: sym.kind.clone(),
            file: sym.file.clone(),
            line: sym.line,
            signature: sym.signature.clone(),
        }
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

/// Read-only port for looking up symbols and their graph relations.
///
/// ## Navigation methods moved to `GraphQueryPort`
///
/// The `callers`, `callees`, `fan_in`, and `fan_out` methods have been
/// moved to [`GraphQueryPort`](cognicode_core::domain::traits::GraphQueryPort).
/// The identity methods below remain here as they are the primary lookup
/// interface for symbol resolution.
pub trait SymbolRepository: Send + Sync {
    /// Resolve a fully-qualified `SymbolId`. Returns `None` when the id is
    /// not present in the underlying graph.
    fn resolve(&self, id: &SymbolId) -> ExplorerResult<Option<ResolvedSymbol>>;

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
}
