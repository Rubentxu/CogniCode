//! N8 — High cyclomatic complexity (>15)
//!
//! Detects functions with high cyclomatic complexity indicating too many branching paths.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_N8"
    name: "Function has high cyclomatic complexity (more than 15)"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Functions with cyclomatic complexity > 15 are hard to test and maintain. Consider refactoring into smaller functions.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find all function definitions
        let func_pattern = regex::Regex::new(r"def\s+(\w+)\s*\([^)]*\):").unwrap();

        for func_cap in func_pattern.captures_iter(source) {
            if let Some(func_name) = func_cap.get(1) {
                // Skip private functions
                if func_name.as_str().starts_with('_') {
                    continue;
                }

                let func_start = func_cap.get(0).unwrap().start();

                // Find the end of the function (next def or end of file)
                let remaining = &source[func_start..];
                let func_end = remaining[2..]
                    .find("\ndef ")
                    .or_else(|| remaining[2..].find("\nclass "))
                    .unwrap_or(remaining.len() - 2);

                let func_body = &remaining[..func_end];

                // Count branching constructs: if, elif, for, while, except, with, and, or
                let branching_pattern = regex::Regex::new(
                    r"\b(if|elif|for|while|except|with|and|or)\b"
                ).unwrap();
                let complexity = branching_pattern.find_iter(func_body).count();

                if complexity > 15 {
                    let line_num = source[..func_start].lines().count() + 1;
                    issues.push(Issue::new(
                        "PY_N8",
                        format!("Function '{}' has cyclomatic complexity of {} (recommended: max 15)", func_name.as_str(), complexity),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Consider refactoring into smaller functions to reduce complexity"
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
    fn test_n8_registered() {
        let rule = PY_N8Rule::new();
        assert_eq!(rule.id(), "PY_N8");
    }

    #[test]
    fn test_n8_detects_high_complexity() {
        let rule = PY_N8Rule::new();
        let smelly = r#"
def complex_function(x):
    if x > 0:
        pass
    if x > 1:
        pass
    if x > 2:
        pass
    if x > 3:
        pass
    if x > 4:
        pass
    if x > 5:
        pass
    if x > 6:
        pass
    if x > 7:
        pass
    if x > 8:
        pass
    if x > 9:
        pass
    if x > 10:
        pass
    if x > 11:
        pass
    if x > 12:
        pass
    if x > 13:
        pass
    if x > 14:
        pass
    if x > 15:
        pass
    if x > 16:
        pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect high cyclomatic complexity");
        assert_eq!(issues[0].rule_id, "PY_N8");
    }

    #[test]
    fn test_n8_allows_low_complexity() {
        let rule = PY_N8Rule::new();
        let clean = r#"
def simple_function(x):
    if x > 0:
        return True
    return False
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag function with low complexity");
    }

    #[test]
    fn test_n8_allows_private_function() {
        let rule = PY_N8Rule::new();
        let clean = r#"
def _helper_function(x):
    if x > 0:
        pass
    if x > 1:
        pass
    # ... many branches
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag private functions");
    }
}