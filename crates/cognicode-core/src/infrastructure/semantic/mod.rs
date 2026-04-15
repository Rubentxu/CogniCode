//! Semantic Analysis Infrastructure
//!
//! This module provides semantic analysis features including:
//! - Hierarchical outline generation
//! - Symbol code retrieval with caching
//! - Semantic search with filtering

pub mod outline;
pub mod semantic_search;
pub mod symbol_code;

pub use outline::{build_outline, OutlineBuilder, OutlineNode};
pub use semantic_search::{
    MatchType, SearchIndex, SearchQuery, SearchResult, SearchSymbolKind, SemanticSearchService,
};
pub use symbol_code::{CachedSymbolCode, SymbolCodeCache, SymbolCodeKey, SymbolCodeService};
