//! SP2 — Component without interface
//!
//! Detects @Component classes without an interface.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_SP2"
    name: "@Component without interface"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "@Component classes should implement an interface for better testability and loose coupling.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find @Component class declarations
        let component_pattern = regex::Regex::new(r"@Component\s+(?:public\s+)?class\s+(\w+)").unwrap();

        for cap in component_pattern.captures_iter(source) {
            if let Some(class_name) = cap.get(1) {
                let class_str = class_name.as_str();
                let class_start = cap.get(0).unwrap().end();

                // Look for "implements" keyword after class declaration
                let after_class = &source[class_start..class_start + 500.min(source.len() - class_start)];
                let has_implements = after_class.trim_start().starts_with("implements");

                if !has_implements {
                    let line_num = source[..cap.get(0).unwrap().start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "JAVA_SP2",
                        format!("@Component class '{}' should implement an interface", class_str),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Create an interface and have the component implement it"
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
    fn test_sp2_registered() {
        let rule = JAVA_SP2Rule::new();
        assert_eq!(rule.id(), "JAVA_SP2");
    }

    #[test]
    fn test_sp2_detects_component_without_interface() {
        let rule = JAVA_SP2Rule::new();
        let smelly = r#"
@Component
public class MyService {
    public void doSomething() {}
}
"#;
        let issues = with_java_context(smelly, "MyService.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect @Component without interface");
        assert_eq!(issues[0].rule_id, "JAVA_SP2");
    }

    #[test]
    fn test_sp2_allows_component_with_interface() {
        let rule = JAVA_SP2Rule::new();
        let clean = r#"
@Component
public class MyService implements MyInterface {
    public void doSomething() {}
}
"#;
        let issues = with_java_context(clean, "MyService.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag @Component with interface");
    }
}
