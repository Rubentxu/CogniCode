//! S112 — Generic exception raised
//!
//! Detects raising of generic Exception instead of specific exception types.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S112"
    name: "Generic exceptions should not be raised"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Raising generic Exception makes error handling imprecise. Specific exception types allow callers to handle errors appropriately.",
    clean_code: Clear,
    impacts: [Security: Info, Maintainability: High],
    check: => {
        let mut issues = Vec::new();
        // Detect raise Exception( - but not raise Exception from ...
        let raise_exception = regex::Regex::new(r"raise\s+Exception\s*\(").unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if raise_exception.is_match(line) && !trimmed.contains("from ") {
                issues.push(Issue::new(
                    "PY_S112",
                    "Generic Exception raised - use a more specific type",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::moderate(
                    "Raise a specific exception type (ValueError, TypeError, custom exception) instead of generic Exception."
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
    fn test_s112_registered() {
        let rule = PY_S112Rule::new();
        assert_eq!(rule.id(), "PY_S112");
    }

    #[test]
    fn test_s112_detects_raise_exception() {
        let rule = PY_S112Rule::new();
        let smelly = r#"
def validate(value):
    if not value:
        raise Exception("Value is required")
"#;
        let issues = with_python_context(smelly, "validator.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect raise Exception()");
        assert_eq!(issues[0].rule_id, "PY_S112");
    }

    #[test]
    fn test_s112_allows_specific_exception() {
        let rule = PY_S112Rule::new();
        let clean = r#"
def validate(value):
    if not value:
        raise ValueError("Value is required")
"#;
        let issues = with_python_context(clean, "validator.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag specific exceptions");
    }
}
