//! S223 — Boolean parameter in public method
//!
//! Detects boolean parameters which make code harder to read.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_S223"
    name: "Boolean parameter in public method"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Boolean parameters make method calls like process(true, false) unreadable. Use named methods or enums instead.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find public methods with boolean parameters
        let method_re = regex::Regex::new(r"public\s+\w+\s+(\w+)\s*\(([^)]*)\)").unwrap();

        for cap in method_re.captures_iter(source) {
            if let Some(method_name) = cap.get(1) {
                if let Some(params) = cap.get(2) {
                    // Check if params contain 'boolean'
                    if params.as_str().contains("boolean") {
                        let line_num = source[..cap.get(0).unwrap().start()].lines().count() + 1;

                        issues.push(Issue::new(
                            "JAVA_S223",
                            format!("Method '{}' has a boolean parameter", method_name.as_str()),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            line_num,
                        ).with_remediation(Remediation::quick(
                            "Use two named methods or an enum parameter instead of boolean"
                        )));
                    }
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
    use std::path::Path;
    use tree_sitter::Parser as TsParser;

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
    fn test_s223_registered() {
        let rule = JAVA_S223Rule::new();
        assert_eq!(rule.id(), "JAVA_S223");
    }

    #[test]
    fn test_s223_detects_boolean_param() {
        let rule = JAVA_S223Rule::new();
        let smelly = r#"
public void process(boolean recursive) {
}
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect boolean parameter");
        assert_eq!(issues[0].rule_id, "JAVA_S223");
    }

    #[test]
    fn test_s223_allows_non_boolean_param() {
        let rule = JAVA_S223Rule::new();
        let clean = r#"
public void process(Mode mode) {
}
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag enum parameter");
    }
}
