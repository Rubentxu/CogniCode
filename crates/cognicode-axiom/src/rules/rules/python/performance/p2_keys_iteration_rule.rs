//! P2 — keys() iteration instead of direct dict iteration
//!
//! Detects unnecessary .keys() calls when iterating over a dictionary.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_P2"
    name: "Unnecessary .keys() iteration over dictionary"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Using .keys() when iterating over a dictionary is redundant. Direct iteration over the dict yields keys.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let keys_pattern = regex::Regex::new(r"\.\s*keys\s*\(\s*\)").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            // Match .keys() in for loops or comprehensions
            if trimmed.starts_with("for ") && keys_pattern.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_P2",
                    format!("Unnecessary .keys() call detected at line {}", line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Remove .keys() and iterate directly over the dictionary."
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
    fn test_p2_registered() {
        let rule = PY_P2Rule::new();
        assert_eq!(rule.id(), "PY_P2");
    }

    #[test]
    fn test_p2_detects_keys_iteration() {
        let rule = PY_P2Rule::new();
        let smelly = r#"
for key in my_dict.keys():
    print(key)
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect .keys() iteration");
        assert_eq!(issues[0].rule_id, "PY_P2");
    }

    #[test]
    fn test_p2_allows_direct_iteration() {
        let rule = PY_P2Rule::new();
        let clean = r#"
for key in my_dict:
    print(key)
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag direct dict iteration");
    }

    #[test]
    fn test_p2_allows_values_iteration() {
        let rule = PY_P2Rule::new();
        let clean = r#"
for value in my_dict.values():
    print(value)
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag .values() iteration");
    }
}
