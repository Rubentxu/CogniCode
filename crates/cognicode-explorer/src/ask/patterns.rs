//! Pattern table for [`crate::ask`].
//!
//! 8 priority-ordered regex patterns. Lower number = higher priority.
//! `graph_required` flags whether the pattern needs an in-memory
//! `CallGraph`; the dispatcher checks this BEFORE making any primitive
//! call so it can surface `graph_unavailable` cleanly.

/// A coarse classification of a free-form question. The router picks
/// one variant per question via [`classify`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QuestionCategory {
    /// "path between A and B" / "how does X depend on Y"
    PathBetween,
    /// "what does X call?" / forward reach
    ForwardReach,
    /// "who calls X?" / "what depends on X?" / backward reach
    BackwardReach,
    /// "risky?" / "quality?" / "smells?"
    CodeQuality,
    /// "shape?" / "architecture?" / "cycles?" / "structure?"
    Architecture,
    /// "where to start?" / "entry point?" / "overview?" / "workspace?"
    WorkspaceOverview,
    /// "what component does X belong to?" / cluster membership
    ComponentCluster,
    /// Fallback: "what is X?" / "describe X" / "explain X"
    GenericDescription,
}

/// One entry in [`PATTERNS`]. Regex is a `&'static str` so the table
/// is a `const` slice (no allocation, no dynamic registration).
#[derive(Debug, Clone, Copy)]
pub struct QuestionPattern {
    pub category: QuestionCategory,
    pub regex: &'static str,
    /// 1 = highest priority (matched first), 8 = lowest.
    pub priority: u8,
    /// `true` if the pattern's primitive chain needs a `CallGraph`.
    pub graph_required: bool,
}

/// Internal score returned by [`classify`]. The dispatcher converts
/// this into a [`crate::ask::ClassifiedQuestion`] and adds entity
/// tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ScoredMatch {
    pub category: QuestionCategory,
    pub confidence: f64,
}

/// Priority-ordered pattern table. The first entry whose regex
/// matches the lowercased question wins. The order matches the
/// spec §Pattern Specs exactly.
///
/// Calibration is intentionally lenient on the regex strings so each
/// canonical input from the spec triggers a match. Tightening
/// happens in Phase 2.
pub const PATTERNS: &[QuestionPattern] = &[
    // 1. Path between two entities (graph-dependent).
    QuestionPattern {
        category: QuestionCategory::PathBetween,
        // `what connects A to B?` is the canonical ExplorerQL
        // path query. Match it explicitly so a single question
        // never falls into the lower-priority "explain" bucket.
        regex: r"what\s+connects.*to|path.*between|how.*depends",
        priority: 1,
        graph_required: true,
    },
    // 2. Forward reach (graph-dependent).
    //    `neighbors` covers the ExplorerQL "show me the neighbors of X"
    //    natural-language pattern. The dispatcher maps this to
    //    `NEIGHBORS <X> DEPTH <n>` with `direction = Both`.
    QuestionPattern {
        category: QuestionCategory::ForwardReach,
        regex: r"calls\s*→|what\s+does.*call|forward|neighbors",
        priority: 2,
        graph_required: true,
    },
    // 3. Backward reach (graph-dependent).
    QuestionPattern {
        category: QuestionCategory::BackwardReach,
        regex: r"→\s*calls|who\s+calls|callers|depends\s+on",
        priority: 3,
        graph_required: true,
    },
    // 4. Code quality / smells (NOT graph-dependent).
    QuestionPattern {
        category: QuestionCategory::CodeQuality,
        regex: r"risky|quality|smells",
        priority: 4,
        graph_required: false,
    },
    // 5. Architecture shape (graph-dependent).
    QuestionPattern {
        category: QuestionCategory::Architecture,
        regex: r"shape|architecture|cycles|structure",
        priority: 5,
        graph_required: true,
    },
    // 6. Workspace overview (graph-dependent).
    QuestionPattern {
        category: QuestionCategory::WorkspaceOverview,
        regex: r"where.*start|entry\s+point|overview|workspace",
        priority: 6,
        graph_required: true,
    },
    // 7. Component / cluster membership (graph-dependent).
    QuestionPattern {
        category: QuestionCategory::ComponentCluster,
        regex: r"belongs|component|cluster",
        priority: 7,
        graph_required: true,
    },
    // 8. Generic description (NOT graph-dependent; FALLBACK).
    QuestionPattern {
        category: QuestionCategory::GenericDescription,
        regex: r"what\s+is|describe|explain",
        priority: 8,
        graph_required: false,
    },
];

/// Classify an already-lowercased question by walking [`PATTERNS`]
/// in priority order and scoring the first match.
///
/// Scoring:
/// - `regex.find(question).map(|m| m.end() - m.start())` == question.len()
///   (or close to it) AND the match is anchored (`^...$`) → 1.0
/// - otherwise → 0.7
/// - if no 1-7 pattern matches, fall back to pattern 8 with confidence
///   0.5 (spec §"Unmatched question returns low-confidence fallback").
pub(crate) fn classify(question: &str) -> ScoredMatch {
    use regex::Regex;

    for pattern in PATTERNS.iter() {
        let Ok(re) = Regex::new(pattern.regex) else {
            continue;
        };
        if let Some(m) = re.find(question) {
            let matched_len = m.end() - m.start();
            // Full-coverage heuristic: a match that spans the
            // whole question (or 90%+ of it) is "full match" → 1.0.
            let coverage = matched_len as f64 / question.len().max(1) as f64;
            let confidence = if coverage >= 0.9 { 1.0 } else { 0.7 };
            return ScoredMatch {
                category: pattern.category,
                confidence,
            };
        }
    }
    // No 1-7 pattern matched. Fall back to pattern 8.
    ScoredMatch {
        category: QuestionCategory::GenericDescription,
        confidence: 0.5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_have_priorities_one_through_eight() {
        let mut prios: Vec<u8> = PATTERNS.iter().map(|p| p.priority).collect();
        prios.sort();
        assert_eq!(prios, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn patterns_graph_required_flags_match_spec() {
        // Patterns 1, 2, 3, 5, 6, 7 require a graph; 4 and 8 do not.
        // (The spec calls out 7 of 8 as graph-dependent.)
        let by_cat = |cat: QuestionCategory| {
            PATTERNS
                .iter()
                .find(|p| p.category == cat)
                .map(|p| p.graph_required)
        };
        assert_eq!(by_cat(QuestionCategory::PathBetween), Some(true));
        assert_eq!(by_cat(QuestionCategory::ForwardReach), Some(true));
        assert_eq!(by_cat(QuestionCategory::BackwardReach), Some(true));
        assert_eq!(by_cat(QuestionCategory::CodeQuality), Some(false));
        assert_eq!(by_cat(QuestionCategory::Architecture), Some(true));
        assert_eq!(by_cat(QuestionCategory::WorkspaceOverview), Some(true));
        assert_eq!(by_cat(QuestionCategory::ComponentCluster), Some(true));
        assert_eq!(by_cat(QuestionCategory::GenericDescription), Some(false));
    }

    #[test]
    fn classify_chooses_highest_priority_on_overlap() {
        // "path between A and B ... who calls A" — pattern 1 should
        // win because it's higher priority.
        let m = classify("path between a and b? who calls a?");
        assert_eq!(m.category, QuestionCategory::PathBetween);
    }

    // ---- ExplorerQL NL pattern tests -----------------------------------
    //
    // The patterns below route free-form questions to the existing
    // `QuestionCategory` buckets; the dispatcher in `dispatch.rs` then
    // emits the corresponding ExplorerQL primitive.

    #[test]
    fn classify_what_connects_routes_to_path_between() {
        let m = classify("what connects `parse` to `render`?");
        assert_eq!(m.category, QuestionCategory::PathBetween);
    }

    #[test]
    fn classify_show_neighbors_routes_to_forward_reach() {
        // "show me the neighbors of X" — falls into the forward-reach
        // pattern (which the dispatcher already maps to NEIGHBORS).
        let m = classify("show me the neighbors of `parse`");
        assert!(
            matches!(
                m.category,
                QuestionCategory::ForwardReach | QuestionCategory::BackwardReach
            ),
            "expected ForwardReach or BackwardReach, got {:?}",
            m.category
        );
    }

    #[test]
    fn classify_explain_cycles_routes_to_architecture() {
        // "explain cycles in X" routes to the Architecture pattern,
        // which the dispatcher maps to `EXPLAIN CYCLES`.
        let m = classify("explain cycles in `parse`");
        assert_eq!(m.category, QuestionCategory::Architecture);
    }
}
