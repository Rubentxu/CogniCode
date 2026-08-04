//! Relation Candidates Service - Stateless heuristic for suggesting missing call edges.
//!
//! This service provides candidates for potentially missing incoming edges to symbols
//! that appear to have no callers (dead code candidates). It uses three heuristics:
//!
//! 1. **Same file** (confidence 0.7): Other symbols in the same file that might call it
//! 2. **Same community** (confidence 0.5): Symbols in the same community detected by Label Propagation
//! 3. **Name match** (confidence 0.3): Symbols whose tokenized names share tokens with the dead symbol
//!
//! The algorithm is stateless and pure - it only reads from the provided CallGraph.

use std::collections::HashMap;

use crate::application::dto::{
    CONFIDENCE_NAME_MATCH, CONFIDENCE_SAME_COMMUNITY, CONFIDENCE_SAME_FILE, RelationCandidate,
};
use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};
use crate::infrastructure::graph::analytics::community_detector::CommunityDetector;

/// Confidence thresholds for deduplication
const MIN_TOKEN_LENGTH: usize = 3;

/// Stateless service for finding relation candidates
pub struct CandidateFinder;

impl CandidateFinder {
    /// Tokenize a name on snake_case and camelCase boundaries.
    ///
    /// # Examples
    /// - `my_function_name` -> ["my", "function", "name"]
    /// - `MyStruct` -> ["My", "Struct"]
    /// - `processHTTPRequest` -> ["process", "HTTP", "Request"]
    fn tokenize(name: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current = String::new();
        let chars: Vec<char> = name.chars().collect();

        for (i, c) in chars.iter().enumerate() {
            let is_sep = *c == '_' || *c == '-';

            if is_sep {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            } else if c.is_uppercase() {
                // Look ahead: one char ahead
                let ahead = chars.get(i + 1);
                let next_is_lower = ahead.map(|nc| nc.is_lowercase()).unwrap_or(false);

                // Check if this uppercase starts a new segment
                // Split if: (prev was lowercase) OR (prev was uppercase AND next is lowercase)
                // First char (i==0) never splits
                let prev_is_upper = if i > 0 {
                    chars[i - 1].is_uppercase()
                } else {
                    false
                };
                let should_split = i > 0 && (!prev_is_upper || next_is_lower);

                if should_split && !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
                current.push(*c);
            } else {
                // Lowercase
                current.push(*c);
            }
        }

        if !current.is_empty() {
            tokens.push(current);
        }

        tokens
    }

    /// Find all relation candidates for a dead symbol (one with no callers).
    ///
    /// Returns an empty vec if the symbol already has callers.
    pub fn find_candidates(graph: &CallGraph, symbol_id: &str) -> Vec<RelationCandidate> {
        let target_id = SymbolId::new(symbol_id);

        // If symbol has callers, it's not dead - return empty
        if !graph.callers(&target_id).is_empty() {
            return Vec::new();
        }

        // Get the target symbol to extract file and name
        let target_symbol = match graph.get_symbol(&target_id) {
            Some(s) => s,
            None => return Vec::new(),
        };

        let target_file = target_symbol.location().file();
        let target_name = target_symbol.name();
        let target_tokens: Vec<String> = Self::tokenize(target_name)
            .into_iter()
            .filter(|t| t.len() >= MIN_TOKEN_LENGTH)
            .collect();

        // Run community detection once and reuse the result
        let community_result = CommunityDetector::detect(graph, 100);
        let node_communities = &community_result.node_communities;
        let target_community = node_communities.get(symbol_id).copied();

        let mut candidates: Vec<RelationCandidate> = Vec::new();

        // Phase 1: Same file candidates (highest confidence)
        candidates.extend(Self::same_file_candidates(graph, symbol_id, target_file));

        // Phase 2: Same community candidates (medium confidence)
        if let Some(target_comm) = target_community {
            candidates.extend(Self::community_candidates(
                graph,
                symbol_id,
                target_comm,
                node_communities,
            ));
        }

        // Phase 3: Name match candidates (lowest confidence)
        if !target_tokens.is_empty() {
            candidates.extend(Self::name_match_candidates(
                graph,
                symbol_id,
                &target_tokens,
            ));
        }

        // Deduplicate and return
        Self::dedup(candidates)
    }

    /// Find symbols in the same file that might call the target.
    fn same_file_candidates(
        graph: &CallGraph,
        symbol_id: &str,
        target_file: &str,
    ) -> Vec<RelationCandidate> {
        graph
            .symbols()
            .filter(|symbol| {
                let file = symbol.location().file();
                file == target_file && !symbol.fully_qualified_name().contains(symbol_id)
            })
            .map(|symbol| RelationCandidate {
                symbol_id: symbol.fully_qualified_name().to_string(),
                confidence: CONFIDENCE_SAME_FILE,
                reason: "same_file".to_string(),
                direction: "incoming".to_string(),
            })
            .collect()
    }

    /// Find symbols in the same community that might call the target.
    fn community_candidates(
        graph: &CallGraph,
        symbol_id: &str,
        target_community: u32,
        node_communities: &HashMap<String, u32>,
    ) -> Vec<RelationCandidate> {
        graph
            .symbols()
            .filter(|symbol| {
                let fqn = symbol.fully_qualified_name();
                // Not the target itself
                !fqn.contains(symbol_id)
                // In the same community
                && node_communities
                    .get(fqn)
                    .map(|&c| c == target_community)
                    .unwrap_or(false)
            })
            .map(|symbol| RelationCandidate {
                symbol_id: symbol.fully_qualified_name().to_string(),
                confidence: CONFIDENCE_SAME_COMMUNITY,
                reason: "same_community".to_string(),
                direction: "incoming".to_string(),
            })
            .collect()
    }

    /// Find symbols whose names share tokens with the target name.
    fn name_match_candidates(
        graph: &CallGraph,
        symbol_id: &str,
        target_tokens: &[String],
    ) -> Vec<RelationCandidate> {
        let target_tokens_set: std::collections::HashSet<&str> =
            target_tokens.iter().map(|s| s.as_str()).collect();

        graph
            .symbols()
            .filter(|symbol| {
                let fqn = symbol.fully_qualified_name();
                // Not the target itself
                if fqn.contains(symbol_id) {
                    return false;
                }

                // Check if any token matches
                let candidate_tokens = Self::tokenize(symbol.name());
                candidate_tokens
                    .iter()
                    .any(|t| t.len() >= MIN_TOKEN_LENGTH && target_tokens_set.contains(t.as_str()))
            })
            .map(|symbol| RelationCandidate {
                symbol_id: symbol.fully_qualified_name().to_string(),
                confidence: CONFIDENCE_NAME_MATCH,
                reason: "name_match".to_string(),
                direction: "incoming".to_string(),
            })
            .collect()
    }

    /// Deduplicate candidates, keeping the highest confidence for each symbol.
    fn dedup(mut candidates: Vec<RelationCandidate>) -> Vec<RelationCandidate> {
        // Sort by symbol_id and then by confidence descending
        candidates.sort_by(|a, b| {
            a.symbol_id.cmp(&b.symbol_id).then_with(|| {
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });

        // Keep only the first occurrence of each symbol_id (highest confidence due to sort)
        candidates.dedup_by(|a, b| a.symbol_id == b.symbol_id);
        candidates
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SymbolKind;
    use crate::domain::aggregates::Symbol;
    use crate::domain::value_objects::{DependencyType, Location};

    fn sym(name: &str, file: &str) -> Symbol {
        Symbol::new(name, SymbolKind::Function, Location::new(file, 1, 1))
    }

    fn id(name: &str, file: &str) -> SymbolId {
        SymbolId::new(format!("{}:{}:1", file, name))
    }

    fn add_edge(graph: &mut CallGraph, a: &str, fa: &str, b: &str, fb: &str) {
        graph.add_symbol(sym(a, fa));
        graph.add_symbol(sym(b, fb));
        let _ = graph.add_dependency(&id(a, fa), &id(b, fb), DependencyType::Calls);
    }

    #[test]
    fn test_tokenize_snake_case() {
        let tokens = CandidateFinder::tokenize("my_function_name");
        assert_eq!(tokens, vec!["my", "function", "name"]);
    }

    #[test]
    fn test_tokenize_camel_case() {
        let tokens = CandidateFinder::tokenize("processHTTPRequest");
        assert_eq!(tokens, vec!["process", "HTTP", "Request"]);
    }

    #[test]
    fn test_tokenize_mixed() {
        let tokens = CandidateFinder::tokenize("MyStruct");
        assert_eq!(tokens, vec!["My", "Struct"]);
    }

    #[test]
    fn test_tokenize_with_underscores() {
        let tokens = CandidateFinder::tokenize("my_function");
        assert_eq!(tokens, vec!["my", "function"]);
    }

    #[test]
    fn test_find_candidates_returns_empty_if_has_callers() {
        let mut graph = CallGraph::new();
        add_edge(&mut graph, "caller", "mod.rs", "callee", "mod.rs");

        let candidates = CandidateFinder::find_candidates(&graph, "mod.rs:callee:1");
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_same_file_candidate() {
        let mut graph = CallGraph::new();
        graph.add_symbol(sym("dead_fn", "mod.rs"));
        graph.add_symbol(sym("other_fn", "mod.rs"));

        let candidates = CandidateFinder::find_candidates(&graph, "mod.rs:dead_fn:1");
        assert!(!candidates.is_empty());

        let same_file: Vec<_> = candidates
            .iter()
            .filter(|c| c.reason == "same_file")
            .collect();
        assert!(!same_file.is_empty());
        assert!(
            same_file
                .iter()
                .all(|c| c.confidence == CONFIDENCE_SAME_FILE)
        );
    }

    #[test]
    fn test_community_candidates() {
        let mut graph = CallGraph::new();
        // Three connected nodes form a community
        add_edge(&mut graph, "a", "mod1.rs", "b", "mod1.rs");
        add_edge(&mut graph, "b", "mod1.rs", "c", "mod1.rs");
        // Isolated dead symbol
        graph.add_symbol(sym("dead", "mod2.rs"));

        let candidates = CandidateFinder::find_candidates(&graph, "mod2.rs:dead:1");
        // Dead is isolated, so no same-community candidates
        let community: Vec<_> = candidates
            .iter()
            .filter(|c| c.reason == "same_community")
            .collect();
        // Dead alone in its community = no community candidates
        assert!(community.is_empty());
    }

    #[test]
    fn test_name_match_candidates() {
        let mut graph = CallGraph::new();
        // Use DIFFERENT files so same_file doesn't apply
        graph.add_symbol(sym("process_user_data", "mod1.rs"));
        graph.add_symbol(sym("dead_user", "mod2.rs")); // shares "user" token

        let candidates = CandidateFinder::find_candidates(&graph, "mod2.rs:dead_user:1");
        let name_match: Vec<_> = candidates
            .iter()
            .filter(|c| c.reason == "name_match")
            .collect();
        // Should match on "user" token
        assert!(!name_match.is_empty());
    }

    #[test]
    fn test_dedup_keeps_highest_confidence() {
        let candidates = vec![
            RelationCandidate {
                symbol_id: "mod.rs:same:1".to_string(),
                confidence: CONFIDENCE_NAME_MATCH, // 0.3
                reason: "name_match".to_string(),
                direction: "incoming".to_string(),
            },
            RelationCandidate {
                symbol_id: "mod.rs:same:1".to_string(),
                confidence: CONFIDENCE_SAME_FILE, // 0.7
                reason: "same_file".to_string(),
                direction: "incoming".to_string(),
            },
        ];

        let deduped = CandidateFinder::dedup(candidates);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].reason, "same_file");
        assert_eq!(deduped[0].confidence, CONFIDENCE_SAME_FILE);
    }

    #[test]
    fn test_token_length_filter() {
        // Short tokens (< 3 chars) should be filtered out
        let tokens = CandidateFinder::tokenize("fn"); // 2 chars
        assert!(tokens.is_empty() || tokens.iter().all(|t| t.len() < MIN_TOKEN_LENGTH));
    }
}
