//! S1244 — Float equality
//!
//! Detects float comparisons using == or != which are unreliable due to floating-point precision.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1244"
    name: "Float equality should not be tested directly"
    severity: Minor
    category: Bug
    language: "Python"
    params: {}

    explanation: "Comparing floats with == or != is unreliable due to floating-point precision issues. Use math.isclose() or a tolerance-based comparison instead.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        // Detect float comparisons with == or !=
        // Match patterns like: x == 0.1, x != 0.1, 0.1 == x, etc.
        let float_eq = regex::Regex::new(r"(?x)
            (?:[a-zA-Z_][a-zA-Z0-9_]*\s*(?:==|!=)\s*(?:0?\.[0-9]+|1\.0*)|(?:0?\.[0-9]+|1\.0*)\s*(?:==|!=)\s*[a-zA-Z_][a-zA-Z0-9_]*)
        ").unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            // Skip if it's a comparison with integer
            if float_eq.is_match(line) && !line.contains("int(") {
                issues.push(Issue::new(
                    "PY_S1244",
                    "Float equality - use math.isclose() or tolerance-based comparison",
                    Severity::Minor,
                    Category::Bug,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Use math.isclose(a, b) or compare with a tolerance: abs(a - b) < epsilon"
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
    fn test_s1244_registered() {
        let rule = PY_S1244Rule::new();
        assert_eq!(rule.id(), "PY_S1244");
    }

    #[test]
    fn test_s1244_detects_float_equality() {
        let rule = PY_S1244Rule::new();
        let smelly = r#"
if price == 0.1:
    print("discount")
"#;
        let issues = with_python_context(smelly, "calc.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect float equality");
        assert_eq!(issues[0].rule_id, "PY_S1244");
    }

    #[test]
    fn test_s1244_detects_float_inequality() {
        let rule = PY_S1244Rule::new();
        let smelly = r#"
result = value != 0.3
"#;
        let issues = with_python_context(smelly, "calc.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect float inequality");
    }

    #[test]
    fn test_s1244_allows_integer_comparison() {
        let rule = PY_S1244Rule::new();
        let clean = r#"
if count == 0:
    print("empty")
"#;
        let issues = with_python_context(clean, "calc.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag integer comparison");
    }
}
