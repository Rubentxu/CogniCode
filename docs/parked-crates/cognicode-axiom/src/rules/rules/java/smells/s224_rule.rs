//! S224 — Boolean parameter in public method
//!
//! Detects boolean parameters in public method signatures (flag arguments).
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_S224"
    name: "Boolean parameter in public method (flag argument)"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Boolean parameters in public methods are flag arguments that make code harder to read. Consider splitting the method or using named constants for clarity.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Detect public methods with boolean parameters
        let public_method_pattern = regex::Regex::new(r"public\s+\w+\s+\w+\s*\([^)]*boolean\s+\w+[^)]*\)").unwrap();

        for cap in public_method_pattern.captures_iter(source) {
            if let Some(matched) = cap.get(0) {
                let start = matched.start();
                let line_num = source[..start].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_S224",
                    "Public method has boolean parameter - consider using named constants or splitting method",
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Consider splitting this method or using an enum constant instead of boolean."
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
    fn test_s224_registered() {
        let rule = JAVA_S224Rule::new();
        assert_eq!(rule.id(), "JAVA_S224");
    }

    #[test]
    fn test_s224_detects_boolean_param() {
        let rule = JAVA_S224Rule::new();
        let smelly = r#"
public void process(boolean recursive) {
    // ...
}
"#;
        let issues = with_java_context(smelly, "Test.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect boolean parameter in public method");
        assert_eq!(issues[0].rule_id, "JAVA_S224");
    }

    #[test]
    fn test_s224_allows_private_method() {
        let rule = JAVA_S224Rule::new();
        let clean = r#"
private void process(boolean recursive) {
    // ...
}
"#;
        let issues = with_java_context(clean, "Test.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag boolean parameter in private method");
    }
}
