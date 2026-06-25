//! S138 — Long function (>50 lines)
//!
//! Detects functions that are too long and should be refactored.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S138"
    name: "Function should not be too long"
    severity: Major
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Functions longer than 50 lines are hard to understand and maintain. Consider splitting them into smaller, more focused functions.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let threshold = 50;

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            // Detect function definition
            if trimmed.starts_with("def ") && trimmed.ends_with(':') {
                // Count lines in this function
                let start_line = line_num;
                let mut func_lines = 0;
                let mut indent_level = line.len() - line.trim_start().len();

                for (check_line_num, check_line) in ctx.source.lines().enumerate().skip(line_num + 1) {
                    let check_trimmed = check_line.trim();
                    if check_trimmed.is_empty() {
                        func_lines += 1;
                        continue;
                    }
                    let check_indent = check_line.len() - check_line.trim_start().len();
                    if check_indent <= indent_level && !check_trimmed.is_empty() {
                        break;
                    }
                    func_lines += 1;
                }

                if func_lines > threshold {
                    issues.push(Issue::new(
                        "PY_S138",
                        format!("Function at line {} has {} lines (threshold: {})", start_line + 1, func_lines, threshold),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        start_line + 1,
                    ).with_remediation(Remediation::quick(
                        "Consider splitting this function into smaller, more focused functions."
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
    fn test_s138_registered() {
        let rule = PY_S138Rule::new();
        assert_eq!(rule.id(), "PY_S138");
    }

    #[test]
    fn test_s138_detects_long_function() {
        let rule = PY_S138Rule::new();
        // Create a function with 55 body lines
        let smelly = "def process_data():\n".to_string() + &"    x = 1\n".repeat(55);
        let issues = with_python_context(&smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect long function");
        assert_eq!(issues[0].rule_id, "PY_S138");
    }

    #[test]
    fn test_s138_allows_short_function() {
        let rule = PY_S138Rule::new();
        let clean = r#"
def add(a, b):
    return a + b
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag short function");
    }
}
