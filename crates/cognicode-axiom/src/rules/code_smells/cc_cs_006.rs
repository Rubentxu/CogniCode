//! CC_CS_006: Wildcard Patterns Should Not Precede Specific Patterns in Match
//!
//! Detects when a wildcard (_) pattern appears before more specific patterns
//! in a match arm's Or pattern.
//!
//! # Problem
//! When wildcard (_) appears before other patterns in an Or pattern, the
//! subsequent patterns will never match because _ catches everything.
//!
//! # Fix
//! Reorder the match arm patterns so that wildcard (_) appears last within
//! an Or pattern.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_CS_006 Rule: Wildcard Before Specific Pattern Detection
pub struct WildcardBeforeSpecificRule;

impl Default for WildcardBeforeSpecificRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for WildcardBeforeSpecificRule {
    fn id(&self) -> RuleId {
        RuleId("CC_CS_006")
    }

    fn name(&self) -> &'static str {
        "Wildcard Patterns Should Not Precede Specific Patterns in Match"
    }

    fn description(&self) -> &'static str {
        "When wildcard (_) appears before more specific patterns in an Or pattern, those patterns will never match."
    }

    fn category(&self) -> Category {
        Category::Correctness
    }

    fn severity(&self) -> Severity {
        Severity::Major
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[SrcLanguage::Rust]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Match match_arm nodes
        let query_str = r#"(match_arm) @arm"#;

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, query_str) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let arm_node = cap.node;
                let arm_text = arm_node.utf8_text(source.as_bytes()).unwrap_or("");

                // Check if the arm contains wildcard followed by | and then specific patterns
                // Pattern: _ | pattern1 | pattern2
                // This is detected by looking for "_ |" in the arm text
                if arm_text.contains("_ |") {
                    let pos = arm_node.start_position();
                    issues.push(Issue::new(
                        "CC_CS_006",
                        "Wildcard Before Specific",
                        Severity::Major,
                        Category::Correctness,
                        ctx.file_path.to_string_lossy(),
                        pos.row + 1,
                        pos.column,
                        "Wildcard (_) pattern appears before specific patterns in match arm. Move _ to the end of the Or pattern.",
                    ));
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["match", "wildcard", "pattern", "_"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_code(code: &str) -> (tree_sitter::Tree, String) {
        let lang = SrcLanguage::Rust.to_ts_language();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse(code, None).unwrap();
        (tree, code.to_string())
    }

    fn check_rule(code: &str) -> Vec<Issue> {
        let (tree, source) = parse_code(code);
        let metrics = crate::types::FileMetrics::default();
        let ctx = RuleContext::new(
            &tree,
            &source,
            std::path::Path::new("test.rs"),
            &SrcLanguage::Rust,
            &metrics,
        );
        let rule = WildcardBeforeSpecificRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_wildcard_before_specific() {
        let code = r#"
fn example(x: i32) {
    match x {
        _ | 1 | 2 => println!("matched"),
    }
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect wildcard before specific");
        assert_eq!(issues[0].rule_id, "CC_CS_006");
    }

    #[test]
    fn test_no_false_positive_wildcard_last() {
        let code = r#"
fn example(x: i32) {
    match x {
        1 | 2 | _ => println!("matched"),
    }
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag wildcard at end");
    }

    #[test]
    fn test_no_false_positive_wildcard_only() {
        let code = r#"
fn example(x: i32) {
    match x {
        _ => println!("matched"),
    }
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag wildcard alone");
    }

    #[test]
    fn test_detects_wildcard_in_middle() {
        let code = r#"
fn example(x: i32) {
    match x {
        1 | _ | 2 => println!("matched"),
    }
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect wildcard in middle");
    }
}