//! P15 — Repeated string format calls
//!
//! Detects inefficient repeated string formatting that could be combined.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_P15"
    name: "Avoid repeated string formatting calls"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Repeated string formatting calls are inefficient. Consider combining them or using join.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        // Match patterns like "...".format() appearing multiple times
        let format_count_pattern = regex::Regex::new(r#"["'][^"']*["']\s*\.\s*format\s*\("#).unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            let matches: Vec<_> = format_count_pattern.find_iter(trimmed).collect();
            if matches.len() > 1 {
                issues.push(Issue::new(
                    "PY_P15",
                    format!("Multiple format calls on same line detected at line {}", line_num + 1),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num + 1,
                ).with_remediation(Remediation::quick(
                    "Combine multiple format calls or use f-strings for better readability."
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
    fn test_p15_registered() {
        let rule = PY_P15Rule::new();
        assert_eq!(rule.id(), "PY_P15");
    }

    #[test]
    fn test_p15_detects_repeated_format() {
        let rule = PY_P15Rule::new();
        let smelly = r#"
result = "{}".format(a) + "{}".format(b)
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect multiple format calls");
        assert_eq!(issues[0].rule_id, "PY_P15");
    }

    #[test]
    fn test_p15_allows_single_format() {
        let rule = PY_P15Rule::new();
        let clean = r#"
result = "{}".format(value)
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag single format call");
    }

    #[test]
    fn test_p15_allows_fstring() {
        let rule = PY_P15Rule::new();
        let clean = r#"
result = f"{a}{b}"
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag f-strings");
    }
}
