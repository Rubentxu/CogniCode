//! S1186 — Empty function
//!
//! Detects function definitions with empty bodies.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1186"
    name: "Function body should not be empty"
    severity: Major
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Empty function bodies serve no purpose and may indicate incomplete implementation.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("def ") && trimmed.ends_with(':') {
                let start_line = line_num;
                let mut indent_level = line.len() - line.trim_start().len();
                let mut body_lines = 0;
                let mut is_empty = true;

                for (check_line_num, check_line) in ctx.source.lines().enumerate().skip(line_num + 1) {
                    let check_trimmed = check_line.trim();
                    if check_trimmed.is_empty() {
                        body_lines += 1;
                        continue;
                    }
                    let check_indent = check_line.len() - check_line.trim_start().len();
                    if check_indent <= indent_level && !check_trimmed.is_empty() {
                        break;
                    }

                    // Check if only pass or ellipsis
                    if check_trimmed == "pass" || check_trimmed == "..." {
                        body_lines += 1;
                        continue;
                    }
                    is_empty = false;
                    body_lines += 1;
                }

                if is_empty || body_lines <= 2 {
                    // Check if body is just pass or ...
                    let next_lines: Vec<_> = ctx.source.lines().skip(line_num + 1).take(3).collect();
                    let has_only_pass = next_lines.iter().all(|l| {
                        let t = l.trim();
                        t.is_empty() || t == "pass" || t == "..."
                    });

                    if has_only_pass {
                        issues.push(Issue::new(
                            "PY_S1186",
                            "Empty function detected",
                            Severity::Major,
                            Category::CodeSmell,
                            ctx.file_path,
                            start_line + 1,
                        ).with_remediation(Remediation::quick(
                            "Either implement the function or remove it."
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
    fn test_s1186_registered() {
        let rule = PY_S1186Rule::new();
        assert_eq!(rule.id(), "PY_S1186");
    }

    #[test]
    fn test_s1186_detects_empty_function() {
        let rule = PY_S1186Rule::new();
        let smelly = r#"
def placeholder():
    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect empty function");
        assert_eq!(issues[0].rule_id, "PY_S1186");
    }

    #[test]
    fn test_s1186_allows_implemented_function() {
        let rule = PY_S1186Rule::new();
        let clean = r#"
def implemented(x):
    return x * 2
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag implemented function");
    }
}
