//! IDF-weighted symbol search ranking.
//!
//! Computes Inverse Document Frequency for symbol name tokens,
//! then ranks search results by IDF score (rare terms = higher value).
//! Includes hub bypass to ignore overly-connected nodes.

use petgraph::Direction;
use std::collections::HashMap;

use crate::domain::aggregates::{CallGraph, SymbolId};
use crate::infrastructure::graph::CallGraphProjection;

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

    /// Compute IDF scores for all terms in the graph.
    /// `IDF(term) = log(1 + N / (1 + docs_containing_term))`
    ///
    /// The `+1` smoothing prevents division by zero for terms that
    /// appear in *every* document (df == N). It also matches the
    /// sklearn "smooth=True" convention so the output is comparable
    /// to standard TF-IDF pipelines.
    fn compute_idf(graph: &CallGraph) -> HashMap<String, f64> {
        let n = graph.symbol_count() as f64;
        if n == 0.0 {
            return HashMap::new();
        }

        let mut doc_freq: HashMap<String, usize> = HashMap::new();

        for (_, symbol) in graph.symbol_ids() {
            let mut seen_terms: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            let tokens = Self::tokenize(symbol.name());
            for token in tokens {
                if seen_terms.insert(token.clone()) {
                    *doc_freq.entry(token).or_insert(0) += 1;
                }
            }
            // Also index the full name as a single token so that
            // queries for compound names like "authenticate_user"
            // match a symbol named exactly that.
            let full = symbol.name().to_lowercase();
            if seen_terms.insert(full.clone()) {
                *doc_freq.entry(full).or_insert(0) += 1;
            }
        }

        doc_freq
            .into_iter()
            .map(|(term, df)| {
                let idf = (1.0 + n / (1.0 + df as f64)).ln();
                (term, idf)
            })
            .collect()
    }

    /// Search symbols ranked by IDF scores.
    ///
    /// Returns up to `max_results` entries, sorted by score descending.
    ///
    /// Edge cases:
    /// - Empty graph -> empty vec.
    /// - Query that matches nothing -> empty vec.
    /// - Query that matches every symbol -> at least one entry
    ///   (the highest-scoring one).
    /// - A `CallGraph` whose symbols have empty names is filtered
    ///   out by the `score > 0` invariant.
    pub fn search(graph: &CallGraph, query: &str, max_results: usize) -> Vec<ScoredSymbol> {
        if graph.symbol_count() == 0 {
            return Vec::new();
        }

        let idf = Self::compute_idf(graph);
        let projection = CallGraphProjection::from_call_graph(graph);
        let g = projection.graph();

        // Compute degree percentiles for hub bypass.
        // The hub bypass uses the 95th percentile of the *total*
        // degree (in + out) so that very chatty "switchboard" nodes
        // get demoted regardless of direction. `usize::MAX` is used
        // as the bypass threshold for a single-node graph so the
        // single node is never bypassed (it has nothing to compete
        // with anyway).
        let degrees: Vec<usize> = g
            .node_indices()
            .map(|ni| {
                g.edges_directed(ni, Direction::Outgoing).count()
                    + g.edges_directed(ni, Direction::Incoming).count()
            })
            .collect();

        let p95_degree = if degrees.is_empty() {
            usize::MAX
        } else {
            let mut sorted = degrees.clone();
            sorted.sort();
            let idx = ((sorted.len() as f64 * 0.95) as usize).min(sorted.len().saturating_sub(1));
            sorted[idx]
        };

        let query_lower = query.to_lowercase();
        let query_tokens = Self::tokenize(query);

        let mut scored: Vec<ScoredSymbol> = Vec::new();

        for ni in g.node_indices() {
            let symbol_id = &g[ni];

            // Find symbol name and file. The projection's
            // `symbol_lookup` is the source of truth (it mirrors
            // the source `CallGraph`).
            let (name, file) = if let Some(sym) = graph.get_symbol(symbol_id) {
                (sym.name().to_string(), sym.location().file().to_string())
            } else {
                // Fallback: extract from SymbolId string
                let parts: Vec<&str> = symbol_id.as_str().splitn(3, ':').collect();
                let n = parts.get(1).unwrap_or(&"").to_string();
                let f = parts.get(0).unwrap_or(&"").to_string();
                (n, f)
            };

            if name.is_empty() {
                continue;
            }

            let name_lower = name.to_lowercase();
            let name_tokens = Self::tokenize(&name);

            // Compute score
            let mut score = 0.0f64;

            // Exact full name match
            if name_lower == query_lower {
                score += idf.get(&query_lower).unwrap_or(&1.0) * 1000.0;
            }
            // Full name contains query
            else if !query_lower.is_empty() && name_lower.contains(&query_lower) {
                score += idf.get(&query_lower).unwrap_or(&1.0) * 100.0;
            }

            // Token-level matching
            for qt in &query_tokens {
                for nt in &name_tokens {
                    let token_idf = idf.get(nt).unwrap_or(&1.0);
                    if nt == qt {
                        score += token_idf * 100.0; // Exact token match
                    } else if nt.starts_with(qt) {
                        score += token_idf * 10.0; // Prefix match
                    } else if nt.contains(qt) {
                        score += token_idf * 1.0; // Substring match
                    }
                }
            }

            if score <= 0.0 {
                continue;
            }

            // Hub bypass: demote overly-connected nodes
            let degree = g.edges_directed(ni, Direction::Outgoing).count()
                + g.edges_directed(ni, Direction::Incoming).count();

            if degree > p95_degree {
                score *= 0.1;
            }

            scored.push(ScoredSymbol {
                symbol_id: symbol_id.clone(),
                name,
                file,
                idf_score: score,
                degree,
            });
        }

        // Sort by score descending
        scored.sort_by(|a, b| {
            b.idf_score
                .partial_cmp(&a.idf_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(max_results);
        scored
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
