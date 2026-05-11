//! L32 — forEach used to modify collection
//!
//! Detects `forEach()` used to modify the collection being iterated over.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_L32"
    name: "forEach used to modify collection"
    severity: Minor
    category: Bug
    language: "Java"
    params: {}

    explanation: "Using forEach() to modify the collection being iterated over can cause ConcurrentModificationException. Use a traditional for loop or collect modifications.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect forEach with .add( or .remove( inside
        let for_each_pattern = regex::Regex::new(r"\.forEach\s*\([^)]+\{[^}]*\.add\s*\(|forEach\s*\([^)]+\{[^}]*\.remove\s*\(").unwrap();

        for cap in for_each_pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_L32",
                    "forEach() used to modify collection - can cause ConcurrentModificationException",
                    Severity:: Minor,
                    Category::Bug,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Use a traditional for loop or collect modifications to a new collection."
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
    fn test_l32_registered() {
        let rule = JAVA_L32Rule::new();
        assert_eq!(rule.id(), "JAVA_L32");
    }

    #[test]
    fn test_l32_detects_forEach_with_add() {
        let rule = JAVA_L32Rule::new();
        let smelly = r#"
items.forEach(item -> {
    if (item.length() > 3) {
        result.add(item);
    }
});
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect forEach with .add()");
        assert_eq!(issues[0].rule_id, "JAVA_L32");
    }

    #[test]
    fn test_l32_allows_read_only_forEach() {
        let rule = JAVA_L32Rule::new();
        let clean = r#"
items.forEach(item -> System.out.println(item));
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag read-only forEach");
    }
}
