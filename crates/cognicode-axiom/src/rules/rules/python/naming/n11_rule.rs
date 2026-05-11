//! N11 — Function too complex (>50 lines)
//!
//! Detects functions with bodies longer than 50 lines.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_N11"
    name: "Function body too long (more than 50 lines)"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Functions longer than 50 lines are hard to read and maintain. Consider splitting into smaller, focused functions.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find all function definitions
        let func_pattern = regex::Regex::new(r"def\s+(\w+)\s*\([^)]*\):").unwrap();

        for func_cap in func_pattern.captures_iter(source) {
            if let Some(func_name) = func_cap.get(1) {
                let func_start = func_cap.get(0).unwrap().start();

                // Find the end of the function (next def or end of file)
                let remaining = &source[func_start..];
                let func_end = remaining[2..]
                    .find("\ndef ")
                    .or_else(|| remaining[2..].find("\nclass "))
                    .unwrap_or(remaining.len() - 2);

                let func_body = &remaining[..func_end];

                // Count non-empty, non-comment lines in function body (skip first line = def header)
                let line_count = func_body.lines()
                    .skip(1) // Skip the def line itself
                    .filter(|l| {
                        let trimmed = l.trim();
                        !trimmed.is_empty() && !trimmed.starts_with('#')
                    })
                    .count();

                if line_count > 50 {
                    let line_num = source[..func_start].lines().count() + 1;
                    issues.push(Issue::new(
                        "PY_N11",
                        format!("Function '{}' has {} lines (recommended: max 50)", func_name.as_str(), line_count),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Consider splitting this function into smaller, focused functions"
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
    fn test_n11_registered() {
        let rule = PY_N11Rule::new();
        assert_eq!(rule.id(), "PY_N11");
    }

    #[test]
    fn test_n11_detects_long_function() {
        let rule = PY_N11Rule::new();
        let smelly = r#"
def long_function():
    x = 1
    x = 2
    x = 3
    x = 4
    x = 5
    x = 6
    x = 7
    x = 8
    x = 9
    x = 10
    x = 11
    x = 12
    x = 13
    x = 14
    x = 15
    x = 16
    x = 17
    x = 18
    x = 19
    x = 20
    x = 21
    x = 22
    x = 23
    x = 24
    x = 25
    x = 26
    x = 27
    x = 28
    x = 29
    x = 30
    x = 31
    x = 32
    x = 33
    x = 34
    x = 35
    x = 36
    x = 37
    x = 38
    x = 39
    x = 40
    x = 41
    x = 42
    x = 43
    x = 44
    x = 45
    x = 46
    x = 47
    x = 48
    x = 49
    x = 50
    x = 51
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect long function");
        assert_eq!(issues[0].rule_id, "PY_N11");
    }

    #[test]
    fn test_n11_allows_short_function() {
        let rule = PY_N11Rule::new();
        let clean = r#"
def short_function():
    x = 1
    y = 2
    return x + y
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag short function");
    }

    #[test]
    fn test_n11_allows_exactly_50_lines() {
        let rule = PY_N11Rule::new();
        let clean = r#"
def fifty_lines():
    x = 1
    x = 2
    x = 3
    x = 4
    x = 5
    x = 6
    x = 7
    x = 8
    x = 9
    x = 10
    x = 11
    x = 12
    x = 13
    x = 14
    x = 15
    x = 16
    x = 17
    x = 18
    x = 19
    x = 20
    x = 21
    x = 22
    x = 23
    x = 24
    x = 25
    x = 26
    x = 27
    x = 28
    x = 29
    x = 30
    x = 31
    x = 32
    x = 33
    x = 34
    x = 35
    x = 36
    x = 37
    x = 38
    x = 39
    x = 40
    x = 41
    x = 42
    x = 43
    x = 44
    x = 45
    x = 46
    x = 47
    x = 48
    x = 49
    x = 50
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag exactly 50 lines");
    }
}