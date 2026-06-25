//! S1122 — Fallthrough in except
//!
//! Detects except blocks that don't re-raise or properly handle the exception.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1122"
    name: "Fallthrough in except"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "An except block that doesn't re-raise or raise a new exception can silently continue execution, potentially causing confusing behavior.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;
        let lines: Vec<&str> = source.lines().collect();

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i].trim();
            if line.starts_with("except") && (line.ends_with(':') || line.contains("except ") && line.contains(":")) {
                let except_col = lines[i].len() - lines[i].trim_start().len();
                let mut has_raise = false;
                let mut has_return = false;
                let mut has_pass = false;
                let mut j = i + 1;

                while j < lines.len() {
                    let next_line = lines[j].trim();
                    let next_col = lines[j].len() - lines[j].trim_start().len();

                    if next_col <= except_col && !next_line.is_empty() {
                        break;
                    }

                    if next_line.starts_with("raise") {
                        has_raise = true;
                    }
                    if next_line.starts_with("return") {
                        has_return = true;
                    }
                    if next_line == "pass" {
                        has_pass = true;
                    }
                    j += 1;
                }

                // Flag if block has pass but no raise or return
                if has_pass && !has_raise && !has_return {
                    issues.push(Issue::new(
                        "PY_S1122",
                        format!("except block with pass but no raise at line {}", i + 1),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        i + 1,
                    ).with_remediation(Remediation::quick(
                        "Either re-raise the exception or raise a different one with context."
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
    fn test_s1122_registered() {
        let rule = PY_S1122Rule::new();
        assert_eq!(rule.id(), "PY_S1122");
    }

    #[test]
    fn test_s1122_detects_fallthrough() {
        let rule = PY_S1122Rule::new();
        let smelly = r#"
try:
    do_something()
except:
    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect except with pass but no raise");
        assert_eq!(issues[0].rule_id, "PY_S1122");
    }

    #[test]
    fn test_s1122_allows_proper_handling() {
        let rule = PY_S1122Rule::new();
        let clean = r#"
try:
    do_something()
except Exception as e:
    logger.error("Error occurred")
    raise
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow except with proper handling");
    }
}
