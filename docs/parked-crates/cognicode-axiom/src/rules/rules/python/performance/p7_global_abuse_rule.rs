//! P7 — global keyword abuse
//!
//! Detects excessive use of global keyword which indicates poor design.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_P7"
    name: "Avoid using global keyword"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Using the global keyword indicates poor encapsulation and makes code harder to test and maintain.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let global_pattern = regex::Regex::new(r"\bglobal\s+\w+").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if global_pattern.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_P7",
                    format!("global keyword detected at line {}", line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Pass values as parameters or use a class to encapsulate state instead of global."
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
    fn test_p7_registered() {
        let rule = PY_P7Rule::new();
        assert_eq!(rule.id(), "PY_P7");
    }

    #[test]
    fn test_p7_detects_global_keyword() {
        let rule = PY_P7Rule::new();
        let smelly = r#"
counter = 0
def increment():
    global counter
    counter += 1
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect global keyword");
        assert_eq!(issues[0].rule_id, "PY_P7");
    }

    #[test]
    fn test_p7_allows_normal_code() {
        let rule = PY_P7Rule::new();
        let clean = r#"
def increment(counter):
    return counter + 1
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag functions without global");
    }

    #[test]
    fn test_p7_detects_multiple_globals() {
        let rule = PY_P7Rule::new();
        let smelly = r#"
def update():
    global x, y, z
    x = 1
    y = 2
    z = 3
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect global with multiple variables");
    }
}
