//! S2701 — assert with literal
//!
//! Detects assertions with literal values (assert True, assert False, assert 1).
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S2701"
    name: "assert with literal"
    severity: Major
    category: Bug
    language: "Python"
    params: {}

    explanation: "Assertions with literal values like 'assert True' or 'assert False' are either no-ops or indicate broken tests. Use meaningful expressions.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        // Match: assert True, assert False, assert 1, assert 0, assert None, assert "..."
        let assert_literal = regex::Regex::new(r#"assert\s+(True|False|None|\d+|'[^']*'|"[^"]*")"#).unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("assert ") && assert_literal.is_match(trimmed) {
                // Skip type: ignore comments
                if trimmed.contains("#") && trimmed.contains("type: ignore") {
                    continue;
                }
                issues.push(Issue::new(
                    "PY_S2701",
                    format!("Assertion with literal value at line {}", line_num + 1),
                    Severity::Major,
                    Category::Bug,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Replace the literal with a meaningful boolean expression or variable."
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
    fn test_s2701_registered() {
        let rule = PY_S2701Rule::new();
        assert_eq!(rule.id(), "PY_S2701");
    }

    #[test]
    fn test_s2701_detects_assert_true() {
        let rule = PY_S2701Rule::new();
        let smelly = r#"
assert True
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect assert True");
        assert_eq!(issues[0].rule_id, "PY_S2701");
    }

    #[test]
    fn test_s2701_detects_assert_false() {
        let rule = PY_S2701Rule::new();
        let smelly = r#"
assert False
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect assert False");
        assert_eq!(issues[0].rule_id, "PY_S2701");
    }

    #[test]
    fn test_s2701_allows_meaningful_assert() {
        let rule = PY_S2701Rule::new();
        let clean = r#"
assert result == expected, "Result should match expected"
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow meaningful assertions");
    }
}
