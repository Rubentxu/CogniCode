//! S227 — for(;;) instead of while(true)
//!
//! Detects infinite loops written as for(;;) instead of while(true).
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_S227"
    name: "for(;;) used instead of while(true)"
    severity: Info
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "The for(;;) syntax for infinite loops is less readable than while(true). Use while(true) for better clarity.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect for(;;) or for (;;)
        let pattern = regex::Regex::new(r"for\s*\(\s*;\s*;\s*\)").unwrap();

        for cap in pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_S227",
                    "for(;;) used - consider while(true) for better readability",
                    Severity::Info,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Replace for(;;) with while(true) for better readability."
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
    fn test_s227_registered() {
        let rule = JAVA_S227Rule::new();
        assert_eq!(rule.id(), "JAVA_S227");
    }

    #[test]
    fn test_s227_detects_for_while() {
        let rule = JAVA_S227Rule::new();
        let smelly = r#"
for (;;) {
    System.out.println("looping");
}
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect for(;;)");
        assert_eq!(issues[0].rule_id, "JAVA_S227");
    }

    #[test]
    fn test_s227_allows_while_true() {
        let rule = JAVA_S227Rule::new();
        let clean = r#"
while (true) {
    System.out.println("looping");
}
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag while(true)");
    }
}
