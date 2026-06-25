//! S2225 — Exception message not informative
//!
//! Detects exception raising with empty or non-informative messages.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S2225"
    name: "Exception message not informative"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Exception messages should provide useful information about what went wrong and potentially how to fix it.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Match raise Xxx() with empty parens or just whitespace inside
        let raise_empty = regex::Regex::new(r"raise\s+\w+(?:\.\w+)*\s*\(\s*\)").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            if raise_empty.is_match(line) {
                issues.push(Issue::new(
                    "PY_S2225",
                    format!("Exception raised with empty or uninformative message at line {}", line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Add a descriptive error message that explains what went wrong."
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
    fn test_s2225_registered() {
        let rule = PY_S2225Rule::new();
        assert_eq!(rule.id(), "PY_S2225");
    }

    #[test]
    fn test_s2225_detects_empty_exception() {
        let rule = PY_S2225Rule::new();
        let smelly = r#"
raise ValueError()
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect exception with empty message");
        assert_eq!(issues[0].rule_id, "PY_S2225");
    }

    #[test]
    fn test_s2225_allows_informative_exception() {
        let rule = PY_S2225Rule::new();
        let clean = r#"
raise ValueError("Expected positive number, got {}".format(value))
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow informative exception messages");
    }
}
