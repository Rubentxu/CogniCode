//! S1541 — Too many branches (>10)
//!
//! Detects functions with too many branches (if/elif/else/case).
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1541"
    name: "Function should not have too many branches"
    severity: Major
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Functions with more than 10 branches are complex and difficult to maintain. Consider refactoring into smaller functions or using a lookup table.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let threshold = 10;
        let branch_keywords = ["if ", "elif ", "else:", "case "];

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("def ") && trimmed.ends_with(':') {
                let start_line = line_num;
                let mut branch_count = 0;
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
                    for keyword in &branch_keywords {
                        if check_trimmed.starts_with(keyword) || check_trimmed.starts_with(&format!("{} ", keyword)) {
                            branch_count += 1;
                            break;
                        }
                    }
                }

                if branch_count > threshold {
                    issues.push(Issue::new(
                        "PY_S1541",
                        format!("Function at line {} has {} branches (threshold: {})", start_line + 1, branch_count, threshold),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        start_line + 1,
                    ).with_remediation(Remediation::quick(
                        "Consider refactoring into smaller functions or using a lookup table."
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
    fn test_s1541_registered() {
        let rule = PY_S1541Rule::new();
        assert_eq!(rule.id(), "PY_S1541");
    }

    #[test]
    fn test_s1541_detects_too_many_branches() {
        let rule = PY_S1541Rule::new();
        let smelly = r#"
def process_status(status):
    if status == 1:
        return "one"
    elif status == 2:
        return "two"
    elif status == 3:
        return "three"
    elif status == 4:
        return "four"
    elif status == 5:
        return "five"
    elif status == 6:
        return "six"
    elif status == 7:
        return "seven"
    elif status == 8:
        return "eight"
    elif status == 9:
        return "nine"
    elif status == 10:
        return "ten"
    elif status == 11:
        return "eleven"
    else:
        return "unknown"
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect too many branches");
        assert_eq!(issues[0].rule_id, "PY_S1541");
    }

    #[test]
    fn test_s1541_allows_normal_branches() {
        let rule = PY_S1541Rule::new();
        let clean = r#"
def process_status(status):
    if status == 1:
        return "one"
    elif status == 2:
        return "two"
    else:
        return "other"
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag function with <= 10 branches");
    }
}
