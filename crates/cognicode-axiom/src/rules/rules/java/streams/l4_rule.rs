//! L19 — Redundant re-streaming
//!
//! Detects `.collect(Collectors.toList()).stream()` which is redundant re-streaming.
use crate::rules::{CleanCodeAttribute, ImpactSeverity, SoftwareQuality, SoftwareQualityImpact};
use crate::{Category, Issue, Remediation, Rule, RuleContext, RuleEntry, Severity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_L19"
    name: "Redundant re-streaming after collect()"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Collecting to a list and then immediately creating a new stream from it is redundant. The operations before collect() could be used directly or refactored.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect .collect(...).stream() pattern, including simple nested calls inside collect().
        let pattern = regex::Regex::new(r"\.collect\s*\([^;]*\)\s*\.\s*stream\s*\(").unwrap();

        for cap in pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_L19",
                    "Redundant .collect().stream() - remove unnecessary re-streaming",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Remove the .collect().stream() pattern and work directly with the stream."
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
    fn test_l19_registered() {
        let rule = JAVA_L19Rule::new();
        assert_eq!(rule.id(), "JAVA_L19");
    }

    #[test]
    fn test_l19_detects_collect_stream() {
        let rule = JAVA_L19Rule::new();
        let smelly = r#"
List<String> list = items.stream()
    .filter(x -> x.length() > 3)
    .collect(Collectors.toList())
    .stream()
    .map(String::toUpperCase)
    .collect(Collectors.toList());
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(
            !issues.is_empty(),
            "Should detect collect().stream() pattern"
        );
        assert_eq!(issues[0].rule_id, "JAVA_L19");
    }

    #[test]
    fn test_l19_allows_normal_stream_usage() {
        let rule = JAVA_L19Rule::new();
        let clean = r#"
List<String> list = items.stream()
    .filter(x -> x.length() > 3)
    .map(String::toUpperCase)
    .collect(Collectors.toList());
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag normal stream usage");
    }
}
