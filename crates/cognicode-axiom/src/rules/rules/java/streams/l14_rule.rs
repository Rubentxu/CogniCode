//! L29 — Optional.orElse() with expensive computation
//!
//! Detects `.orElse()` with expensive computation (new object or method call).
use crate::rules::{CleanCodeAttribute, ImpactSeverity, SoftwareQuality, SoftwareQualityImpact};
use crate::{Category, Issue, Remediation, Rule, RuleContext, RuleEntry, Severity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_L29"
    name: "Optional.orElse() with expensive computation"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Using orElse() with a new object creation or method call evaluates the expression even when the Optional is present. Use orElseGet() to defer the computation.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect .orElse( followed by eager object creation or a method call.
        let or_else_pattern = regex::Regex::new(r"\.orElse\s*\(\s*(?:new\s+\w+|\w+\s*\()").unwrap();

        for cap in or_else_pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_L29",
                    "Optional.orElse() with expensive computation - use orElseGet()",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Use orElseGet() with a Supplier to defer the computation."
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
    fn test_l29_registered() {
        let rule = JAVA_L29Rule::new();
        assert_eq!(rule.id(), "JAVA_L29");
    }

    #[test]
    fn test_l29_detects_orElse_with_new() {
        let rule = JAVA_L29Rule::new();
        let smelly = r#"
String value = optional.orElse(new String("default"));
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect orElse with new");
        assert_eq!(issues[0].rule_id, "JAVA_L29");
    }

    #[test]
    fn test_l29_allows_orElseGet() {
        let rule = JAVA_L29Rule::new();
        let clean = r#"
String value = optional.orElseGet(() -> "default");
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag orElseGet()");
    }
}
