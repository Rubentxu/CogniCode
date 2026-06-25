//! S2259 — None dereference
//!
//! Detects attribute access on potentially None values (e.g., None.something, None()).
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S2259"
    name: "None should not be dereferenced"
    severity: Major
    category: Bug
    language: "Python"
    params: {}

    explanation: "Accessing an attribute or calling a method on None will raise AttributeError at runtime.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        // Detect None.attr or None() patterns
        let none_deref = regex::Regex::new(r"None\.[a-zA-Z_][a-zA-Z0-9_]*|None\s*\(").unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if none_deref.is_match(line) {
                issues.push(Issue::new(
                    "PY_S2259",
                    "None dereference - accessing attribute or calling method on None",
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::moderate(
                    "Add a None check before accessing attributes or methods on potentially None values."
                )));
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
    fn test_s2259_registered() {
        let rule = PY_S2259Rule::new();
        assert_eq!(rule.id(), "PY_S2259");
    }

    #[test]
    fn test_s2259_detects_none_attribute_access() {
        let rule = PY_S2259Rule::new();
        let smelly = r#"
# None attribute access - this would raise AttributeError at runtime
None.something()
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect None.something()");
        assert_eq!(issues[0].rule_id, "PY_S2259");
    }

    #[test]
    fn test_s2259_detects_none_call() {
        let rule = PY_S2259Rule::new();
        let smelly = r#"
# None() call - this would raise TypeError at runtime
result = None()
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect None() call");
        assert_eq!(issues[0].rule_id, "PY_S2259");
    }

    #[test]
    fn test_s2259_allows_valid_code() {
        let rule = PY_S2259Rule::new();
        let clean = r#"
result = some_object
result.method()
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag valid attribute access");
    }
}
