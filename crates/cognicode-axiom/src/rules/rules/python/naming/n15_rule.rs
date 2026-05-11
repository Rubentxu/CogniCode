//! N15 — f-string without interpolation
//!
//! Detects f-strings that don't contain any interpolation placeholders.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_N15"
    name: "f-string without interpolation detected"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "f-strings without {} placeholders are just regular strings. Remove the 'f' prefix for clarity.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find all f-strings
        // Pattern: f"..." or f'...' (including multi-line)
        let fstring_pattern = regex::Regex::new(r#"f("(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*')"#).unwrap();

        for cap in fstring_pattern.captures_iter(source) {
            if let Some(fstring) = cap.get(0) {
                let fstring_str = fstring.as_str();

                // Check if it contains { ... } interpolation
                // Strip the f prefix and quotes first
                let content = &fstring_str[2..fstring_str.len() - 1];

                // Check for unescaped { characters (not {{ or }})
                // A simple approach: check if { appears before }
                let has_interpolation = content.contains('{') && content.contains('}');

                if !has_interpolation {
                    let line_num = source[..fstring.start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "PY_N15",
                        format!("f-string without interpolation detected. Consider removing the 'f' prefix."),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Remove the 'f' prefix from the string if no interpolation is needed"
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
    fn test_n15_registered() {
        let rule = PY_N15Rule::new();
        assert_eq!(rule.id(), "PY_N15");
    }

    #[test]
    fn test_n15_detects_fstring_without_interpolation() {
        let rule = PY_N15Rule::new();
        let smelly = r#"
x = f"hello world"
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect f-string without interpolation");
        assert_eq!(issues[0].rule_id, "PY_N15");
    }

    #[test]
    fn test_n15_allows_fstring_with_interpolation() {
        let rule = PY_N15Rule::new();
        let clean = r#"
x = f"hello {name}"
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag f-string with interpolation");
    }

    #[test]
    fn test_n15_allows_regular_string() {
        let rule = PY_N15Rule::new();
        let clean = r#"
x = "hello world"
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag regular string");
    }

    #[test]
    fn test_n15_allows_fstring_with_expression() {
        let rule = PY_N15Rule::new();
        let clean = r#"
x = f"result: {1 + 2}"
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag f-string with expression interpolation");
    }
}