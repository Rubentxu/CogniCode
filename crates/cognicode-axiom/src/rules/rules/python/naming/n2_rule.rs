//! N2 — Class naming (PascalCase)
//!
//! Detects class definitions that don't follow PascalCase naming convention.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_N2"
    name: "Class naming should use PascalCase"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Class names should be PascalCase (each word capitalized, no underscores). Detected class names not starting with uppercase.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find all class definitions
        let class_pattern = regex::Regex::new(r"(?m)^class\s+([a-z_][a-zA-Z_]*)\s*[(:]").unwrap();

        for cap in class_pattern.captures_iter(source) {
            if let Some(class_name) = cap.get(1) {
                let class_name_str = class_name.as_str();
                // Check if name starts with lowercase (not PascalCase)
                if class_name_str.chars().next().map(|c| c.is_lowercase()).unwrap_or(false) {
                    let line_num = source[..class_name.start()].lines().count() + 1;
                    issues.push(Issue::new(
                        "PY_N2",
                        format!("Class '{}' should use PascalCase naming", class_name_str),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Rename class to use PascalCase: capitalize first letter of each word"
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

    fn with_python_context<F, R>(source: &str, file_path: &str, f: F) -> R
    where
        F: FnOnce(&RuleContext) -> R,
    {
        let ts_language = Language::Python.to_ts_language();
        let mut parser = TsParser::new();
        parser.set_language(&ts_language).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let graph = CallGraph::new();
        let metrics = FileMetrics::new();

        let ctx = RuleContext {
            tree: &tree,
            source,
            file_path: Path::new(file_path),
            language: &Language::Python,
            graph: &graph,
            metrics: &metrics,
        };

        f(&ctx)
    }

    #[test]
    fn test_n2_registered() {
        let rule = PY_N2Rule::new();
        assert_eq!(rule.id(), "PY_N2");
    }

    #[test]
    fn test_n2_detects_lowercase_class() {
        let rule = PY_N2Rule::new();
        let smelly = r#"
class my_class:
    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect lowercase class name");
        assert_eq!(issues[0].rule_id, "PY_N2");
    }

    #[test]
    fn test_n2_detects_snake_case_class() {
        let rule = PY_N2Rule::new();
        let smelly = r#"
class my_class_name:
    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect snake_case class name");
    }

    #[test]
    fn test_n2_allows_pascal_case() {
        let rule = PY_N2Rule::new();
        let clean = r#"
class MyClass:
    pass
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag PascalCase class names");
    }

    #[test]
    fn test_n2_allows_acronym() {
        let rule = PY_N2Rule::new();
        let clean = r#"
class XMLParser:
    pass
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should allow class names starting with uppercase acronym");
    }
}