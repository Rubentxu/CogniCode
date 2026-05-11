//! S3358 — Nested ternary expressions
//!
//! Detects chained/nested ternary expressions which reduce readability.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S3358"
    name: "Nested ternary expressions should not be used"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Nested ternary expressions like 'x if a else y if b else z' are hard to read and maintain. Use if/else statements or extract to helper functions instead.",
    clean_code: Clear,
    impacts: [Security: Info, Maintainability: High],
    check: => {
        let mut issues = Vec::new();
        // Detect nested ternary: line contains multiple 'if' and 'else' with ternary pattern
        // Simple approach: check for pattern like "X if Y else Z if W else V"
        let nested_ternary = regex::Regex::new(r"\bif\s+.+\s+else\s+\S+\s+if\s+").unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if nested_ternary.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_S3358",
                    "Nested ternary expression reduces readability",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Refactor nested ternary to if/else statements or extract to a helper function."
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
    fn test_s3358_registered() {
        let rule = PY_S3358Rule::new();
        assert_eq!(rule.id(), "PY_S3358");
    }

    #[test]
    fn test_s3358_detects_nested_ternary() {
        let rule = PY_S3358Rule::new();
        let smelly = r#"
result = "a" if x > 0 else "b" if x < 0 else "c"
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect nested ternary");
        assert_eq!(issues[0].rule_id, "PY_S3358");
    }

    #[test]
    fn test_s3358_allows_simple_ternary() {
        let rule = PY_S3358Rule::new();
        let clean = r#"
result = "positive" if x > 0 else "non-positive"
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag simple ternary");
    }
}
