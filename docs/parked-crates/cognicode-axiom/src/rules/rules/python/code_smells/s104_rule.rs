//! S104 — Module too long
//!
//! Detects Python modules that are too long.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S104"
    name: "Module should not be too long"
    severity: Major
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Files longer than 500 lines are considered too long and should be split.",
    clean_code: Clear,
    impacts: [Maintainability: High],
    check: => {
        let mut issues = Vec::new();
        let threshold = 500;

        let line_count = ctx.source.lines().count();

        if line_count > threshold {
            issues.push(Issue::new(
                "PY_S104",
                format!("Module has {} lines (threshold: {})", line_count, threshold),
                Severity::Major,
                Category::CodeSmell,
                ctx.file_path,
                1,
            ).with_remediation(Remediation::quick(
                "Consider splitting this module into multiple files."
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
    fn test_s104_registered() {
        let rule = PY_S104Rule::new();
        assert_eq!(rule.id(), "PY_S104");
    }

    #[test]
    fn test_s104_detects_long_module() {
        let rule = PY_S104Rule::new();
        let smelly = "# module\n".repeat(501);
        let issues = with_python_context(&smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect long module");
        assert_eq!(issues[0].rule_id, "PY_S104");
    }

    #[test]
    fn test_s104_allows_short_module() {
        let rule = PY_S104Rule::new();
        let clean = "# short module\ndef foo():\n    pass\n";
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag short module");
    }
}
