//! P10 — Class-level attribute instead of instance
//!
//! Detects class-level attributes that should be instance attributes.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_P10"
    name: "Avoid mutable class-level attributes"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Mutable class-level attributes are shared across all instances. Use instance attributes for mutable state.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        // Pattern to detect class-level mutable default assignments (list or dict)
        let list_pattern = regex::Regex::new(r"^\s*\w+\s*=\s*\[\s*\]").unwrap();
        let dict_pattern = regex::Regex::new(r"^\s*\w+\s*=\s*\{\s*\}").unwrap();

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            // Skip lines that start with keywords that wouldn't be class attributes
            if trimmed.starts_with("def ")
                || trimmed.starts_with("if ")
                || trimmed.starts_with("for ")
                || trimmed.starts_with("while ")
                || trimmed.starts_with("try ")
                || trimmed.starts_with("except ")
                || trimmed.starts_with("finally ")
                || trimmed.starts_with("with ")
                || trimmed.starts_with("import ")
                || trimmed.starts_with("from ")
                || trimmed.starts_with("#")
                || trimmed.is_empty()
            {
                continue;
            }

            // Check for class definition nearby and the attribute pattern
            if list_pattern.is_match(trimmed) || dict_pattern.is_match(trimmed) {
                // Check if we're inside a class context by looking for class definition before
                let source_before = ctx.source.lines().take(line_num + 1).collect::<Vec<_>>().join("\n");
                if source_before.contains("class ") && !trimmed.contains("self.") {
                    issues.push(Issue::new(
                        "PY_P10",
                        format!("Mutable class-level attribute detected at line {}", line_num + 1),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num + 1,
                    ).with_remediation(Remediation::quick(
                        "Move mutable default values to __init__ as instance attributes."
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
    fn test_p10_registered() {
        let rule = PY_P10Rule::new();
        assert_eq!(rule.id(), "PY_P10");
    }

    #[test]
    fn test_p10_detects_class_level_list() {
        let rule = PY_P10Rule::new();
        let smelly = r#"
class MyClass:
    items = []
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect class-level list");
        assert_eq!(issues[0].rule_id, "PY_P10");
    }

    #[test]
    fn test_p10_detects_class_level_dict() {
        let rule = PY_P10Rule::new();
        let smelly = r#"
class MyClass:
    data = {}
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect class-level dict");
    }

    #[test]
    fn test_p10_allows_instance_attribute() {
        let rule = PY_P10Rule::new();
        let clean = r#"
class MyClass:
    def __init__(self):
        self.items = []
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag instance attributes in __init__");
    }
}
