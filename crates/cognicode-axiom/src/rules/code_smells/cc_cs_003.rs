//! CC_CS_003: Duplicate Branches in If-Else Chain Should Be Consolidated
//!
//! Detects when multiple branches in an if-else chain have identical code.
//!
//! # Problem
//! When multiple branches have identical code, only one will ever execute,
//! making the conditions misleading and indicating dead code.
//!
//! # Fix
//! Consolidate duplicate branches into a single branch with combined condition,
//! or extract common logic into a helper function.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_CS_003 Rule: Duplicate Branches Detection
pub struct DuplicateBranchesRule;

impl Default for DuplicateBranchesRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for DuplicateBranchesRule {
    fn id(&self) -> RuleId {
        RuleId("CC_CS_003")
    }

    fn name(&self) -> &'static str {
        "Duplicate Branches in If-Else Chain Should Be Consolidated"
    }

    fn description(&self) -> &'static str {
        "When multiple branches in an if-else chain have identical code, it indicates dead code."
    }

    fn category(&self) -> Category {
        Category::Maintainability
    }

    fn severity(&self) -> Severity {
        Severity::Major
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[SrcLanguage::Rust]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();

        // Find if_expressions with else clauses and compare their block bodies
        let query_str = r#"
            (if_expression
                consequence: (block) @then_body
                alternative: (else_clause
                    (block) @else_body)
            )
        "#;

        if let Ok(query) = tree_sitter::Query::new(&ctx.language.to_ts_language(), query_str) {
            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), ctx.source.as_bytes());

            while let Some(m) = matches.next() {
                let mut then_text = None;
                let mut else_text = None;
                let mut then_line = 0;

                for capture in m.captures {
                    let text = capture.node.utf8_text(ctx.source.as_bytes()).unwrap_or("");
                    let pt = capture.node.start_position();
                    let kind = capture.node.kind();
                    if kind == "block" {
                        if then_text.is_none() {
                            then_text = Some(text.to_string());
                            then_line = pt.row + 1;
                        } else {
                            else_text = Some(text.to_string());
                        }
                    }
                }

                // Normalize by removing whitespace for comparison
                let normalized = |s: &str| {
                    s.split_whitespace().collect::<Vec<_>>().join(" ")
                };

                if let (Some(then), Some(else_t)) = (then_text, else_text)
                    && !then.is_empty()
                    && normalized(&then) == normalized(&else_t)
                {
                    issues.push(Issue::new(
                        "CC_CS_003",
                        "Duplicate Branches",
                        Severity::Major,
                        Category::Maintainability,
                        ctx.file_path.to_string_lossy(),
                        then_line,
                        0,
                        "Duplicate branches detected - only one condition can be true. Consider combining conditions with ||.",
                    ));
                }
            }
        }
        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["duplicate", "branch", "if-else", "conditional"])
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
        let rule = DuplicateBranchesRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_duplicate_branches() {
        let code = r#"
fn example(x: i32) {
    if x > 0 {
        println!("positive");
    } else {
        println!("positive");
    }
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect duplicate branches");
        assert_eq!(issues[0].rule_id, "CC_CS_003");
    }

    #[test]
    fn test_no_false_positive_different_branches() {
        let code = r#"
fn example(x: i32) {
    if x > 0 {
        println!("positive");
    } else {
        println!("non-positive");
    }
}
"#;
        let issues = check_rule(code);
        assert!(issues.is_empty(), "Should not flag different branches");
    }

    #[test]
    fn test_detects_duplicate_branches_simple_expression() {
        let code = r#"
fn example(flag: bool) {
    if flag {
        return 1;
    } else {
        return 1;
    }
}
"#;
        let issues = check_rule(code);
        assert!(!issues.is_empty(), "Should detect duplicate return statements");
    }
}