//! S2111 — f-string without interpolation
//!
//! Detects f-strings that don't actually interpolate any variables.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S2111"
    name: "f-strings should not be used without interpolation"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "f-strings without interpolation are inefficient. Use regular strings instead.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let fstring_pattern = regex::Regex::new(r#"f"([^"]*)""#).unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            if let Some(caps) = fstring_pattern.captures(line) {
                let content = caps.get(1).map_or("", |m| m.as_str());
                // Check if there's any {} interpolation
                if !content.contains('{') {
                    issues.push(Issue::new(
                        "PY_S2111",
                        format!("f-string without interpolation at line {}", line_num + 1),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::quick(
                        "Use a regular string instead of an f-string when no interpolation is needed."
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
    fn test_s2111_registered() {
        let rule = PY_S2111Rule::new();
        assert_eq!(rule.id(), "PY_S2111");
    }

    #[test]
    fn test_s2111_detects_no_interpolation() {
        let rule = PY_S2111Rule::new();
        let smelly = r#"
def greet():
    return f"Hello World"
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect f-string without interpolation");
        assert_eq!(issues[0].rule_id, "PY_S2111");
    }

    #[test]
    fn test_s2111_allows_interpolation() {
        let rule = PY_S2111Rule::new();
        let clean = r#"
def greet(name):
    return f"Hello {name}"
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag f-string with interpolation");
    }

    #[test]
    fn test_s2111_allows_regular_string() {
        let rule = PY_S2111Rule::new();
        let clean = r#"
def greet():
    return "Hello World"
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag regular string");
    }
}
