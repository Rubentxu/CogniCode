//! CC_TEST_011: Nested Test Hooks
//!
//! Detects beforeAll/afterAll/beforeEach/afterEach inside nested describe blocks.
//!
//! # Problem
//! Hooks in nested describes cause scoping issues and unexpected
//! execution order. The behavior can be confusing and lead to
//! test pollution.
//!
//! # Fix
//! Move hooks to the appropriate scope (usually top-level describe).
//! Use fixtures for complex initialization needs.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_TEST_011 Rule: Nested Test Hooks
pub struct NestedTestHooksRule;

impl Default for NestedTestHooksRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for NestedTestHooksRule {
    fn id(&self) -> RuleId {
        RuleId("CC_TEST_011")
    }

    fn name(&self) -> &'static str {
        "Nested Test Hooks"
    }

    fn description(&self) -> &'static str {
        "Detects hooks inside nested describe blocks"
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

        // Collect all hook calls (beforeAll, afterAll, beforeEach, afterEach)
        let hook_query = r#"(call_expression
            function: (identifier) @hook
            arguments: (arguments) @hook_args)"#;

        let lang = ctx.language.to_ts_language();
        let Ok(hook_query) = tree_sitter::Query::new(&lang, hook_query) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&hook_query, ctx.tree.root_node(), source.as_bytes());

        // Hook names that indicate test hooks
        let hook_names = ["beforeAll", "afterAll", "beforeEach", "afterEach"];

        while let Some(m) = matches.next() {
            for cap in m.captures {
                let field_name = &hook_query.capture_names()[cap.index as usize];
                if *field_name == "hook" {
                    let hook_name = cap.node.utf8_text(source.as_bytes()).unwrap_or("");

                    if !hook_names.contains(&hook_name) {
                        continue;
                    }

                    // Check if this hook is inside a nested describe
                    // We need to count the describe depth
                    let mut node = cap.node;
                    let mut inside_nested_describe = false;
                    let mut describe_depth = 0;

                    while let Some(parent) = node.parent() {
                        let parent_kind = parent.kind();
                        if parent_kind == "call_expression" {
                            if let Some(func_node) = parent.child(0) {
                                let func_text = func_node.utf8_text(source.as_bytes()).unwrap_or("");
                                if func_text == "describe" {
                                    describe_depth += 1;
                                    if describe_depth >= 2 {
                                        inside_nested_describe = true;
                                        break;
                                    }
                                }
                            }
                        }
                        // Stop at program level
                        if parent_kind == "program" {
                            break;
                        }
                        node = parent;
                    }

                    if inside_nested_describe {
                        let pos = cap.node.start_position();
                        issues.push(Issue::new(
                            "CC_TEST_011",
                            "Nested Test Hooks",
                            Severity::Minor,
                            Category::TestSmell,
                            ctx.file_path.to_string_lossy(),
                            pos.row + 1,
                            pos.column,
                            format!(
                                "Hook '{}' is inside a nested describe block. \
                                 Hooks in nested describes cause scoping issues and \
                                 unpredictable execution order. Move to top-level describe.",
                                hook_name
                            ),
                        ));
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["beforeAll", "afterAll", "beforeEach", "afterEach", "describe"])
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
        let rule = NestedTestHooksRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_nested_before_each() {
        let code = r#"
describe('outer', () => {
    describe('inner', () => {
        beforeEach(() => {
            setup();
        });
        it('test', () => { });
    });
});
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(!issues.is_empty(), "Should detect nested beforeEach");
        assert_eq!(issues[0].rule_id, "CC_TEST_011");
    }

    #[test]
    fn test_no_false_positive_top_level_hook() {
        let code = r#"
describe('Feature', () => {
    beforeEach(() => {
        setup();
    });
    it('works', () => { });
});
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(issues.is_empty(), "Should not flag top-level hook");
    }
}
