//! S148 — Low comment ratio
//!
//! Detects functions with very few or no comments.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S148"
    name: "Functions should have adequate comments"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Functions with more than 10 lines should have comments explaining their purpose and logic.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let min_lines_for_comment = 5;
        let min_comment_ratio = 0.1; // 10% of lines should be comments

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("def ") && trimmed.ends_with(':') {
                let start_line = line_num;
                let mut func_lines = 0;
                let mut comment_lines = 0;
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
                    if check_trimmed.starts_with('#') {
                        comment_lines += 1;
                    }
                }

                if func_lines > min_lines_for_comment {
                    let comment_ratio = comment_lines as f64 / func_lines as f64;
                    if comment_ratio < min_comment_ratio {
                        issues.push(Issue::new(
                            "PY_S148",
                            format!("Function at line {} has low comment ratio ({:.1}%)", start_line + 1, comment_ratio * 100.0),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            start_line + 1,
                        ).with_remediation(Remediation::quick(
                            "Add comments to explain the function's purpose and logic."
                        )));
                    }
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
    fn test_s148_registered() {
        let rule = PY_S148Rule::new();
        assert_eq!(rule.id(), "PY_S148");
    }

    #[test]
    fn test_s148_detects_low_comments() {
        let rule = PY_S148Rule::new();
        let smelly = r#"
def process_data(data):
    result = []
    for item in data:
        if item > 0:
            result.append(item * 2)
        else:
            result.append(0)
    return result
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect low comment ratio");
        assert_eq!(issues[0].rule_id, "PY_S148");
    }

    #[test]
    fn test_s148_allows_well_commented() {
        let rule = PY_S148Rule::new();
        let clean = r#"
def process_data(data):
    # Initialize result list
    result = []
    # Iterate through each item
    for item in data:
        # Double positive items
        if item > 0:
            result.append(item * 2)
        else:
            result.append(0)
    # Return the processed result
    return result
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag well-commented function");
    }
}
