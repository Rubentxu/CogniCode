//! S3776 — Cognitive complexity (>15)
//!
//! Detects functions with high cognitive complexity.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S3776"
    name: "Cognitive complexity should not be too high"
    severity: Major
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Functions with cognitive complexity over 15 are hard to understand. Consider refactoring to reduce cyclomatic complexity and nesting.",
    clean_code: Clear,
    impacts: [Maintainability: High],
    check: => {
        let mut issues = Vec::new();
        let threshold = 15;

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("def ") && trimmed.ends_with(':') {
                let start_line = line_num;
                let mut complexity = 0;
                let mut indent_level = line.len() - line.trim_start().len();
                let mut nesting_level = 0;

                for (check_line_num, check_line) in ctx.source.lines().enumerate().skip(line_num + 1) {
                    let check_trimmed = check_line.trim();
                    if check_trimmed.is_empty() || check_trimmed.starts_with('#') {
                        continue;
                    }
                    let check_indent = check_line.len() - check_line.trim_start().len();
                    if check_indent <= indent_level && !check_trimmed.is_empty() {
                        break;
                    }

                    let current_nesting = (check_indent - indent_level) / 4;

                    // Increment complexity for nesting
                    if current_nesting > nesting_level {
                        complexity += current_nesting - nesting_level;
                    }
                    nesting_level = current_nesting;

                    // Increment complexity for control structures
                    if check_trimmed.starts_with("if ") || check_trimmed.starts_with("elif ") {
                        complexity += 1;
                    } else if check_trimmed.starts_with("for ") || check_trimmed.starts_with("while ") {
                        complexity += 1;
                    } else if check_trimmed.starts_with("except ") || check_trimmed.starts_with("finally:") {
                        complexity += 1;
                    } else if check_trimmed.starts_with("with ") {
                        complexity += 1;
                    }
                }

                if complexity > threshold {
                    issues.push(Issue::new(
                        "PY_S3776",
                        format!("Function at line {} has cognitive complexity {} (threshold: {})", start_line + 1, complexity, threshold),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        start_line + 1,
                    ).with_remediation(Remediation::quick(
                        "Consider refactoring to reduce complexity - extract helper functions, simplify conditions."
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
    fn test_s3776_registered() {
        let rule = PY_S3776Rule::new();
        assert_eq!(rule.id(), "PY_S3776");
    }

    #[test]
    fn test_s3776_detects_high_complexity() {
        let rule = PY_S3776Rule::new();
        // High complexity with deep nesting
        let smelly = "def complex_function(data):\n    if data:\n        for item in data:\n            if item > 0:\n                while item < 100:\n                    try:\n                        if item % 2 == 0:\n                            if item > 50:\n                                if item > 75:\n                                    pass\n                    except:\n                        pass\n".to_string();
        let issues = with_python_context(&smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect high cognitive complexity");
        assert_eq!(issues[0].rule_id, "PY_S3776");
    }

    #[test]
    fn test_s3776_allows_low_complexity() {
        let rule = PY_S3776Rule::new();
        let clean = r#"
def simple_function(x):
    if x > 0:
        return x * 2
    return 0
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag low complexity function");
    }
}
