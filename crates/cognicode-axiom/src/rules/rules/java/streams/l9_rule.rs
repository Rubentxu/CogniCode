//! L24 — Stream.allMatch on empty stream
//!
//! Detects `.allMatch()` usage and warns that it always returns true on empty streams.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_L24"
    name: "Stream.allMatch on empty stream always returns true"
    severity: Info
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "allMatch() on an empty stream always returns true regardless of the predicate. This may be a logic error if the stream is expected to always contain elements.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect .allMatch( usage
        let pattern = regex::Regex::new(r"\.allMatch\s*\(").unwrap();

        for cap in pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_L24",
                    "allMatch() on empty stream always returns true",
                    Severity::Info,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Consider adding a check for empty stream or using a different approach."
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
    fn test_l24_registered() {
        let rule = JAVA_L24Rule::new();
        assert_eq!(rule.id(), "JAVA_L24");
    }

    #[test]
    fn test_l24_detects_allmatch() {
        let rule = JAVA_L24Rule::new();
        let smelly = r#"
boolean allPositive = items.stream().allMatch(x -> x > 0);
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect allMatch() usage");
        assert_eq!(issues[0].rule_id, "JAVA_L24");
    }

    #[test]
    fn test_l24_allows_none_match() {
        let rule = JAVA_L24Rule::new();
        let clean = r#"
boolean hasMatch = items.stream().anyMatch(x -> x > 0);
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag anyMatch");
    }
}
