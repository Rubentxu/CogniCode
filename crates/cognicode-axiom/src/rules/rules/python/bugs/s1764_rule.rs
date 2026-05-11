//! S1764 — Identical operands
//!
//! Detects identical operands in expressions like x == x, x + x, x - x.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1764"
    name: "Identical operands should not be used"
    severity: Minor
    category: Bug
    language: "Python"
    params: {}

    explanation: "Using identical operands in a comparison or arithmetic expression is suspicious and often indicates a bug.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        // Detect expressions where left and right operand are the same variable
        // Pattern: var op var (where op is ==, !=, +, -, *, /)
        let binary_op_re = regex::Regex::new(r"^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*(==|!=|\+|-|\*|/)\s*(.+)\s*$").unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            
            if let Some(caps) = binary_op_re.captures(trimmed) {
                if let (Some(lhs), Some(op), Some(rhs)) = (caps.get(1), caps.get(2), caps.get(3)) {
                    let lhs_str = lhs.as_str();
                    let rhs_str = rhs.as_str().trim();
                    let op_str = op.as_str();
                    
                    // Check if the right side is the same variable (possibly with whitespace)
                    if lhs_str == rhs_str {
                        issues.push(Issue::new(
                            "PY_S1764",
                            &format!("Identical operands '{} {} {}' - suspicious expression", lhs_str, op_str, rhs_str),
                            Severity::Minor,
                            Category::Bug,
                            ctx.file_path,
                            line_num + 1,
                        ).with_remediation(Remediation::quick(
                            "Verify this is intentional. If comparing with itself, consider if a different variable was meant."
                        )));
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
    fn test_s1764_registered() {
        let rule = PY_S1764Rule::new();
        assert_eq!(rule.id(), "PY_S1764");
    }

    #[test]
    fn test_s1764_detects_identical_comparison() {
        let rule = PY_S1764Rule::new();
        // Line-based detection: expression must be on its own line
        let smelly = r#"
x == x
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect identical comparison");
        assert_eq!(issues[0].rule_id, "PY_S1764");
    }

    #[test]
    fn test_s1764_detects_identical_addition() {
        let rule = PY_S1764Rule::new();
        let smelly = r#"
value + value
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect identical addition");
    }

    #[test]
    fn test_s1764_allows_different_operands() {
        let rule = PY_S1764Rule::new();
        let clean = r#"
x + y
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag different operands");
    }
}
