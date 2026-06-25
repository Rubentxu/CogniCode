//! L33 — Optional.of(null) should be Optional.ofNullable()
//!
//! Detects `Optional.of(null)` which throws NullPointerException.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_L33"
    name: "Optional.of(null) throws NullPointerException"
    severity: Critical
    category: Bug
    language: "Java"
    params: {}

    explanation: "Optional.of(null) throws NullPointerException. Use Optional.ofNullable() instead if the value might be null.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect Optional.of(null)
        let pattern = regex::Regex::new(r"Optional\s*\.\s*of\s*\(\s*null\s*\)").unwrap();

        for cap in pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_L33",
                    "Optional.of(null) throws NullPointerException - use ofNullable()",
                    Severity::Critical,
                    Category::Bug,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Replace Optional.of(null) with Optional.ofNullable(null)."
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
    use cognicode_core::infrastructure::parser::Language;

    fn with_java_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Java.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Java,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_l33_registered() {
        let rule = JAVA_L33Rule::new();
        assert_eq!(rule.id(), "JAVA_L33");
    }

    #[test]
    fn test_l33_detects_optional_of_null() {
        let rule = JAVA_L33Rule::new();
        let smelly = r#"
Optional<String> opt = Optional.of(null);
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect Optional.of(null)");
        assert_eq!(issues[0].rule_id, "JAVA_L33");
    }

    #[test]
    fn test_l33_allows_optional_of_value() {
        let rule = JAVA_L33Rule::new();
        let clean = r#"
Optional<String> opt = Optional.of("value");
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag Optional.of with non-null value");
    }
}
