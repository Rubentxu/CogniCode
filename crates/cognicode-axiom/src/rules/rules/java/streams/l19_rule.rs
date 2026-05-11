//! L34 — Redundant stream().collect(Collectors.joining(""))
//!
//! Detects `.stream().collect(Collectors.joining(""))` which could be replaced with StringBuilder.
use crate::rules::{CleanCodeAttribute, ImpactSeverity, SoftwareQuality, SoftwareQualityImpact};
use crate::{Category, Issue, Remediation, Rule, RuleContext, RuleEntry, Severity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_L34"
    name: "Redundant stream().collect(Collectors.joining()) for StringBuilder"
    severity: Info
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Using stream().collect(Collectors.joining(\"\")) is less efficient than using StringBuilder directly for concatenating strings.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect .stream().collect(Collectors.joining(""))
        let pattern = regex::Regex::new(r#"\.stream\s*\(\s*\)\s*\.\s*collect\s*\(\s*Collectors\s*\.\s*joining\s*\(\s*""\s*\)\s*\)"#).unwrap();

        for cap in pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_L34",
                    "stream().collect(Collectors.joining(\"\")) - consider using StringBuilder",
                    Severity::Info,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Consider using StringBuilder directly for better performance."
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
    fn test_l34_registered() {
        let rule = JAVA_L34Rule::new();
        assert_eq!(rule.id(), "JAVA_L34");
    }

    #[test]
    fn test_l34_detects_joining_empty_string() {
        let rule = JAVA_L34Rule::new();
        let smelly = r#"
String result = items.stream().collect(Collectors.joining(""));
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect joining(\"\")");
        assert_eq!(issues[0].rule_id, "JAVA_L34");
    }

    #[test]
    fn test_l34_allows_joining_with_separator() {
        let rule = JAVA_L34Rule::new();
        let clean = r#"
String result = items.stream().collect(Collectors.joining(", "));
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag joining with separator");
    }
}
