//! CC_TEST_012: Mock Implementation Confusion
//!
//! Detects mocks that use both mockImplementation and mockReturnValue/mockResolvedValue.
//!
//! # Problem
//! Using both mockImplementation and mockReturnValue on the same mock
//! creates confusion about which takes precedence. mockImplementation
//! overrides mockReturnValue but the intent is unclear.
//!
//! # Fix
//! Choose one approach:
//! - mockReturnValue/mockResolvedValue for simple return values
//! - mockImplementation for custom logic

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_TEST_012 Rule: Mock Implementation Confusion
pub struct MockImplementationConfusionRule;

impl Default for MockImplementationConfusionRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for MockImplementationConfusionRule {
    fn id(&self) -> RuleId {
        RuleId("CC_TEST_012")
    }

    fn name(&self) -> &'static str {
        "Mock Implementation Confusion"
    }

    fn description(&self) -> &'static str {
        "Detects mocks using both mockImplementation and mockReturnValue"
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

        // Collect all mock-related call expressions
        // Look for: jest.fn(), spyOn(), mock() calls followed by method chains
        // The object can be any expression (identifier, call_expression, etc.)
        let query = r#"(call_expression
            function: (member_expression
                object: (_) @mock_call
                property: (property_identifier) @mock_method))"#;

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, query) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        let mut mock_methods: Vec<(String, usize)> = Vec::new();

        while let Some(m) = matches.next() {
            let mut mock_call_text = String::new();
            let mut mock_method = String::new();

            for cap in m.captures {
                let field_name = &query.capture_names()[cap.index as usize];
                let text = cap.node.utf8_text(source.as_bytes()).unwrap_or("");

                if *field_name == "mock_call" {
                    // Check if this looks like jest.fn() or similar
                    if text.contains("jest.fn") || text.contains("spyOn") || text.contains("mock()") {
                        mock_call_text = text.to_string();
                    }
                }
                if *field_name == "mock_method" {
                    mock_method = text.to_string();
                    let pos = cap.node.start_position();
                    mock_methods.push((mock_method, pos.row));
                }
            }
        }

        // Check for conflicting patterns
        let has_impl = mock_methods.iter().any(|(m, _)| m == "mockImplementation");
        let has_return = mock_methods.iter().any(|(m, _)| {
            m == "mockReturnValue" || m == "mockResolvedValue" || m == "mockRejectedValue"
        });

        if has_impl && has_return {
            // Find the position of the first conflict
            let pos = mock_methods
                .iter()
                .find(|(m, _)| m == "mockImplementation" || m == "mockReturnValue")
                .map(|(_, pos)| *pos);

            if let Some(row) = pos {
                issues.push(Issue::new(
                    "CC_TEST_012",
                    "Mock Implementation Confusion",
                    Severity::Minor,
                    Category::TestSmell,
                    ctx.file_path.to_string_lossy(),
                    row + 1,
                    0,
                    "Mock uses both mockImplementation and mockReturnValue/mockResolvedValue. \
                     This is confusing as mockImplementation takes precedence. \
                     Choose one approach: mockReturnValue for simple values, \
                     mockImplementation for custom logic.",
                ));
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["mockImplementation", "mockReturnValue", "mockResolvedValue", "jest.fn"])
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
        let rule = MockImplementationConfusionRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_mock_confusion() {
        let code = r#"
const mockFn = jest.fn();
mockFn.mockImplementation(() => 42);
mockFn.mockReturnValue(10);
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(!issues.is_empty(), "Should detect mock confusion");
        assert_eq!(issues[0].rule_id, "CC_TEST_012");
    }

    #[test]
    fn test_no_false_positive_single_approach() {
        let code = r#"
const fn = jest.fn(() => 'value');
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(issues.is_empty(), "Should not flag single approach");
    }
}
