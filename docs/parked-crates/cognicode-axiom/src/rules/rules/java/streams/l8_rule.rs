//! L23 — IntStream.boxed() before collect() warning
//!
//! Detects `IntStream.boxed().collect()` which involves unnecessary boxing.
use crate::rules::{CleanCodeAttribute, ImpactSeverity, SoftwareQuality, SoftwareQualityImpact};
use crate::{Category, Issue, Remediation, Rule, RuleContext, RuleEntry, Severity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_L23"
    name: "Unnecessary boxing in Stream.collect()"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Using boxed() before collect() on primitive streams involves unnecessary boxing. Consider using collectors that work directly with primitives.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect primitive stream pipelines that box before collecting.
        let pattern = regex::Regex::new(r"(?:IntStream|LongStream|DoubleStream)\s*\.\s*\w+\s*\([^;]*?\.\s*boxed\s*\(\s*\)\s*\.\s*collect\s*\(").unwrap();

        for cap in pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_L23",
                    "Unnecessary boxing before collect() on primitive stream",
                    Severity::Minor,
                    Category:: CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Consider using Collectors.toSet() or other collectors that work with primitives directly."
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
    fn test_l23_registered() {
        let rule = JAVA_L23Rule::new();
        assert_eq!(rule.id(), "JAVA_L23");
    }

    #[test]
    fn test_l23_detects_boxed_collect() {
        let rule = JAVA_L23Rule::new();
        let smelly = r#"
List<Integer> result = IntStream.range(0, 10)
    .boxed()
    .collect(Collectors.toList());
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(
            !issues.is_empty(),
            "Should detect boxed().collect() pattern"
        );
        assert_eq!(issues[0].rule_id, "JAVA_L23");
    }

    #[test]
    fn test_l23_allows_direct_collect() {
        let rule = JAVA_L23Rule::new();
        let clean = r#"
int sum = IntStream.range(0, 10).sum();
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(
            issues.is_empty(),
            "Should not flag direct primitive operations"
        );
    }
}
