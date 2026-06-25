//! S2178 — is instead of == with literals
//!
//! Detects using 'is' operator with string or numeric literals instead of '=='.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S2178"
    name: "'is' should not be used with literals"
    severity: Minor
    category: Bug
    language: "Python"
    params: {}

    explanation: "Using 'is' to compare with literals like 'is 42' or 'is \"string\"' is unreliable because Python caches small integers and strings. Use '==' instead.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        // Detect: x is "string", x is 42, x is 3.14, etc.
        let num_literal = regex::Regex::new(r"[a-zA-Z_][a-zA-Z0-9_]*\s+is\s+\d+\.?\d*").unwrap();
        let str_literal = regex::Regex::new(r#"[a-zA-Z_][a-zA-Z0-9_]*\s+is\s+"[^"]*"#).unwrap();
        let str_literal_single = regex::Regex::new(r"[a-zA-Z_][a-zA-Z0-9_]*\s+is\s+'[^']*'").unwrap();
        // Also detect the reverse: "string" is x
        let literal_is_var = regex::Regex::new(r#"\d+\.?\d*\s+is\s+[a-zA-Z_][a-zA-Z0-9_]*"#).unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if num_literal.is_match(trimmed) || str_literal.is_match(trimmed) || str_literal_single.is_match(trimmed) || literal_is_var.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_S2178",
                    "Use '==' instead of 'is' when comparing with literals",
                    Severity::Minor,
                    Category::Bug,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Use '==' for value comparison with literals instead of 'is'."
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
    fn test_s2178_registered() {
        let rule = PY_S2178Rule::new();
        assert_eq!(rule.id(), "PY_S2178");
    }

    #[test]
    fn test_s2178_detects_is_with_string() {
        let rule = PY_S2178Rule::new();
        let smelly = r#"
if x is "hello":
    print("hi")
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect is with string literal");
        assert_eq!(issues[0].rule_id, "PY_S2178");
    }

    #[test]
    fn test_s2178_detects_is_with_number() {
        let rule = PY_S2178Rule::new();
        let smelly = r#"
if value is 42:
    print("answer")
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect is with number literal");
    }

    #[test]
    fn test_s2178_allows_is_with_none() {
        let rule = PY_S2178Rule::new();
        let clean = r#"
if x is None:
    print("null")
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag is None");
    }

    #[test]
    fn test_s2178_allows_equality_with_literal() {
        let rule = PY_S2178Rule::new();
        let clean = r#"
if x == 42:
    print("answer")
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag == with literal");
    }
}
