//! CC_TEST_013: SpyOn Not Restored
//!
//! Detects spyOn calls that don't have corresponding cleanup.
//!
//! # Problem
//! Without proper cleanup, spies pollute other tests causing
//! cross-test contamination and flaky test behavior.
//!
//! # Fix
//! Add afterEach with jest.restoreAllMocks() or jest.clearAllMocks().
//! Or use jest.spyOn() which auto-restores.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_TEST_013 Rule: SpyOn Not Restored
pub struct SpyOnNotRestoredRule;

impl Default for SpyOnNotRestoredRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for SpyOnNotRestoredRule {
    fn id(&self) -> RuleId {
        RuleId("CC_TEST_013")
    }

    fn name(&self) -> &'static str {
        "SpyOn Not Restored"
    }

    fn description(&self) -> &'static str {
        "Detects spyOn without corresponding cleanup"
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

        // Check if spyOn exists and restoreAllMocks/clearAllMocks exists
        let has_spy_on = source.contains("spyOn(");
        let has_restore = source.contains("restoreAllMocks")
            || source.contains("clearAllMocks")
            || source.contains("mockRestore");

        if has_spy_on && !has_restore {
            let query = r#"(call_expression
                function: (identifier) @spy
                arguments: (arguments)
                (#eq? @spy "spyOn"))"#;

            let lang = ctx.language.to_ts_language();
            let Ok(query) = tree_sitter::Query::new(&lang, query) else {
                return issues;
            };

            let mut cursor = tree_sitter::QueryCursor::new();
            let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

            while let Some(m) = matches.next() {
                for cap in m.captures {
                    let field_name = &query.capture_names()[cap.index as usize];
                    if *field_name == "spy" {
                        let pos = cap.node.start_position();

                        issues.push(Issue::new(
                            "CC_TEST_013",
                            "SpyOn Not Restored",
                            Severity::Major,
                            Category::TestSmell,
                            ctx.file_path.to_string_lossy(),
                            pos.row + 1,
                            pos.column,
                            "spyOn is used without restoreAllMocks/clearAllMocks. \
                             This causes test pollution where spies affect other tests. \
                             Add afterEach with jest.restoreAllMocks() or use jest.spyOn().",
                        ));
                        break;
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["spyOn", "restoreAllMocks", "clearAllMocks", "mockRestore"])
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
        let rule = SpyOnNotRestoredRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_spy_on_without_restore() {
        let code = r#"
test('uses spy', () => {
    spyOn(obj, 'method');
    // missing restore
});
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(!issues.is_empty(), "Should detect spyOn without restore");
        assert_eq!(issues[0].rule_id, "CC_TEST_013");
    }

    #[test]
    fn test_no_false_positive_with_restore() {
        let code = r#"
afterEach(() => {
    restoreAllMocks();
});

test('uses spy', () => {
    spyOn(obj, 'method');
});
"#;
        let issues = check_rule(code, SrcLanguage::JavaScript);
        assert!(issues.is_empty(), "Should not flag spyOn with restore");
    }
}
