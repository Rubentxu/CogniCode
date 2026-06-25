//! S107 — Too many parameters (>7)
//!
//! Detects functions with too many parameters.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S107"
    name: "Function should not have too many parameters"
    severity: Major
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Functions with more than 7 parameters are difficult to use and maintain. Consider grouping related parameters into a data class or configuration object.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let threshold = 7;

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("def ") && trimmed.ends_with(':') {
                // Extract parameter list between parentheses
                if let Some(start) = trimmed.find('(') {
                    if let Some(end) = trimmed.rfind(')') {
                        let params_str = trimmed[start+1..end].trim();
                        if params_str.is_empty() {
                            continue;
                        }
                        // Count parameters by splitting on comma
                        let param_count = params_str.split(',').count();

                        if param_count > threshold {
                            issues.push(Issue::new(
                                "PY_S107",
                                format!("Function has {} parameters (threshold: {})", param_count, threshold),
                                Severity::Major,
                                Category::CodeSmell,
                                ctx.file_path,
                                line_num + 1,
                            ).with_remediation(Remediation::quick(
                                "Consider grouping parameters into a data class or configuration object."
                            )));
                        }
                    }
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
    fn test_s107_registered() {
        let rule = PY_S107Rule::new();
        assert_eq!(rule.id(), "PY_S107");
    }

    #[test]
    fn test_s107_detects_too_many_params() {
        let rule = PY_S107Rule::new();
        let smelly = r#"
def create_user(name, age, email, phone, address, city, country, zipcode, role):
    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect too many parameters");
        assert_eq!(issues[0].rule_id, "PY_S107");
    }

    #[test]
    fn test_s107_allows_normal_params() {
        let rule = PY_S107Rule::new();
        let clean = r#"
def add(a, b, c, d, e, f, g):
    return a + b + c + d + e + f + g
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag function with <= 7 params");
    }
}
