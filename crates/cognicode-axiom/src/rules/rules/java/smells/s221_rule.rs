//! S221 — Method returns null
//!
//! Detects return null statements in non-void methods, suggesting Optional instead.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_S221"
    name: "Method returns null instead of Optional"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Returning null from a method forces callers to handle null checks. Consider returning Optional instead to make the absence of a value explicit.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect return null; in methods
        // We look for methods that have a return type (not void)
        let return_null_pattern = regex::Regex::new(r"return\s+null\s*;").unwrap();

        for cap in return_null_pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;

                // Look backwards to find the method signature
                let before_return = &source[..start];
                let last_method_start = before_return.rfind("private ")
                    .or_else(|| before_return.rfind("public "))
                    .or_else(|| before_return.rfind("protected "))
                    .or_else(|| before_return.rfind(""));

                if let Some(method_pos) = last_method_start {
                    let method_fragment = &before_return[method_pos..];

                    // Check if this is a non-void method
                    let is_void = method_fragment.contains("void ")
                        || method_fragment.contains("void(");

                    if !is_void {
                        issues.push(Issue::new(
                            "JAVA_S221",
                            "Method returns null - consider returning Optional instead",
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            line_num,
                        ).with_remediation(Remediation::quick(
                            "Consider changing return type to Optional<T> instead of returning null."
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
    fn test_s221_registered() {
        let rule = JAVA_S221Rule::new();
        assert_eq!(rule.id(), "JAVA_S221");
    }

    #[test]
    fn test_s221_detects_return_null() {
        let rule = JAVA_S221Rule::new();
        let smelly = r#"
public String findById(Long id) {
    if (id == null) {
        return null;
    }
    return "found";
}
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect return null");
        assert_eq!(issues[0].rule_id, "JAVA_S221");
    }

    #[test]
    fn test_s221_allows_void_method() {
        let rule = JAVA_S221Rule::new();
        let clean = r#"
public void doSomething() {
    if (condition) {
        return;
    }
    System.out.println("done");
}
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag return in void method");
    }
}
