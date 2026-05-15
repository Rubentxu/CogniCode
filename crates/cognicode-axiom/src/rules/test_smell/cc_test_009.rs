//! CC_TEST_009: Test Without Describe Block
//!
//! Detects JavaScript tests that are not organized in describe blocks.
//!
//! # Problem
//! Root-level tests without describe blocks make it hard to navigate
//! and understand the test organization, especially as test suites grow.
//!
//! # Fix
//! Wrap related tests in describe blocks grouping by feature or component.
//! Use nested describes for sub-groupings.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_TEST_009 Rule: Test Without Describe Block
pub struct TestWithoutDescribeRule;

impl Default for TestWithoutDescribeRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for TestWithoutDescribeRule {
    fn id(&self) -> RuleId {
        RuleId("CC_TEST_009")
    }

    fn name(&self) -> &'static str {
        "Test Without Describe Block"
    }

    fn description(&self) -> &'static str {
        "Detects tests not organized in describe blocks"
    }

    fn category(&self) -> Category {
        Category::TestSmell
    }

    fn severity(&self) -> Severity {
        Severity::Minor
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[SrcLanguage::JavaScript]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Collect all test/it calls first
        let query = r#"(call_expression
            function: (identifier) @test_func
            arguments: (arguments) @test_args)"#;

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, query) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        // Valid test function names
        let test_funcs = ["test", "it", "specify", "ftest", "fdescribe"];

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let field_name = &query.capture_names()[cap.index as usize];
                if *field_name == "test_func" {
                    let func_name = cap.node.utf8_text(source.as_bytes()).unwrap_or("");

                    if !test_funcs.contains(&func_name) {
                        continue;
                    }

                    // Check if this call is inside a describe block
                    let mut node = cap.node;
                    let mut inside_describe = false;

                    // Walk up the tree to see if we're inside a describe call
                    // Start from the call_expression itself (parent of identifier)
                    let mut current = node.parent();
                    while let Some(parent) = current {
                        let parent_kind = parent.kind();
                        if parent_kind == "call_expression" {
                            // Check if this is a describe call by looking at the function field
                            if let Some(func_node) = parent.child_by_field_name("function") {
                                let func_text = func_node.utf8_text(source.as_bytes()).unwrap_or("");
                                if func_text == "describe" {
                                    inside_describe = true;
                                    break;
                                }
                            }
                        }
                        // Stop at program level
                        if parent_kind == "program" {
                            break;
                        }
                        current = parent.parent();
                    }

                    if !inside_describe {
                        let pos = cap.node.start_position();
                        issues.push(Issue::new(
                            "CC_TEST_009",
                            "Test Without Describe Block",
                            Severity::Minor,
                            Category::TestSmell,
                            ctx.file_path.to_string_lossy(),
                            pos.row + 1,
                            pos.column,
                            format!(
                                "Test '{}' is not inside a describe block. \
                                 Organize tests by feature or component using describe().",
                                func_name
                            ),
                        ));
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["it(", "test(", "describe(", "specify("])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_code(code: &str, language: SrcLanguage) -> (tree_sitter::Tree, String) {
        let lang = language.to_ts_language();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang).unwrap();
        let tree = parser.parse(code, None).unwrap();
        (tree, code.to_string())
    }

    fn check_rule(code: &str, language: SrcLanguage) -> Vec<Issue> {
        let (tree, source) = parse_code(code, language);
        let metrics = crate::types::FileMetrics::default();
        let ctx = RuleContext::new(
            &tree,
            &source,
            std::path::Path::new("test.js"),
            &language,
            &metrics,
        );
        let rule = TestWithoutDescribeRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_root_level_it() {
        let code = r#"
it('should work', () => {
    expect(true).toBe(true);
});
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(!issues.is_empty(), "Should detect root-level it");
        assert_eq!(issues[0].rule_id, "CC_TEST_009");
    }

    #[test]
    fn test_no_false_positive_nested_in_describe() {
        let code = r#"
describe('User', () => {
    it('should login', () => {
        expect(true).toBe(true);
    });
});
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(issues.is_empty(), "Should not flag test inside describe");
    }
}
