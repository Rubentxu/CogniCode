//! L26 — Optional.get() without isPresent() check
//!
//! Detects `.get()` on Optional without prior `.isPresent()` check.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_L26"
    name: "Optional.get() without isPresent() check"
    severity: Major
    category: Bug
    language: "Java"
    params: {}

    explanation: "Calling get() on an Optional without first checking isPresent() can throw NoSuchElementException if the Optional is empty.",
    clean_code: Clear,
    impacts: [Reliability: High],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect .get() followed by isPresent() check (problematic pattern)
        // We look for .get() used in a way that suggests no prior check
        let get_pattern = regex::Regex::new(r"\.get\s*\(\s*\)").unwrap();

        for cap in get_pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;

                // Look backwards from get() to see if there's an isPresent() or orElse check
                let before_get = &source[..start];
                let has_check_before = before_get.rfind(".isPresent()").is_some()
                    || before_get.rfind(".orElse").is_some()
                    || before_get.rfind(".orElseGet").is_some()
                    || before_get.rfind(".orElseThrow").is_some();

                if !has_check_before {
                    issues.push(Issue::new(
                        "JAVA_L26",
                        "Optional.get() called without isPresent() check - may throw NoSuchElementException",
                        Severity::Major,
                        Category::Bug,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Check isPresent() before calling get(), or use orElse/orElseGet/orElseThrow."
                    )));
                }
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
    fn test_l26_registered() {
        let rule = JAVA_L26Rule::new();
        assert_eq!(rule.id(), "JAVA_L26");
    }

    #[test]
    fn test_l26_detects_get_without_check() {
        let rule = JAVA_L26Rule::new();
        let smelly = r#"
String value = optional.get();
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect get() without isPresent()");
        assert_eq!(issues[0].rule_id, "JAVA_L26");
    }

    #[test]
    fn test_l26_allows_get_with_check() {
        let rule = JAVA_L26Rule::new();
        let clean = r#"
if (optional.isPresent()) {
    String value = optional.get();
}
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag get() after isPresent() check");
    }
}
