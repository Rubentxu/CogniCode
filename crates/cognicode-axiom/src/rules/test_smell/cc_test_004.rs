//! CC_TEST_004: Test Method Naming Convention Violation
//!
//! Detects test methods in TestCase classes that don't follow naming conventions.
//!
//! # Problem
//! Test methods without the 'test_' prefix may not be discovered by the
//! test runner, leading to untested code.
//!
//! # Fix
//! Rename methods to start with 'test_' prefix. Example:
//! - def user_login(self) -> def test_user_login(self)

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_TEST_004 Rule: Test Method Naming Convention Violation
pub struct TestNamingConventionRule;

impl Default for TestNamingConventionRule {
    fn default() -> Self {
        Self
    }
}

impl Rule for TestNamingConventionRule {
    fn id(&self) -> RuleId {
        RuleId("CC_TEST_004")
    }

    fn name(&self) -> &'static str {
        "Test Method Naming Convention Violation"
    }

    fn description(&self) -> &'static str {
        "Detects test methods that don't follow naming conventions"
    }

    fn category(&self) -> Category {
        Category::TestSmell
    }

    fn severity(&self) -> Severity {
        Severity::Minor
    }

    fn languages(&self) -> &[SrcLanguage] {
        &[SrcLanguage::Python]
    }

    fn check(&self, ctx: &RuleContext) -> Vec<Issue> {
        let mut issues = Vec::new();
        let source = ctx.source;

        let lang = ctx.language.to_ts_language();
        let valid_class_patterns = ["Test", "TestCase", "Tests"];

        // First, find all class definitions that look like test classes
        let class_query = r#"(class_definition
            name: (identifier) @class_name
            body: (block) @class_body)"#;

        let Ok(class_query) = tree_sitter::Query::new(&lang, class_query) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut class_matches = cursor.matches(&class_query, ctx.tree.root_node(), source.as_bytes());

        while let Some(m) = class_matches.next() {
            let mut class_name = String::new();
            let mut class_body: Option<tree_sitter::Node> = None;

            for cap in m.captures {
                let field_name = &class_query.capture_names()[cap.index as usize];
                match *field_name {
                    "class_name" => {
                        class_name = cap.node.utf8_text(source.as_bytes()).unwrap_or("").to_string();
                    }
                    "class_body" => {
                        class_body = Some(cap.node);
                    }
                    _ => {}
                }
            }

            // Check if this is a test class
            let is_test_class = valid_class_patterns.iter().any(|p| class_name.ends_with(p))
                || class_name.starts_with("Test");

            if !is_test_class {
                continue;
            }

            if let Some(body_node) = class_body {
                // Find all function definitions in this class body
                let func_query = r#"(function_definition
                    name: (identifier) @func_name)"#;

                if let Ok(func_query) = tree_sitter::Query::new(&lang, func_query) {
                    let mut func_cursor = tree_sitter::QueryCursor::new();
                    let mut func_matches = func_cursor.matches(&func_query, body_node, source.as_bytes());

                    while let Some(fm) = func_matches.next() {
                        for cap in fm.captures {
                            let field_name = &func_query.capture_names()[cap.index as usize];
                            if *field_name == "func_name" {
                                let func_name = cap.node.utf8_text(source.as_bytes()).unwrap_or("");
                                let pos = cap.node.start_position();

                                // Skip valid method patterns
                                if func_name.starts_with("test_")
                                    || func_name.starts_with("Test")
                                    || func_name == "setUp"
                                    || func_name == "tearDown"
                                    || func_name == "setup_method"
                                    || func_name == "teardown_method"
                                    || func_name.starts_with("__")
                                    || func_name.ends_with("__")
                                {
                                    continue;
                                }

                                issues.push(Issue::new(
                                    "CC_TEST_004",
                                    "Test Method Naming Convention Violation",
                                    Severity::Minor,
                                    Category::TestSmell,
                                    ctx.file_path.to_string_lossy(),
                                    pos.row + 1,
                                    pos.column,
                                    format!(
                                        "Test method '{}' in class '{}' should start with 'test_'. \
                                         Methods without this prefix may not be discovered by the test runner.",
                                        func_name, class_name
                                    ),
                                ));
                            }
                        }
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["def", "test_", "Test", "setUp", "tearDown"])
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
            std::path::Path::new("test.py"),
            &language,
            &metrics,
        );
        let rule = TestNamingConventionRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_naming_violation() {
        let code = r#"
class TestUser(unittest.TestCase):
    def user_login(self):
        pass
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(!issues.is_empty(), "Should detect method without test_ prefix");
        assert_eq!(issues[0].rule_id, "CC_TEST_004");
    }

    #[test]
    fn test_no_false_positive_with_test_prefix() {
        let code = r#"
class TestUser(unittest.TestCase):
    def test_login(self):
        pass
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(issues.is_empty(), "Should not flag method with test_ prefix");
    }

    #[test]
    fn test_no_false_positive_setUp() {
        let code = r#"
class TestBase(unittest.TestCase):
    def setUp(self):
        self.client = Client()
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(issues.is_empty(), "Should not flag setUp method");
    }
}
