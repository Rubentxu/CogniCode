//! T6 — Test method not starting with test_
//!
//! Detects test methods that don't follow the test_ naming convention.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_T6"
    name: "Test method should start with test_"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Test methods in unittest.TestCase subclasses should start with 'test_' to be automatically discovered by test runners.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find class definitions that inherit from TestCase
        let test_class_pattern = regex::Regex::new(r"class (\w+)\s*\([^)]*TestCase[^)]*\)\s*:").unwrap();

        for class_cap in test_class_pattern.captures_iter(source) {
            if let Some(class_name) = class_cap.get(1) {
                let class_start = class_cap.get(0).unwrap().end();

                // Find methods in this class
                let remaining = &source[class_start..];
                let class_end = remaining.find("\nclass ").unwrap_or(remaining.len());
                let class_body = &remaining[..class_end];

                let method_pattern = regex::Regex::new(r"def (\w+)\s*\(").unwrap();

                for method_cap in method_pattern.captures_iter(class_body) {
                    if let Some(method_name) = method_cap.get(1) {
                        let method_name_str = method_name.as_str();
                        if !method_name_str.starts_with("test_")
                            && !method_name_str.starts_with("setUp")
                            && !method_name_str.starts_with("tearDown")
                            && !method_name_str.starts_with("setUpClass")
                            && !method_name_str.starts_with("tearDownClass") {
                            let line_num = source[..class_start + method_cap.get(0).unwrap().start()].lines().count() + 1;
                            issues.push(Issue::new(
                                "PY_T6",
                                format!("Method '{}' in TestCase class should start with 'test_'", method_name_str),
                                Severity::Minor,
                                Category::CodeSmell,
                                ctx.file_path,
                                line_num,
                            ).with_remediation(Remediation::quick(
                                "Rename method to start with 'test_'"
                            )));
                        }
                    }
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
    fn test_t6_registered() {
        let rule = PY_T6Rule::new();
        assert_eq!(rule.id(), "PY_T6");
    }

    #[test]
    fn test_t6_detects_method_not_starting_with_test() {
        let rule = PY_T6Rule::new();
        let smelly = r#"
class MyTest(unittest.TestCase):
    def helper_method(self):
        pass

    def test_something(self):
        pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect method not starting with test_");
        assert_eq!(issues[0].rule_id, "PY_T6");
    }

    #[test]
    fn test_t6_allows_test_methods() {
        let rule = PY_T6Rule::new();
        let clean = r#"
class MyTest(unittest.TestCase):
    def test_something(self):
        pass
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag test methods");
    }
}
