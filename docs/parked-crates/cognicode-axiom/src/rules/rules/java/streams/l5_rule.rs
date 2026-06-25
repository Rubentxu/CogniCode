//! L20 — Stream.flatMap(Collection::stream) simplification
//!
//! Detects `flatMap(Collection::stream)` or `flatMap(x -> x.stream())` which can be simplified.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_L20"
    name: "Stream.flatMap(Collection::stream) can be simplified"
    severity: Info
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "flatMap(Collection::stream) or flatMap(x -> x.stream()) is redundant. The collection is already iterable, so consider using a simpler approach.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect flatMap(Collection::stream) or flatMap(x -> x.stream())
        let pattern1 = regex::Regex::new(r"\.flatMap\s*\(\s*(\w+)\s*::\s*stream\s*\)").unwrap();
        let pattern2 = regex::Regex::new(r"\.flatMap\s*\(\s*\w+\s*->\s*\w+\s*\.\s*stream\s*\(\s*\)\s*\)").unwrap();

        for cap in pattern1.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_L20",
                    "flatMap(x -> x.stream()) is redundant - consider simplifying",
                    Severity::Info,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Consider using a simpler iteration or flatMap with identity."
                )));
            }
        }

        for cap in pattern2.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_L20",
                    "flatMap(x -> x.stream()) is redundant - consider simplifying",
                    Severity::Info,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Consider using a simpler iteration or flatMap with identity."
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
    fn test_l20_registered() {
        let rule = JAVA_L20Rule::new();
        assert_eq!(rule.id(), "JAVA_L20");
    }

    #[test]
    fn test_l20_detects_flatmap_collection_stream() {
        let rule = JAVA_L20Rule::new();
        let smelly = r#"
List<String> result = items.stream()
    .flatMap(List::stream)
    .collect(Collectors.toList());
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect flatMap(List::stream)");
        assert_eq!(issues[0].rule_id, "JAVA_L20");
    }

    #[test]
    fn test_l20_allows_proper_flatmap() {
        let rule = JAVA_L20Rule::new();
        let clean = r#"
List<String> result = items.stream()
    .flatMap(item -> Arrays.asList(item.split(",")))
    .collect(Collectors.toList());
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag proper flatMap usage");
    }
}
