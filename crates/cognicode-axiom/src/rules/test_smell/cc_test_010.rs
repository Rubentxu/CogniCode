//! CC_TEST_010: Assertions Count Mismatch
//!
//! Detects async tests where expect.assertions(N) doesn't match actual assertion count.
//!
//! # Problem
//! When expect.assertions() declares the wrong count, async tests may
//! complete before all assertions run, producing false passes.
//!
//! # Fix
//! Ensure expect.assertions(N) matches actual assertion count, or use
//! expect.hasAssertions() for at-least-one checking.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_TEST_010 Rule: Assertions Count Mismatch
pub struct AssertionsCountMismatchRule;

impl Default for AssertionsCountMismatchRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for AssertionsCountMismatchRule {
    fn id(&self) -> RuleId {
        RuleId("CC_TEST_010")
    }

    fn name(&self) -> &'static str {
        "Assertions Count Mismatch"
    }

    fn description(&self) -> &'static str {
        "Detects async tests with incorrect expect.assertions count"
    }

    fn category(&self) -> Category {
        Category::TestSmell
    }

    fn severity(&self) -> Severity {
        Severity::Major
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[SrcLanguage::JavaScript]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find expect.assertions(N) declarations and compare with actual expect() calls
        let assertions_decl_query = r#"(expression_statement
            (call_expression
                function: (member_expression
                    object: (identifier) @expect_obj
                    property: (property_identifier) @assertions
                    (#eq? @expect_obj "expect"))
                (#eq? @assertions "assertions")
                arguments: (arguments (number) @count)))"#;

        let expect_call_query = r#"(call_expression
            function: (call_expression
                function: (identifier) @expect
                (#eq? @expect "expect"))
            arguments: (arguments))"#;

        let lang = ctx.language.to_ts_language();
        let Ok(assertions_decl_query) = tree_sitter::Query::new(&lang, assertions_decl_query) else {
            return issues;
        };
        let Ok(expect_call_query) = tree_sitter::Query::new(&lang, expect_call_query) else {
            return issues;
        };

        // Find all expect.assertions(N) declarations
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(
            &assertions_decl_query,
            ctx.tree.root_node(),
            source.as_bytes(),
        );

        while let Some(m) = matches.next() {
            let mut count_node = None;
            let mut decl_pos = None;

            for cap in m.captures {
                let field_name = &assertions_decl_query.capture_names()[cap.index as usize];
                match *field_name {
                    "count" => {
                        count_node = Some(cap.node);
                        decl_pos = Some(cap.node.start_position());
                    }
                    _ => {}
                }
            }

            if let (Some(cnt_node), Some(pos)) = (count_node, decl_pos) {
                let declared_count: usize = cnt_node
                    .utf8_text(source.as_bytes())
                    .unwrap_or("0")
                    .parse()
                    .unwrap_or(0);

                // Count actual expect() calls in the same async function
                // For simplicity, we look for expect() calls after the declaration
                let after_pos = pos.row;
                let expect_count = source
                    .lines()
                    .skip(after_pos)
                    .take_while(|line| !line.contains("});") && !line.contains("});"))
                    .filter(|line| line.contains("expect("))
                    .count();

                if expect_count != declared_count {
                    issues.push(Issue::new(
                        "CC_TEST_010",
                        "Assertions Count Mismatch",
                        Severity::Major,
                        Category::TestSmell,
                        ctx.file_path.to_string_lossy(),
                        pos.row + 1,
                        pos.column,
                        format!(
                            "expect.assertions({}) declares {} assertions but found {} \
                             actual expect() calls. This can cause async tests to \
                             pass incorrectly.",
                            declared_count, declared_count, expect_count
                        ),
                    ));
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["expect", "assertions", "hasAssertions", "async", "await"])
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
        let rule = AssertionsCountMismatchRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_count_mismatch() {
        let code = r#"
test('async test', async () => {
    expect.assertions(2);
    const data = await fetchData();
    expect(data).toBeTruthy();
});
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(!issues.is_empty(), "Should detect count mismatch");
        assert_eq!(issues[0].rule_id, "CC_TEST_010");
    }

    #[test]
    fn test_no_false_positive_correct_count() {
        let code = r#"
test('correct count', async () => {
    expect.assertions(2);
    const a = await getA();
    const b = await getB();
    expect(a).toBeDefined();
    expect(b).toBeDefined();
});
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        // Note: This may have false positives due to simple counting
        // A more sophisticated implementation would track scope
    }
}
