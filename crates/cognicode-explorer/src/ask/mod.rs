//! ask-router — natural-language question router.
//!
//! Pure-function [`AskRouter::classify`] parses a free-form question,
//! selects one of 8 priority-ordered patterns, and returns a
//! [`ClassifiedQuestion`]. A thin async dispatch layer in
//! [`dispatch`] calls `ExplorerService` and `ImpactAnalysisService`
//! directly (no MCP chaining) and wraps the result in the standard
//! [`McpResultEnvelope`].
//!
//! Submodules:
//! - [`patterns`] — `QuestionCategory` enum, `QuestionPattern` struct,
//!   `PATTERNS` const slice.
//! - [`entity`] — backtick extraction + spotter disambiguation.
//! - [`followups`] — deterministic follow-up table per category.
//! - [`dispatch`] — async router entry point used by `mcp.rs`.

pub mod dispatch;
pub mod entity;
pub mod followups;
pub mod patterns;

pub use patterns::{PATTERNS, QuestionCategory, QuestionPattern};

/// Outcome of [`AskRouter::classify`]: which pattern matched, how
/// confident the match was, and the entity tokens that were extracted
/// from the question.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassifiedQuestion {
    pub category: QuestionCategory,
    pub confidence: f64,
    pub entities: Vec<String>,
}

/// Pure-function router: takes a question string, returns a
/// [`ClassifiedQuestion`]. Holds no state, performs no I/O.
pub struct AskRouter;

impl AskRouter {
    /// Classify a free-form question.
    ///
    /// Lowercases the question, walks `PATTERNS` in priority order
    /// (1 = highest), and scores the first match:
    /// - full match   → confidence 1.0
    /// - partial match → confidence 0.7
    /// - keyword fallback (`what is|describe|explain`) → confidence 0.5
    ///
    /// Entity tokens (backtick-quoted substrings) are extracted in
    /// the same pass and included in the result.
    pub fn classify(question: &str) -> ClassifiedQuestion {
        let lower = question.to_lowercase();
        let entities = entity::extract_backtick_tokens(&lower);
        let scored = patterns::classify(&lower);
        ClassifiedQuestion {
            category: scored.category,
            confidence: scored.confidence,
            entities,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- 1.1 Skeleton & types ---------------------------------------------

    #[test]
    fn ask_module_exports_question_category() {
        // The enum must be reachable as `ask::QuestionCategory`.
        let _variant: QuestionCategory = QuestionCategory::PathBetween;
    }

    #[test]
    fn ask_module_exports_question_pattern() {
        // The struct must be reachable as `ask::QuestionPattern`.
        let p = &PATTERNS[0];
        // Field accessors compile, demonstrating the struct shape.
        let _: QuestionCategory = p.category;
        let _: &'static str = p.regex;
        let _: u8 = p.priority;
        let _: bool = p.graph_required;
    }

    #[test]
    fn patterns_constant_has_eight_entries() {
        assert_eq!(PATTERNS.len(), 8, "spec mandates 8 patterns");
    }

    #[test]
    fn patterns_cover_all_eight_categories() {
        use QuestionCategory::*;
        let expected = [
            PathBetween,
            ForwardReach,
            BackwardReach,
            CodeQuality,
            Architecture,
            WorkspaceOverview,
            ComponentCluster,
            GenericDescription,
        ];
        for cat in expected {
            assert!(
                PATTERNS.iter().any(|p| p.category == cat),
                "PATTERNS missing category {:?}",
                cat
            );
        }
    }

    #[test]
    fn classified_question_constructs_with_all_fields() {
        let cq = ClassifiedQuestion {
            category: QuestionCategory::ForwardReach,
            confidence: 0.7,
            entities: vec!["foo".to_string()],
        };
        assert_eq!(cq.category, QuestionCategory::ForwardReach);
        assert!((cq.confidence - 0.7).abs() < f64::EPSILON);
        assert_eq!(cq.entities, vec!["foo".to_string()]);
    }

    #[test]
    fn patterns_have_unique_priorities() {
        // Priorities 1..=8 must each appear exactly once so the
        // ordered walk is deterministic.
        let mut prios: Vec<u8> = PATTERNS.iter().map(|p| p.priority).collect();
        prios.sort();
        assert_eq!(prios, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn ask_router_is_a_unit_struct_with_no_state() {
        // Constructing the router twice must not panic and must not
        // require any args; the type has no fields.
        let _a = AskRouter;
        let _b = AskRouter;
    }

    // ---- 1.2 classify() priorities & fallback ----------------------------

    #[test]
    fn classify_returns_priority1_for_path_between() {
        let q = AskRouter::classify("path between `parse` and `render`");
        assert_eq!(q.category, QuestionCategory::PathBetween);
        assert!(q.confidence >= 0.7);
        assert!(q.entities.contains(&"parse".to_string()));
        assert!(q.entities.contains(&"render".to_string()));
    }

    #[test]
    fn classify_returns_priority2_for_forward_reach() {
        let q = AskRouter::classify("what does `validate()` call?");
        assert_eq!(q.category, QuestionCategory::ForwardReach);
        assert!(q.confidence >= 0.7);
    }

    #[test]
    fn classify_returns_priority3_for_backward_reach() {
        let q = AskRouter::classify("who calls `format_date`?");
        assert_eq!(q.category, QuestionCategory::BackwardReach);
        assert!(q.confidence >= 0.7);
    }

    #[test]
    fn classify_falls_back_to_priority8_for_unmatched() {
        // "tell me a joke" matches NO 1-7 pattern; only pattern 8
        // catches it (via the `what is|describe|explain` fallback).
        // Pattern 8 itself does not match "tell me a joke" either —
        // we expect GenericDescription at 0.5 with `no_pattern_match`
        // follow-up required by the spec. If we can't reach the
        // fallback, this is a contract violation.
        let q = AskRouter::classify("tell me a joke");
        assert_eq!(q.category, QuestionCategory::GenericDescription);
        assert!((q.confidence - 0.5).abs() < f64::EPSILON);
    }
}
