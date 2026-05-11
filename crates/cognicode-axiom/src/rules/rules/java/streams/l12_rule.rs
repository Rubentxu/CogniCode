//! L27 — Stream.peek() with side effects
//!
//! Detects `.peek()` usage which may indicate side effects in a stream pipeline.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_L27"
    name: "Stream.peek() with side effects"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "peek() is intended for debugging and should not be used for side effects. Side effects in stream pipelines make code harder to understand and debug.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect .peek( usage
        let pattern = regex::Regex::new(r"\.peek\s*\(").unwrap();

        for cap in pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_L27",
                    "Stream.peek() used - consider if side effects are appropriate",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Consider using forEach() for side effects, or remove peek() if used for debugging."
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
    fn test_l27_registered() {
        let rule = JAVA_L27Rule::new();
        assert_eq!(rule.id(), "JAVA_L27");
    }

    #[test]
    fn test_l27_detects_peek() {
        let rule = JAVA_L27Rule::new();
        let smelly = r#"
List<String> result = items.stream()
    .peek(System.out::println)
    .map(String::toUpperCase)
    .collect(Collectors.toList());
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect peek() usage");
        assert_eq!(issues[0].rule_id, "JAVA_L27");
    }

    #[test]
    fn test_l27_allows_for_each() {
        let rule = JAVA_L27Rule::new();
        let clean = r#"
items.stream().forEach(System.out::println);
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag forEach()");
    }
}
