//! S154 — High cyclomatic complexity
//!
//! Detects functions with high cyclomatic complexity (too many branches/loops).
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S154"
    name: "Function should not have high cyclomatic complexity"
    severity: Major
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Functions with cyclomatic complexity > 10 are difficult to test and maintain. Consider refactoring into smaller functions.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let threshold = 5;
        let complexity_keywords = ["if ", "elif ", "for ", "while ", "and ", "or ", "except ", "finally", "with "];

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("def ") && trimmed.ends_with(':') {
                let start_line = line_num;
                let mut complexity = 1; // Base complexity
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
                    for keyword in &complexity_keywords {
                        if check_trimmed.starts_with(keyword) {
                            complexity += 1;
                            break;
                        }
                    }
                }

                if complexity > threshold {
                    issues.push(Issue::new(
                        "PY_S154",
                        format!("Function at line {} has cyclomatic complexity {} (threshold: {})", start_line + 1, complexity, threshold),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        start_line + 1,
                    ).with_remediation(Remediation::quick(
                        "Consider refactoring into smaller functions to reduce complexity."
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
    fn test_s154_registered() {
        let rule = PY_S154Rule::new();
        assert_eq!(rule.id(), "PY_S154");
    }

    #[test]
    fn test_s154_detects_high_complexity() {
        let rule = PY_S154Rule::new();
        let smelly = r#"
def process(data):
    if data:
        for x in data:
            if x > 0:
                while True:
                    try:
                        if x > 10 and x < 20:
                            pass
                    except:
                        pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect high cyclomatic complexity");
        assert_eq!(issues[0].rule_id, "PY_S154");
    }

    #[test]
    fn test_s154_allows_low_complexity() {
        let rule = PY_S154Rule::new();
        let clean = r#"
def add(a, b):
    if a > 0 and b > 0:
        return a + b
    return 0
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag function with low complexity");
    }
}
