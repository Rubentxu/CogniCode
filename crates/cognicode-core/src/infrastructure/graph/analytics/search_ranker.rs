//! IDF-weighted symbol search ranking.
//!
//! Computes Inverse Document Frequency for symbol name tokens,
//! then ranks search results by IDF score (rare terms = higher value).
//! Includes hub bypass to ignore overly-connected nodes.

use crate::domain::aggregates::{CallGraph, SymbolId};

use super::ranking_stats::RankingStats;

/// A scored search result.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScoredSymbol {
    pub symbol_id: SymbolId,
    pub name: String,
    pub file: String,
    pub idf_score: f64,
    pub degree: usize,
}

/// IDF-weighted search ranker.
///
/// The ranker treats each symbol name as a "document" and tokenises it
/// on snake_case, camelCase, and `::`/`.`/`-` boundaries. Tokens that
/// appear in many names (e.g. `get`, `new`, `function`) receive a low
/// IDF score; tokens that appear in only a handful of names
/// (e.g. `authenticate`, `merkle`) receive a high IDF score.
///
/// The final ranking combines:
/// - **Exact full-name match** (×1000 IDF)
/// - **Full-name contains query** (×100 IDF)
/// - **Exact token match** (×100 IDF)
/// - **Prefix token match** (×10 IDF)
/// - **Substring token match** (×1 IDF)
///
/// Nodes whose degree exceeds the 95th percentile are demoted by
/// ×0.1 to bypass "hub" symbols (e.g. very generic `process` or
/// `handle` nodes that match every query). This is the "hub bypass"
/// pattern borrowed from HITS / SALSA.
pub struct SearchRanker;

impl SearchRanker {
    /// Tokenize a symbol name into searchable terms.
    /// Splits on '_', '::', '.', '-' and camelCase boundaries.
    ///
    /// Tokens are lowercased; the original casing is dropped because
    /// search is case-insensitive. An empty string yields an empty
    /// token list (no panic on degenerate input).
    pub fn tokenize(name: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current = String::new();

        for ch in name.chars() {
            if ch == '_' || ch == ':' || ch == '.' || ch == '-' {
                if !current.is_empty() {
                    tokens.push(current.to_lowercase());
                    current.clear();
                }
            } else if ch.is_uppercase() && !current.is_empty() {
                // camelCase / PascalCase boundary: emit the buffer
                // (already lowercased up to this point) and start a
                // new buffer with the uppercase char.
                tokens.push(current.to_lowercase());
                current.clear();
                current.push(ch);
            } else {
                current.push(ch);
            }
        }
        if !current.is_empty() {
            tokens.push(current.to_lowercase());
        }
        tokens
    }

    /// Search symbols ranked by IDF scores.
    ///
    /// Returns up to `max_results` entries, sorted by score descending;
    /// equal scores come out in symbol id order.
    ///
    /// This reads the whole graph on every call. A caller that answers more
    /// than one query over the same snapshot builds a [`RankingStats`] once
    /// (or takes it from a
    /// [`RankingStatsCache`](super::ranking_stats::RankingStatsCache)) and
    /// asks it to [`rank`](RankingStats::rank): the result is the same, bit
    /// for bit.
    ///
    /// `IDF(term) = log(1 + N / (1 + docs_containing_term))`; the `+1`
    /// smoothing keeps terms that appear in every document finite and
    /// matches the sklearn `smooth_idf` convention.
    ///
    /// Edge cases:
    /// - Empty graph -> empty vec.
    /// - Query that matches nothing -> empty vec.
    /// - Query that matches every symbol -> at least one entry
    ///   (the highest-scoring one).
    /// - Symbols with empty names never match.
    pub fn search(graph: &CallGraph, query: &str, max_results: usize) -> Vec<ScoredSymbol> {
        RankingStats::build(graph).rank(query, max_results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_snake_case() {
        let tokens = SearchRanker::tokenize("my_function_name");
        assert_eq!(tokens, vec!["my", "function", "name"]);
    }

    #[test]
    fn test_tokenize_camel_case() {
        let tokens = SearchRanker::tokenize("myFunctionName");
        assert_eq!(tokens, vec!["my", "function", "name"]);
    }

    #[test]
    fn test_tokenize_path_separated() {
        let tokens = SearchRanker::tokenize("module::sub::function");
        assert_eq!(tokens, vec!["module", "sub", "function"]);
    }

    #[test]
    fn test_tokenize_hyphenated() {
        let tokens = SearchRanker::tokenize("kebab-case-name");
        assert_eq!(tokens, vec!["kebab", "case", "name"]);
    }

    #[test]
    fn test_tokenize_dotted() {
        let tokens = SearchRanker::tokenize("a.b.c");
        assert_eq!(tokens, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_tokenize_empty() {
        let tokens = SearchRanker::tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_empty_graph_returns_empty() {
        let graph = CallGraph::new();
        let results = SearchRanker::search(&graph, "anything", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_exact_match_scores_highest() {
        // Build graph with two symbols
        let mut graph = CallGraph::new();
        use crate::domain::aggregates::Symbol;
        use crate::domain::value_objects::{Location, SymbolKind};
        graph.add_symbol(Symbol::new(
            "authenticate",
            SymbolKind::Function,
            Location::new("auth.rs", 1, 1),
        ));
        graph.add_symbol(Symbol::new(
            "authorize",
            SymbolKind::Function,
            Location::new("auth.rs", 2, 1),
        ));

        let results = SearchRanker::search(&graph, "authenticate", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].name, "authenticate");
        assert!(results[0].idf_score > 100.0); // Exact match gets high score
    }

    #[test]
    fn test_partial_match_returns_results() {
        let mut graph = CallGraph::new();
        use crate::domain::aggregates::Symbol;
        use crate::domain::value_objects::{Location, SymbolKind};
        graph.add_symbol(Symbol::new(
            "get_user",
            SymbolKind::Function,
            Location::new("api.rs", 1, 1),
        ));
        graph.add_symbol(Symbol::new(
            "get_post",
            SymbolKind::Function,
            Location::new("api.rs", 2, 1),
        ));
        graph.add_symbol(Symbol::new(
            "delete_user",
            SymbolKind::Function,
            Location::new("api.rs", 3, 1),
        ));

        let results = SearchRanker::search(&graph, "user", 10);
        assert!(results.len() >= 2); // get_user and delete_user should match
    }

    #[test]
    fn test_no_match_returns_empty() {
        let mut graph = CallGraph::new();
        use crate::domain::aggregates::Symbol;
        use crate::domain::value_objects::{Location, SymbolKind};
        graph.add_symbol(Symbol::new(
            "foo",
            SymbolKind::Function,
            Location::new("a.rs", 1, 1),
        ));
        let results = SearchRanker::search(&graph, "xyzzy", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_max_results_truncates() {
        let mut graph = CallGraph::new();
        use crate::domain::aggregates::Symbol;
        use crate::domain::value_objects::{Location, SymbolKind};
        for i in 0..10 {
            graph.add_symbol(Symbol::new(
                format!("common_fn_{}", i).as_str(),
                SymbolKind::Function,
                Location::new("a.rs", i, 1),
            ));
        }
        let results = SearchRanker::search(&graph, "common", 3);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_idf_prefers_rare_tokens() {
        // Build a graph where "user" appears in many symbols (low IDF)
        // and "authenticate" appears in only one (high IDF). A search
        // for "authenticate" should rank that one symbol highest.
        let mut graph = CallGraph::new();
        use crate::domain::aggregates::Symbol;
        use crate::domain::value_objects::{Location, SymbolKind};
        graph.add_symbol(Symbol::new(
            "authenticate",
            SymbolKind::Function,
            Location::new("a.rs", 1, 1),
        ));
        for i in 0..5 {
            graph.add_symbol(Symbol::new(
                format!("user_op_{}", i).as_str(),
                SymbolKind::Function,
                Location::new("a.rs", i + 2, 1),
            ));
        }

        let results = SearchRanker::search(&graph, "authenticate", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].name, "authenticate");
    }
}
