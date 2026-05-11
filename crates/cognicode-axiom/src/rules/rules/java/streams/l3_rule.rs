//! L18 — Stream.findFirst().isPresent() suggestion
//!
//! Detects `.findFirst().isPresent()` and suggests using `findFirst().ifPresent()` instead.
use crate::rules::{CleanCodeAttribute, ImpactSeverity, SoftwareQuality, SoftwareQualityImpact};
use crate::{Category, Issue, Remediation, Rule, RuleContext, RuleEntry, Severity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_L18"
    name: "findFirst().isPresent() should be findFirst().ifPresent()"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Using findFirst().isPresent() followed by get() is a code smell. Consider using findFirst().ifPresent() for a more idiomatic approach.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect chained .findFirst().isPresent() and the common two-step form:
        // Optional<T> first = stream.findFirst(); if (first.isPresent()) { ... }
        let pattern = regex::Regex::new(r"\.findFirst\s*\(\s*\)\s*\.\s*isPresent\s*\(").unwrap();

        for cap in pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_L18",
                    "findFirst().isPresent() - consider using ifPresent() instead",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Use findFirst().ifPresent() for a more idiomatic Optional handling."
                )));
            }
        }

        let assigned_pattern = regex::Regex::new(
            r"(?s)(?:Optional(?:\s*<[^;=]+>)?|var)\s+(\w+)\s*=\s*[^;]*\.findFirst\s*\(\s*\)\s*;",
        ).unwrap();

        for cap in assigned_pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let var_name = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
                let after_assignment = &source[matched.end()..];
                let is_present_pattern = regex::Regex::new(&format!(
                    r"\b{}\s*\.\s*isPresent\s*\(",
                    regex::escape(var_name)
                )).unwrap();

                if !is_present_pattern.is_match(after_assignment) {
                    continue;
                }

                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_L18",
                    "findFirst().isPresent() - consider using ifPresent() instead",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Use findFirst().ifPresent() for a more idiomatic Optional handling."
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
    fn test_l18_registered() {
        let rule = JAVA_L18Rule::new();
        assert_eq!(rule.id(), "JAVA_L18");
    }

    #[test]
    fn test_l18_detects_findfirst_ispresent() {
        let rule = JAVA_L18Rule::new();
        let smelly = r#"
Optional<String> first = items.stream().findFirst();
if (first.isPresent()) {
    String value = first.get();
}
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect findFirst().isPresent()");
        assert_eq!(issues[0].rule_id, "JAVA_L18");
    }

    #[test]
    fn test_l18_allows_ifpresent() {
        let rule = JAVA_L18Rule::new();
        let clean = r#"
items.stream().findFirst().ifPresent(value -> {
    System.out.println(value);
});
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag ifPresent() usage");
    }
}
