//! T4 — setUp/tearDown vs setUpClass/tearDownClass
//!
//! Detects inefficient per-test setup when class-level setup would be more appropriate.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_T4"
    name: "setUp/tearDown should be setUpClass/tearDownClass"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Using setUp/tearDown for expensive operations that don't vary per test wastes resources. Consider using setUpClass/tearDownClass for class-level setup.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Check if file has setUp/tearDown but no setUpClass/tearDownClass
        let has_setup_teardown = source.contains("def setUp(") || source.contains("def tearDown(");
        let has_setup_class = source.contains("@classmethod") &&
            (source.contains("def setUpClass(") || source.contains("def tearDownClass("));

        if has_setup_teardown && !has_setup_class {
            // Find setUp method and check its body for expensive operations
            let setup_method_pattern = regex::Regex::new(r"def setUp\s*\([^)]*\):").unwrap();

            for cap in setup_method_pattern.find_iter(source) {
                let setup_start = cap.start();
                let remaining = &source[setup_start..];

                // Extract setUp body - find next method or class
                let body_end = remaining[2..]
                    .find("\n    def ")
                    .or_else(|| remaining[2..].find("\nclass "))
                    .map(|p| p + 2)
                    .unwrap_or(remaining.len());

                let set_up_body = &remaining[..body_end];

                // Flag if setUp contains expensive operations
                let has_db = set_up_body.contains("connect(") || set_up_body.contains("cursor(")
                    || set_up_body.contains("Session()") || set_up_body.contains(".query(");
                let has_file = set_up_body.contains("open(") || set_up_body.contains("read(")
                    || set_up_body.contains("write(");
                let has_network = set_up_body.contains("requests.") || set_up_body.contains("http")
                    || set_up_body.contains("urllib");

                if has_db || has_file || has_network {
                    let line_num = source[..setup_start].lines().count() + 1;
                    issues.push(Issue::new(
                        "PY_T4",
                        format!("setUp() contains potentially expensive operations"),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Consider using setUpClass/tearDownClass for expensive per-class setup."
                    )));
                }
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::types::FileMetrics;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;
    use cognicode_core::infrastructure::parser::Language;

    fn with_python_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Python.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Python,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_t4_registered() {
        let rule = PY_T4Rule::new();
        assert_eq!(rule.id(), "PY_T4");
    }

    #[test]
    fn test_t4_detects_db_in_setup() {
        let rule = PY_T4Rule::new();
        let smelly = r#"
class MyTest(unittest.TestCase):
    def setUp(self):
        self.conn = connect("database")
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect database connection in setUp");
        assert_eq!(issues[0].rule_id, "PY_T4");
    }

    #[test]
    fn test_t4_allows_lightweight_setup() {
        let rule = PY_T4Rule::new();
        let clean = r#"
class MyTest(unittest.TestCase):
    def setUp(self):
        self.value = 42
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag lightweight setUp");
    }
}
