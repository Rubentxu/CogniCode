//! S101 — Class naming (PascalCase)
//!
//! Detects classes not following PascalCase naming convention.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S101"
    name: "Class names should use PascalCase"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Class names should follow the PascalCase naming convention (each word capitalized).",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let pascal_case_pattern = regex::Regex::new(r"^[A-Z][a-zA-Z0-9]*$").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("class ") && trimmed.ends_with(':') {
                if let Some(start) = trimmed.find("class ") {
                    if let Some(paren) = trimmed.find('(') {
                        let class_name = &trimmed[start + 6..paren].trim();
                    } else if let Some(colon) = trimmed.find(':') {
                        let class_name = &trimmed[start + 6..colon].trim();
                        if !pascal_case_pattern.is_match(class_name) {
                            issues.push(Issue::new(
                                "PY_S101",
                                format!("Class '{}' should use PascalCase naming", class_name),
                                Severity::Minor,
                                Category::CodeSmell,
                                ctx.file_path,
                                line_num + 1,
                            ).with_remediation(Remediation::quick(
                                "Rename class to use PascalCase (e.g., 'MyClass' instead of 'myClass')."
                            )));
                        }
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
    fn test_s101_registered() {
        let rule = PY_S101Rule::new();
        assert_eq!(rule.id(), "PY_S101");
    }

    #[test]
    fn test_s101_detects_snake_case_class() {
        let rule = PY_S101Rule::new();
        let smelly = r#"
class my_class:
    pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect snake_case class name");
        assert_eq!(issues[0].rule_id, "PY_S101");
    }

    #[test]
    fn test_s101_allows_pascal_case() {
        let rule = PY_S101Rule::new();
        let clean = r#"
class MyClass:
    pass
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag PascalCase class name");
    }
}
