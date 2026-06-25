//! S1164 — Catch-all except
//!
//! Detects bare except clauses that catch all exceptions.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1164"
    name: "Catch-all except"
    severity: Major
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "A bare 'except:' clause catches all exceptions, including KeyboardInterrupt and SystemExit. Catch specific exceptions instead.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let catchall_pattern = regex::Regex::new(r"except\s*:\s*(?:#.*)?$").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed == "except:" || catchall_pattern.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_S1164",
                    format!("Catch-all except clause at line {}", line_num + 1),
                    Severity::Major,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Catch specific exception types instead of using bare except."
                )));
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
    fn test_s1164_registered() {
        let rule = PY_S1164Rule::new();
        assert_eq!(rule.id(), "PY_S1164");
    }

    #[test]
    fn test_s1164_detects_bare_except() {
        let rule = PY_S1164Rule::new();
        let smelly = r#"
try:
    do_something()
except:
    print("Error")
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect bare except clause");
        assert_eq!(issues[0].rule_id, "PY_S1164");
    }

    #[test]
    fn test_s1164_allows_specific_except() {
        let rule = PY_S1164Rule::new();
        let clean = r#"
try:
    do_something()
except ValueError as e:
    print(f"Value error: {e}")
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag specific exception handling");
    }
}
