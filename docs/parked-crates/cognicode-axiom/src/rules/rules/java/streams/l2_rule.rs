//! L17 — Stream.limit() without sorted()
//!
//! Detects `.limit()` used without prior `.sorted()`, which may produce non-deterministic results.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_L17"
    name: "Stream.limit() without sorted()"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Using limit() on an unsorted stream may produce non-deterministic results since stream order is not guaranteed without sorted().",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect .limit( without prior .sorted(
        // Simple heuristic: find .limit( and check if there's a .sorted( before it in the same chain
        let limit_pattern = regex::Regex::new(r"\.limit\s*\(").unwrap();

        for cap in limit_pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;

                // Look backwards from limit to see if sorted() appears before
                let before_limit = &source[..start];
                let has_sorted_before = before_limit.rfind(".sorted(").is_some();

                if !has_sorted_before {
                    issues.push(Issue::new(
                        "JAVA_L17",
                        "Stream.limit() used without prior sorted() - may produce non-deterministic results",
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Consider adding sorted() before limit() to ensure deterministic results."
                    )));
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
    fn test_l17_registered() {
        let rule = JAVA_L17Rule::new();
        assert_eq!(rule.id(), "JAVA_L17");
    }

    #[test]
    fn test_l17_detects_limit_without_sorted() {
        let rule = JAVA_L17Rule::new();
        let smelly = r#"
List<String> result = items.stream()
    .filter(x -> x.length() > 3)
    .limit(10)
    .collect(Collectors.toList());
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect limit() without sorted()");
        assert_eq!(issues[0].rule_id, "JAVA_L17");
    }

    #[test]
    fn test_l17_allows_limit_with_sorted() {
        let rule = JAVA_L17Rule::new();
        let clean = r#"
List<String> result = items.stream()
    .sorted()
    .limit(10)
    .collect(Collectors.toList());
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag limit() after sorted()");
    }
}
