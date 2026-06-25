//! S1643 — String concatenation in loop
//!
//! Detects string concatenation in loops that should use join().
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1643"
    name: "String concatenation in loop should use join()"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Using + or += for string concatenation in loops is inefficient. Use str.join() instead.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            // Detect loop keywords
            if trimmed.starts_with("for ") || trimmed.starts_with("while ") {
                let mut indent_level = line.len() - line.trim_start().len();

                // Check subsequent lines for augmented assignment (+=)
                for (check_line_num, check_line) in ctx.source.lines().enumerate().skip(line_num + 1) {
                    let check_trimmed = check_line.trim();
                    if check_trimmed.is_empty() {
                        continue;
                    }
                    let check_indent = check_line.len() - check_line.trim_start().len();
                    if check_indent <= indent_level && !check_trimmed.is_empty() {
                        break;
                    }
                    // Check for += augmented assignment (common string concatenation pattern)
                    if check_trimmed.contains("+=") {
                        issues.push(Issue::new(
                            "PY_S1643",
                            format!("String concatenation in loop detected at line {}", check_line_num + 1),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            check_line_num + 1,
                        ).with_remediation(Remediation::quick(
                            "Use str.join() for efficient string concatenation in loops."
                        )));
                        break; // Only report once per loop
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
    fn test_s1643_registered() {
        let rule = PY_S1643Rule::new();
        assert_eq!(rule.id(), "PY_S1643");
    }

    #[test]
    fn test_s1643_detects_concat_in_loop() {
        let rule = PY_S1643Rule::new();
        let smelly = r#"
def concat_items(items):
    result = ""
    for item in items:
        result += item
    return result
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect string concatenation in loop");
        assert_eq!(issues[0].rule_id, "PY_S1643");
    }

    #[test]
    fn test_s1643_allows_join() {
        let rule = PY_S1643Rule::new();
        let clean = r#"
def concat_items(items):
    return "".join(items)
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag join() usage");
    }
}
