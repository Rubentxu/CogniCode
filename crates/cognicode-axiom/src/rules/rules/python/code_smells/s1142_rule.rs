//! S1142 — Too many returns (>5)
//!
//! Detects functions with too many return statements.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1142"
    name: "Function should not have too many return statements"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Functions with more than 5 return statements are difficult to understand. Consider refactoring for clarity.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let threshold = 5;

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("def ") && trimmed.ends_with(':') {
                let start_line = line_num;
                let mut return_count = 0;
                let mut indent_level = line.len() - line.trim_start().len();

                for (check_line_num, check_line) in ctx.source.lines().enumerate().skip(line_num + 1) {
                    let check_trimmed = check_line.trim();
                    if check_trimmed.is_empty() {
                        continue;
                    }
                    let check_indent = check_line.len() - check_line.trim_start().len();
                    if check_indent <= indent_level && !check_trimmed.is_empty() {
                        break;
                    }

                    // Count return statements (including early returns)
                    if check_trimmed.contains("return ") {
                        return_count += 1;
                    }
                }

                if return_count > threshold {
                    issues.push(Issue::new(
                        "PY_S1142",
                        format!("Function at line {} has {} return statements (threshold: {})", start_line + 1, return_count, threshold),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        start_line + 1,
                    ).with_remediation(Remediation::quick(
                        "Consider refactoring to reduce the number of return statements."
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
    fn test_s1142_registered() {
        let rule = PY_S1142Rule::new();
        assert_eq!(rule.id(), "PY_S1142");
    }

    #[test]
    fn test_s1142_detects_too_many_returns() {
        let rule = PY_S1142Rule::new();
        let smelly = r#"
def get_status(status):
    if status == 1: return "one"
    if status == 2: return "two"
    if status == 3: return "three"
    if status == 4: return "four"
    if status == 5: return "five"
    if status == 6: return "six"
    return "unknown"
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect too many return statements");
        assert_eq!(issues[0].rule_id, "PY_S1142");
    }

    #[test]
    fn test_s1142_allows_normal_returns() {
        let rule = PY_S1142Rule::new();
        let clean = r#"
def get_status(status):
    if status == 1: return "one"
    if status == 2: return "two"
    if status == 3: return "three"
    return "unknown"
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag function with <= 5 returns");
    }
}
