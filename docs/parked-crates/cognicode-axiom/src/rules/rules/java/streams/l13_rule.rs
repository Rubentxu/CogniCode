//! L28 — Collectors.toList() should be toUnmodifiableList()
//!
//! Detects `Collectors.toList()` and suggests using `Collectors.toUnmodifiableList()`.
use crate::rules::{CleanCodeAttribute, ImpactSeverity, SoftwareQuality, SoftwareQualityImpact};
use crate::{Category, Issue, Remediation, Rule, RuleContext, RuleEntry, Severity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_L28"
    name: "Collectors.toList() returns modifiable list"
    severity: Info
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Collectors.toList() returns a mutable ArrayList. Use Collectors.toUnmodifiableList() (Java 9+) for an immutable list that better expresses intent.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect Collectors.toList()
        let pattern = regex::Regex::new(r"Collectors\s*\.\s*toList\s*\(\s*\)").unwrap();

        for cap in pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_L28",
                    "Collectors.toList() returns mutable list - consider toUnmodifiableList()",
                    Severity::Info,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Consider using Collectors.toUnmodifiableList() for immutable lists."
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
    fn test_l28_registered() {
        let rule = JAVA_L28Rule::new();
        assert_eq!(rule.id(), "JAVA_L28");
    }

    #[test]
    fn test_l28_detects_toList() {
        let rule = JAVA_L28Rule::new();
        let smelly = r#"
List<String> result = items.stream()
    .filter(x -> x.length() > 3)
    .collect(Collectors.toList());
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect Collectors.toList()");
        assert_eq!(issues[0].rule_id, "JAVA_L28");
    }

    #[test]
    fn test_l28_allows_toSet() {
        let rule = JAVA_L28Rule::new();
        let clean = r#"
Set<String> result = items.stream()
    .filter(x -> x.length() > 3)
    .collect(Collectors.toSet());
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag Collectors.toSet()");
    }
}
