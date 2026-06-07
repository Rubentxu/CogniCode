//! Domain port for full-text / fuzzy symbol search.
//!
//! Separated from [`super::symbol_repository::SymbolRepository`] so adapters
//! that only know how to navigate a graph (the in-memory `CallGraph`,
//! future mocked repos) are not forced to implement FTS5 — and so a search
//! backend that does not have a graph (e.g. a pure FTS5 table) can stand on
//! its own. This is an explicit ISP split: "find me a symbol by identity /
//! relation" is one concern, "find me symbols that match this text" is
//! another.

use crate::error::ExplorerResult;

/// One search hit — a single symbol surfaced by some full-text / fuzzy
/// backend.
///
/// `line` is the resolved 1-based source line for the symbol. FTS5-backed
/// adapters do not store the line and emit `0` here; the *service* layer
/// is responsible for resolving lines via the symbol repository and
/// constructing the canonical MVP id `symbol:{file}:{name}:{line}`.
///
/// `mvp_id` is the canonical MVP id; it is empty for unresolved FTS5 hits
/// and filled in by the service after line resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub mvp_id: String,
    pub name: String,
    pub kind: String,
    pub file: String,
    /// 1-based source line. `0` means the backend did not provide a line
    /// (e.g. FTS5) and the service must resolve it from the symbol
    /// repository before exposing the hit to clients.
    pub line: u32,
    /// Score in `[0.0, 1.0]`. Exact matches are expected to score `1.0`;
    /// FTS5 hits should score below `1.0` so a fused result preserves
    /// "exact > fuzzy" ordering.
    pub score: f32,
    /// `"exact"`, `"fts5"`, or any future backend tag. Surfaced to the
    /// Spotter UI so callers can tell why a result landed in the list.
    pub match_type: String,
}

impl SearchHit {
    /// Build a fully-resolved hit from a known `(name, kind, file, line)`
    /// tuple. The service uses this for both exact matches and for FTS5
    /// hits that it has resolved through the symbol repository.
    pub fn resolved(
        name: impl Into<String>,
        kind: impl Into<String>,
        file: impl Into<String>,
        line: u32,
        score: f32,
        match_type: impl Into<String>,
    ) -> Self {
        let name = name.into();
        let file = file.into();
        Self {
            mvp_id: format!("symbol:{file}:{name}:{line}"),
            name,
            kind: kind.into(),
            file,
            line,
            score,
            match_type: match_type.into(),
        }
    }
}

/// Read-only port for full-text / fuzzy symbol search.
pub trait SearchRepository: Send + Sync {
    /// Return up to `limit` hits for `query`. An empty `query` MUST return
    /// an empty `Vec` (no errors). Adapters that cannot service the query
    /// (e.g. DB missing) return an empty `Vec` — graceful degradation over
    /// hard failure.
    fn search(&self, query: &str, limit: usize) -> ExplorerResult<Vec<SearchHit>>;
}
