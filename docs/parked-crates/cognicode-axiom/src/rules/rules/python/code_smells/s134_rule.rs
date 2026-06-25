//! S134 — Deep nesting (>4 levels)
//!
//! Detects code with too many nested levels.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S134"
    name: "Nesting should not be too deep"
    severity: Major
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Code with more than 4 levels of nesting is hard to read and maintain. Consider refactoring to reduce nesting.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let threshold = 4;

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            let indent = line.len() - line.trim_start().len();
            let indent_level = indent / 4; // Assuming 4 spaces per indent

            if indent_level > threshold {
                issues.push(Issue::new(
                    "PY_S134",
                    format!("Deep nesting detected: {} levels (threshold: {})", indent_level, threshold),
                    Severity::Major,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Consider refactoring to reduce nesting - extract to helper functions or use early returns."
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
    fn test_s134_registered() {
        let rule = PY_S134Rule::new();
        assert_eq!(rule.id(), "PY_S134");
    }

    #[test]
    fn test_s134_detects_deep_nesting() {
        let rule = PY_S134Rule::new();
        let smelly = r#"
if a:
    if b:
        if c:
            if d:
                if e:
                    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect deep nesting");
        assert_eq!(issues[0].rule_id, "PY_S134");
    }

    #[test]
    fn test_s134_allows_normal_nesting() {
        let rule = PY_S134Rule::new();
        let clean = r#"
if a:
    if b:
        if c:
            if d:
                pass
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag normal nesting (4 levels)");
    }
}
