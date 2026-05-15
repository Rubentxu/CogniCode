//! CC_TEST_005: Complex Test Fixture Setup
//!
//! Detects test setup methods that are overly complex.
//!
//! # Problem
//! Complex setup methods indicate high coupling and make tests
//! hard to understand and maintain. They often violate the principle
//! of test isolation.
//!
//! # Fix
//! Extract complex object construction into reusable fixtures or
//! helper classes. Use dependency injection for test dependencies.

use crate::context::RuleContext;
use crate::issue::{Category, Issue, Severity};
use crate::types::{Rule, RuleId, SrcLanguage};
use streaming_iterator::StreamingIterator;

/// CC_TEST_005 Rule: Complex Test Fixture Setup
pub struct ComplexFixtureSetupRule;

impl Default for ComplexFixtureSetupRule {
    fn default() -> Self {
        Self
    }
}

impl ComplexFixtureSetupRule {
    fn count_lines_in_block(&self, node: tree_sitter::Node) -> usize {
        if node.kind() == "block" {
            let start = node.start_position();
            let end = node.end_position();
            return end.row - start.row;
        }
        0
    }
}

impl Rule for ComplexFixtureSetupRule {
    fn id(&self) -> RuleId {
        RuleId("CC_TEST_005")
    }

    fn name(&self) -> &'static str {
        "Complex Test Fixture Setup"
    }

    fn description(&self) -> &'static str {
        "Detects test setup methods that are overly complex"
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

        // Collect candidates, filter in post-processing
        let setup_query = match ctx.language {
            SrcLanguage::Python => {
                // Match all function definitions, filter by name later
                r#"(function_definition
                    name: (identifier) @setup_name
                    body: (block) @setup_body)"#
            }
            SrcLanguage::JavaScript => {
                // Match call expressions with arrow function args
                r#"(call_expression
                    function: (identifier) @func
                    arguments: (arguments (arrow_function
                        body: (block) @setup_body))"#
            }
            _ => return issues,
        };

        let lang = ctx.language.to_ts_language();
        let Ok(query) = tree_sitter::Query::new(&lang, setup_query) else {
            return issues;
        };

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&query, ctx.tree.root_node(), source.as_bytes());

        // Setup method name patterns
        let setup_patterns: &[&str] = match ctx.language {
            SrcLanguage::Python => &["setUp", "setup", "setup_method", "setUpClass"],
            SrcLanguage::JavaScript => &["beforeEach", "beforeAll", "setup", "before"],
            _ => return issues,
        };

        while let Some(m) = matches.next() {
            let mut name_node = None;
            let mut body_node = None;

            for cap in m.captures {
                let field_name = &query.capture_names()[cap.index as usize];
                match *field_name {
                    "setup_name" | "func" => name_node = Some(cap.node),
                    "setup_body" => body_node = Some(cap.node),
                    _ => {}
                }
            }

            if let (Some(name_node), Some(body_node)) = (name_node, body_node) {
                let name_text = name_node.utf8_text(source.as_bytes()).unwrap_or("");

                // Filter by setup method name pattern
                let is_setup = setup_patterns.iter().any(|p| name_text == *p);

                if is_setup {
                    let line_count = self.count_lines_in_block(body_node);
                    let pos = body_node.start_position();

                    // Threshold: 10 lines for Python setUp, 8 for JS beforeEach
                    let threshold = match ctx.language {
                        SrcLanguage::Python => 10,
                        SrcLanguage::JavaScript => 8,
                        _ => continue,
                    };

                    if line_count > threshold {
                        issues.push(Issue::new(
                            "CC_TEST_005",
                            "Complex Test Fixture Setup",
                            Severity::Minor,
                            Category::TestSmell,
                            ctx.file_path.to_string_lossy(),
                            pos.row + 1,
                            0,
                            format!(
                                "Test setup method '{}' has {} lines (threshold: {}). \
                                 Complex setup indicates high coupling. Extract shared objects \
                                 into fixtures or helper classes.",
                                name_text, line_count, threshold
                            ),
                        ));
                    }
                }
            }
        }

        issues
    }

    fn preflight_keywords(&self) -> Option<&'static [&'static str]> {
        Some(&["setUp", "beforeEach", "beforeAll", "setup", "fixture"])
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
        let rule = ComplexFixtureSetupRule::default();
        rule.check(&ctx)
    }

    #[test]
    fn test_detects_complex_setup() {
        let code = r#"
def setUp(self):
    self.client = APIClient()
    self.client.set_auth('token123')
    self.user = User()
    self.user.id = 1
    self.user.name = 'test'
    self.db = Database()
    self.db.connect('localhost')
    self.service = Service()
    self.service.init()
    self.mock = Mock()
    self.mock.setup()
    self.config = Config()
    self.config.load()
    self.cache = Cache()
    self.cache.start()
    self.queue = Queue()
    self.queue.initialize()
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(!issues.is_empty(), "Should detect complex setup");
        assert_eq!(issues[0].rule_id, "CC_TEST_005");
    }

    #[test]
    fn test_no_false_positive_simple_setup() {
        let code = r#"
def setUp(self):
    self.client = APIClient()
"#;
        let issues = check_rule(code, SrcLanguage::Python);
        assert!(issues.is_empty(), "Should not flag simple setup");
    }
}
