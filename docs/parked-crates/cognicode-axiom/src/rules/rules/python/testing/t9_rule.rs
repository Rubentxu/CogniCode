//! T9 — Test using random (non-deterministic)
//!
//! Detects tests that use random values, making them non-deterministic.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_T9"
    name: "Test using random values (non-deterministic)"
    severity: Major
    category: Bug
    language: "Python"
    params: {}

    explanation: "Tests using random values are non-deterministic and can produce flaky results. Use fixed seed or deterministic values instead.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find test methods
        let test_method_pattern = regex::Regex::new(r"def (test_\w+)\s*\(").unwrap();

        for cap in test_method_pattern.captures_iter(source) {
            if let Some(method_name) = cap.get(1) {
                let method_start = cap.get(0).unwrap().start();
                let method_name_str = method_name.as_str();

                // Find the method body
                let remaining = &source[method_start..];
                let body_end = remaining[2..]
                    .find("\ndef ")
                    .or_else(|| remaining[2..].find("\nclass "))
                    .unwrap_or(remaining.len() - 2);

                let method_body = &remaining[2..body_end];

                // Check for random usage
                let has_random = method_body.contains("random.")
                    || method_body.contains("random(")
                    || method_body.contains("randint(")
                    || method_body.contains("randrange(")
                    || method_body.contains("choice(")
                    || method_body.contains("shuffle(")
                    || method_body.contains("uniform(")
                    || method_body.contains("numpy.random")
                    || method_body.contains("faker.");

                if has_random {
                    let line_num = source[..method_start].lines().count() + 1;
                    issues.push(Issue::new(
                        "PY_T9",
                        format!("Test '{}' uses random values - tests should be deterministic", method_name_str),
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Use fixed seed (random.seed()) or deterministic test data instead."
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
    fn test_t9_registered() {
        let rule = PY_T9Rule::new();
        assert_eq!(rule.id(), "PY_T9");
    }

    #[test]
    fn test_t9_detects_random_in_test() {
        let rule = PY_T9Rule::new();
        let smelly = r#"
def test_something():
    value = random.randint(1, 100)
    assert value > 0
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect random usage in test");
        assert_eq!(issues[0].rule_id, "PY_T9");
    }

    #[test]
    fn test_t9_detects_random_without_seed() {
        let rule = PY_T9Rule::new();
        let smelly = r#"
def test_something():
    value = random.randint(1, 100)
    assert value > 0
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect random without seed");
        assert_eq!(issues[0].rule_id, "PY_T9");
    }

    #[test]
    fn test_t9_detects_faker() {
        let rule = PY_T9Rule::new();
        let smelly = r#"
def test_user():
    name = faker.name()
    assert len(name) > 0
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect faker usage");
        assert_eq!(issues[0].rule_id, "PY_T9");
    }
}
