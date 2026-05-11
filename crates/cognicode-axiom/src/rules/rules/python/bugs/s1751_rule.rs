//! S1751 — Loop with single iteration
//!
//! Detects for loops or while loops that can only iterate once.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use inventory::submit;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1751"
    name: "Loop should not iterate only once"
    severity: Minor
    category: Bug
    language: "Python"
    params: {}

    explanation: "A loop that can only iterate once should be replaced with a simple conditional or the loop body should be inlined.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Detect for x in [value]: or for x in (value): or range(1)
        let single_iter_list = regex::Regex::new(r"for\s+[a-zA-Z_][a-zA-Z0-9_]*\s+in\s+\[[^\]]+\]:").unwrap();
        let single_iter_tuple = regex::Regex::new(r"for\s+[a-zA-Z_][a-zA-Z0-9_]*\s+in\s+\([^\)]+\):").unwrap();
        let range_one = regex::Regex::new(r"for\s+[a-zA-Z_][a-zA-Z0-9_]*\s+in\s+range\s*\(\s*1\s*\)\s*:").unwrap();
        
        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                continue;
            }
            if single_iter_list.is_match(trimmed) || single_iter_tuple.is_match(trimmed) || range_one.is_match(trimmed) {
                issues.push(Issue::new(
                    "PY_S1751",
                    "Loop with single iteration - loop can only execute once",
                    Severity::Minor,
                    Category::Bug,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Replace the single-iteration loop with a simple conditional or inline the loop body."
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
    fn test_s1751_registered() {
        let rule = PY_S1751Rule::new();
        assert_eq!(rule.id(), "PY_S1751");
    }

    #[test]
    fn test_s1751_detects_single_item_list() {
        let rule = PY_S1751Rule::new();
        let smelly = r#"
for item in [single_value]:
    process(item)
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect single-item list loop");
        assert_eq!(issues[0].rule_id, "PY_S1751");
    }

    #[test]
    fn test_s1751_detects_range_one() {
        let rule = PY_S1751Rule::new();
        let smelly = r#"
for i in range(1):
    print(i)
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect range(1) loop");
    }

    #[test]
    fn test_s1751_allows_multi_iteration() {
        let rule = PY_S1751Rule::new();
        let clean = r#"
for item in items:
    process(item)
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag multi-iteration loop");
    }
}
