//! SP7 — Value injection for complex config
//!
//! Detects @Value injection for complex configuration objects.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_SP7"
    name: "@Value injection for complex configuration"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Using @Value for complex objects (nested properties, maps, lists) is error-prone. Use @ConfigurationProperties instead for type-safe configuration binding.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find @Value with complex expressions (nested ${...})
        let value_pattern = regex::Regex::new(r#"@Value\s*\(\s*["']?\$\{[^}]+\$\{[^}]+\}"#).unwrap();

        for cap in value_pattern.find_iter(source) {
            let line_num = source[..cap.start()].lines().count() + 1;
            issues.push(Issue::new(
                "JAVA_SP7",
                format!("@Value with nested placeholders may indicate complex config: {}", cap.as_str()),
                Severity::Minor,
                Category::CodeSmell,
                ctx.file_path,
                line_num,
            ).with_remediation(Remediation::quick(
                "Consider using @ConfigurationProperties for type-safe configuration"
            )));
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
    fn test_sp7_registered() {
        let rule = JAVA_SP7Rule::new();
        assert_eq!(rule.id(), "JAVA_SP7");
    }

    #[test]
    fn test_sp7_detects_complex_value() {
        let rule = JAVA_SP7Rule::new();
        let smelly = r#"
@Component
public class MyService {
    @Value("${app.config.${env.property}}")
    private String config;
}
"#;
        let issues = with_java_context(smelly, "MyService.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect complex @Value expression");
        assert_eq!(issues[0].rule_id, "JAVA_SP7");
    }

    #[test]
    fn test_sp7_allows_simple_value() {
        let rule = JAVA_SP7Rule::new();
        let clean = r#"
@Component
public class MyService {
    @Value("${app.name}")
    private String name;
}
"#;
        let issues = with_java_context(clean, "MyService.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag simple @Value");
    }
}
