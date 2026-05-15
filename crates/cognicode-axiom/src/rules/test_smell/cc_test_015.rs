//! CC_TEST_015: Weak Assertion Style
//!
//! Detects weak assertions like toBeTruthy/toBeFalsy on boolean values.
//!
//! # Problem
//! Weak assertions produce unclear failure messages. When they fail,
//! you don't know if the value was undefined, null, 0, or actually false.
//!
//! # Fix
//! Use precise assertions:
//! - toBe(true) instead of toBeTruthy() for booleans
//! - toBe(false) instead of toBeFalsy() for booleans
//! - toEqual() for object comparisons

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_TEST_015 Rule: Weak Assertion Style
pub struct WeakAssertionStyleRule;

impl Default for WeakAssertionStyleRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for WeakAssertionStyleRule {
    fn id(&self) -> RuleId {
        RuleId("CC_TEST_015")
    }

    fn name(&self) -> &'static str {
        "Weak Assertion Style"
    }

    fn description(&self) -> &'static str {
        "Detects weak assertions like toBeTruthy/toBeFalsy on booleans"
    }

    fn category(&self) -> Category {
        Category::TestSmell
    }

    fn severity(&self) -> Severity {
        Severity::Minor
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[SrcLanguage::Python, SrcLanguage::JavaScript]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Collect call expressions that look like expect().toBeTruthy() etc
        // Pattern: expect(x).toBeTruthy() -> call_expression with function=member_expression
        let weak_assert_query = r#"(call_expression
            function: (member_expression
                object: (call_expression
                    function: (identifier) @expect)
                property: (property_identifier) @assert_method))"#;

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, weak_assert_query) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        // Weak assertion methods to detect
        let weak_assertions = ["toBeTruthy", "toBeFalsy"];

        while let Some(m) = matches.next() {
            let mut method_name = String::new();
            let mut pos = tree_sitter::Point::new(0, 0);

            for cap in m.captures {
                let field_name = &query.capture_names()[cap.index as usize];
                if *field_name == "assert_method" {
                    method_name = cap.node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
                    pos = cap.node.start_position();
                }
            }

            // Only flag weak assertions
            if weak_assertions.contains(&method_name.as_str()) {
                issues.push(Issue::new(
                    "CC_TEST_015",
                    "Weak Assertion Style",
                    Severity::Minor,
                    Category::TestSmell,
                    ctx.file_path.to_string_lossy(),
                    pos.row + 1,
                    pos.column,
                    format!(
                        "Weak assertion '{}' used. Use toBe(true) or toBe(false) \
                         instead of toBeTruthy()/toBeFalsy() for boolean checks \
                         to get clearer failure messages.",
                        method_name
                    ),
                ));
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["toBeTruthy", "toBeFalsy", "toBe", "assertTrue", "assertEqual"])
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
        let rule = WeakAssertionStyleRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_to_be_truthy() {
        let code = r#"
test('checks boolean', () => {
    expect(result).toBeTruthy();
});
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(!issues.is_empty(), "Should detect toBeTruthy");
        assert_eq!(issues[0].rule_id, "CC_TEST_015");
    }

    #[test]
    fn test_no_false_positive_to_be_true() {
        let code = r#"
test('checks boolean', () => {
    expect(result).toBe(true);
});
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(issues.is_empty(), "Should not flag toBe(true)");
    }
}
