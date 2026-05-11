//! S2737 — except with pass
//!
//! Detects except blocks that only contain pass, silently swallowing exceptions.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S2737"
    name: "except with pass"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "An except block that only contains 'pass' silently swallows exceptions. Add logging or a comment explaining why this is intentional.",
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
                let mut j = i + 1;
                let mut only_pass = true;

                while j < lines.len() {
                    let next_line = lines[j].trim();
                    let next_col = lines[j].len() - lines[j].trim_start().len();

                    if next_col <= except_col && !next_line.is_empty() {
                        break;
                    }

                    if !next_line.is_empty() && next_line != "pass" {
                        only_pass = false;
                        break;
                    }
                    j += 1;
                }

                if only_pass && j > i + 1 {
                    issues.push(Issue::new(
                        "PY_S2737",
                        format!("except block with only 'pass' at line {}", i + 1),
                        Severity::Minor,
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
    fn test_s2737_registered() {
        let rule = PY_S2737Rule::new();
        assert_eq!(rule.id(), "PY_S2737");
    }

    #[test]
    fn test_s2737_detects_except_with_pass() {
        let rule = PY_S2737Rule::new();
        let smelly = r#"
try:
    do_something()
except ValueError:
    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect except block with only pass");
        assert_eq!(issues[0].rule_id, "PY_S2737");
    }

    #[test]
    fn test_s2737_allows_proper_handling() {
        let rule = PY_S2737Rule::new();
        let clean = r#"
try:
    do_something()
except ValueError as e:
    logger.error(f"Value error: {e}")
    raise
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag except with proper handling");
    }
}
