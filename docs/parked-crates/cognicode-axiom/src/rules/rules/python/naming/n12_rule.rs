//! N12 — String concatenation in loop (use join())
//!
//! Detects string concatenation using += inside loops.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_N12"
    name: "String concatenation in loop detected"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Using += on strings inside loops is inefficient. Use str.join() or list append + join pattern instead.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        let lines: Vec<&str> = source.lines().collect();
        let mut loop_indent_stack: Vec<usize> = Vec::new();

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            let line_indent = line.len() - line.trim_start().len();

            if trimmed.starts_with("for ") || trimmed.starts_with("while ") {
                loop_indent_stack.push(line_indent);
                continue;
            }

            while let Some(&loop_ind) = loop_indent_stack.last() {
                if !trimmed.is_empty() && line_indent <= loop_ind {
                    loop_indent_stack.pop();
                } else {
                    break;
                }
            }

            if !loop_indent_stack.is_empty() {
                let concat_re = regex::Regex::new(r"\w+\s*\+=").unwrap();
                if concat_re.is_match(trimmed) {
                    issues.push(Issue::new(
                        "PY_N12",
                        "String concatenation in loop detected. Use str.join() instead for better performance.",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_idx + 1,
                    ).with_remediation(Remediation::quick(
                        "Use list append + join pattern: parts = []; parts.append(x); result = ''.join(parts)"
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
    fn test_n12_registered() {
        let rule = PY_N12Rule::new();
        assert_eq!(rule.id(), "PY_N12");
    }

    #[test]
    fn test_n12_detects_string_concat_in_for() {
        let rule = PY_N12Rule::new();
        let smelly = r#"
for item in items:
    result += item
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect string concat in for loop");
        assert_eq!(issues[0].rule_id, "PY_N12");
    }

    #[test]
    fn test_n12_detects_string_concat_in_while() {
        let rule = PY_N12Rule::new();
        let smelly = r#"
while condition:
    result += "x"
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect string concat in while loop");
    }

    #[test]
    fn test_n12_allows_no_concat() {
        let rule = PY_N12Rule::new();
        let clean = r#"
for item in items:
    parts.append(item)
    x = item + 1
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag code without += in loop");
    }
}
