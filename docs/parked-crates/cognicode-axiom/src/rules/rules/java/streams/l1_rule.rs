//! L16 — Stream.distinct().sorted() order matters
//!
//! Detects `.distinct().sorted()` chains where order of operations may affect behavior.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_L16"
    name: "Stream.distinct().sorted() order matters"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "When using distinct() before sorted(), the order of operations matters for performance and semantics. distinct() removes duplicates based on equals() method, which may behave differently before or after sorting.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect .distinct().sorted() pattern
        let pattern = regex::Regex::new(r"\.distinct\(\)\s*\.\s*sorted\s*\(").unwrap();

        for cap in pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_L16",
                    "Stream.distinct().sorted() - consider if order of operations is correct",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Consider if sorted() should be called before distinct() for better performance."
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
    fn test_l16_registered() {
        let rule = JAVA_L16Rule::new();
        assert_eq!(rule.id(), "JAVA_L16");
    }

    #[test]
    fn test_l16_detects_distinct_before_sorted() {
        let rule = JAVA_L16Rule::new();
        let smelly = r#"
List<String> result = items.stream()
    .distinct()
    .sorted()
    .collect(Collectors.toList());
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect distinct().sorted() pattern");
        assert_eq!(issues[0].rule_id, "JAVA_L16");
    }

    #[test]
    fn test_l16_allows_sorted_before_distinct() {
        let rule = JAVA_L16Rule::new();
        let clean = r#"
List<String> result = items.stream()
    .sorted()
    .distinct()
    .collect(Collectors.toList());
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag sorted().distinct() pattern");
    }
}
