//! S108 — Empty except block
//!
//! Detects empty except blocks that silently swallow exceptions.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S108"
    name: "Empty except block"
    severity: Major
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "An empty except block silently swallows exceptions and makes debugging difficult. At minimum, log the exception or re-raise it."
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;
        let lines: Vec<&str> = source.lines().collect();

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i].trim();
            if line.starts_with("except") && (line.ends_with(':') || line.contains("except ") && line.contains(":")) {
                // Find the except block body
                let except_col = lines[i].len() - lines[i].trim_start().len();
                let mut body_empty = true;
                let mut j = i + 1;
                while j < lines.len() {
                    let next_line = lines[j];
                    let next_col = next_line.len() - next_line.trim_start().len();
                    if next_col <= except_col && !next_line.trim().is_empty() {
                        break;
                    }
                    let trimmed_next = next_line.trim();
                    if !trimmed_next.is_empty() && trimmed_next != "pass" {
                        body_empty = false;
                        break;
                    }
                    if trimmed_next == "pass" {
                        body_empty = true;
                        break;
                    }
                    j += 1;
                }
                if body_empty {
                    issues.push(Issue::new(
                        "PY_S108",
                        format!("Empty except block at line {}", i + 1),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        i + 1,
                    ).with_remediation(Remediation::quick(
                        "Add logging or a comment explaining why the exception is silently caught."
                    )));
                }
            }
            i += 1;
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
    fn test_s108_registered() {
        let rule = PY_S108Rule::new();
        assert_eq!(rule.id(), "PY_S108");
    }

    #[test]
    fn test_s108_detects_empty_except() {
        let rule = PY_S108Rule::new();
        let smelly = r#"
try:
    do_something()
except:
    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect empty except block");
        assert_eq!(issues[0].rule_id, "PY_S108");
    }

    #[test]
    fn test_s108_allows_proper_except() {
        let rule = PY_S108Rule::new();
        let clean = r#"
try:
    do_something()
except ValueError as e:
    logger.error("Value error occurred")
    raise
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag except block with proper handling");
    }
}
