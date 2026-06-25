//! L31 — Stream.count() with filter suggestion
//!
//! Detects `.filter(...).count()` and suggests checking collection size directly.
use crate::rules::{CleanCodeAttribute, ImpactSeverity, SoftwareQuality, SoftwareQualityImpact};
use crate::{Category, Issue, Remediation, Rule, RuleContext, RuleEntry, Severity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_L31"
    name: "Stream.filter().count() could be replaced with size check"
    severity: Info
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Using filter().count() on a collection to check if any elements match is inefficient. Consider using a direct size check or other approaches.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect filter() followed by count() in the same statement.
        let filter_count_pattern = regex::Regex::new(r"\.filter\s*\([^;]*\)\s*\.\s*count\s*\(\s*\)").unwrap();

        for cap in filter_count_pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_L31",
                    "filter().count() - consider if a size check would be more appropriate",
                    Severity::Info,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Consider checking collection size directly or using a different approach."
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
    fn test_l31_registered() {
        let rule = JAVA_L31Rule::new();
        assert_eq!(rule.id(), "JAVA_L31");
    }

    #[test]
    fn test_l31_detects_filter_count() {
        let rule = JAVA_L31Rule::new();
        let smelly = r#"
long count = items.stream().filter(x -> x.length() > 3).count();
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect filter().count() pattern");
        assert_eq!(issues[0].rule_id, "JAVA_L31");
    }

    #[test]
    fn test_l31_allows_normal_count() {
        let rule = JAVA_L31Rule::new();
        let clean = r#"
long total = items.stream().count();
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag normal count()");
    }
}
