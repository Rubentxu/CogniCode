//! S122 — Source file too long (>1000 lines)
//!
//! Detects Python source files that are too long.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S122"
    name: "Source file should not be too long"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Files longer than 1000 lines are difficult to maintain. Consider splitting into multiple modules.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let threshold = 1000;

        let line_count = ctx.source.lines().count();

        if line_count > threshold {
            issues.push(Issue::new(
                "PY_S122",
                format!("File has {} lines (threshold: {})", line_count, threshold),
                Severity::Minor,
                Category::CodeSmell,
                ctx.file_path,
                1,
            ).with_remediation(Remediation::quick(
                "Consider splitting this file into multiple modules."
            )));
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
    fn test_s122_registered() {
        let rule = PY_S122Rule::new();
        assert_eq!(rule.id(), "PY_S122");
    }

    #[test]
    fn test_s122_detects_long_file() {
        let rule = PY_S122Rule::new();
        let smelly = "def foo():\n    pass\n".repeat(1001);
        let issues = with_python_context(&smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect long file");
        assert_eq!(issues[0].rule_id, "PY_S122");
    }

    #[test]
    fn test_s122_allows_short_file() {
        let rule = PY_S122Rule::new();
        let clean = "def foo():\n    pass\n";
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag short file");
    }
}
