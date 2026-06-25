//! S229 — finalize() override in non-final class
//!
//! Detects overriding finalize() in classes not declared final.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "JAVA_S229"
    name: "finalize() override without class being final"
    severity: Minor
    category: CodeSmell
    language: "Java"
    params: {}

    explanation: "Overriding finalize() is deprecated and error-prone. If a class is not final, subclasses may override finalize() incorrectly.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find finalize() method
        let finalize_pattern = regex::Regex::new(r"protected\s+void\s+finalize\s*\(\s*\)").unwrap();

        for cap in finalize_pattern.find_iter(source) {
            // Find enclosing class - search backwards for "class" keyword
            let before = &source[..cap.start()];
            let class_pos = before.rfind("\nclass").or_else(|| before.rfind("class")).unwrap_or(0);

            // Check if class is final by looking at preceding lines
            let class_line_start = source[..class_pos].rfind('\n').map(|p| p + 1).unwrap_or(0);
            let class_line = &source[class_line_start..cap.start()];

            // Check if 'final' appears before 'class' on the same line
            let is_final = class_line.contains("final class") || class_line.contains("final\nclass");

            if !is_final {
                let line_num = source[..cap.start()].lines().count() + 1;
                issues.push(Issue::new(
                    "JAVA_S229",
                    "finalize() override in non-final class is error-prone".to_string(),
                    Severity::Minor,
                    Category::CodeSmell,
                    ctx.file_path,
                    line_num,
                ).with_remediation(Remediation::quick(
                    "Remove finalize() and use try-with-resources or explicit cleanup methods instead"
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
    fn test_s229_registered() {
        let rule = JAVA_S229Rule::new();
        assert_eq!(rule.id(), "JAVA_S229");
    }

    #[test]
    fn test_s229_detects_finalize_in_non_final_class() {
        let rule = JAVA_S229Rule::new();
        let smelly = r#"
class MyResource {
    protected void finalize() throws Throwable {
        cleanup();
    }
}
"#;
        let issues = with_java_context(smelly, "MyResource.java", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect finalize in non-final class");
        assert_eq!(issues[0].rule_id, "JAVA_S229");
    }

    #[test]
    fn test_s229_allows_finalize_in_final_class() {
        let rule = JAVA_S229Rule::new();
        let clean = r#"
final class MyResource {
    protected void finalize() throws Throwable {
        cleanup();
    }
}
"#;
        let issues = with_java_context(clean, "MyResource.java", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag finalize in final class");
    }
}
