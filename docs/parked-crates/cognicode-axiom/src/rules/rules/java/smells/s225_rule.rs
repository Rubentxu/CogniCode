//! S225 — Public method with too many parameters
//!
//! Detects methods with more than 5 parameters.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_S225"
    name: "Method has too many parameters (>5)"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Methods with many parameters are hard to call and maintain. Consider using a parameter object.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find public methods and count parameters
        let method_pattern = regex::Regex::new(r"public\s+\w+\s+(\w+)\s*\(([^)]*)\)").unwrap();

        for cap in method_pattern.captures_iter(source) {
            if let Some(method_name) = cap.get(1) {
                let params = cap.get(2).map(|m| m.as_str()).unwrap_or("");
                let param_count = if params.trim().is_empty() {
                    0
                } else {
                    params.split(',').count()
                };

                if param_count > 5 {
                    let line_num = source[..cap.get(0).unwrap().start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "JAVA_S225",
                        format!("Method '{}' has {} parameters (max 5)", method_name.as_str(), param_count),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Consider using a parameter object or breaking into smaller methods"
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
    fn test_s225_registered() {
        let rule = JAVA_S225Rule::new();
        assert_eq!(rule.id(), "JAVA_S225");
    }

    #[test]
    fn test_s225_detects_too_many_params() {
        let rule = JAVA_S225Rule::new();
        let smelly = r#"
public void createUser(String first, String last, String email, String phone, String address, String city) {
}
"#;
        let issues = with_java_context(smelly, "UserService.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect too many parameters");
        assert_eq!(issues[0].rule_id, "JAVA_S225");
    }

    #[test]
    fn test_s225_allows_few_params() {
        let rule = JAVA_S225Rule::new();
        let clean = r#"
public void createUser(String email, String password) {
}
"#;
        let issues = with_java_context(clean, "UserService.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag methods with <= 5 params");
    }
}
