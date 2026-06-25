//! N7 — Too many fields in class (>15)
//!
//! Detects classes with too many instance fields, indicating potential God class anti-pattern.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_N7"
    name: "Class has too many fields (more than 15)"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Classes with more than 15 fields may have too many responsibilities. Consider splitting into smaller classes or using composition.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find all class definitions and count their __init__ fields
        let class_pattern = regex::Regex::new(r"(?m)^class\s+(\w+)").unwrap();

        for class_cap in class_pattern.captures_iter(source) {
            if let Some(class_name) = class_cap.get(1) {
                let class_start = class_cap.get(0).unwrap().start();

                // Find the end of the class (next class def or end of file)
                let remaining = &source[class_start..];
                let class_end = remaining[2..]
                    .find("\nclass ")
                    .unwrap_or(remaining.len() - 2);

                let class_body = &remaining[..class_end];

                // Find __init__ method
                let init_pattern = regex::Regex::new(r"def __init__\s*\([^)]*\):").unwrap();
                if let Some(init_cap) = init_pattern.captures(class_body) {
                    let init_start = init_cap.get(0).unwrap().start();

                    // Find the end of __init__ (next def or end of class)
                    let init_body = &class_body[init_start..];
                    let init_end = init_body[2..]
                        .find("\n    def ")
                        .unwrap_or(init_body.len() - 2);

                    let init_content = &init_body[..init_end];

                    // Count self.x = ... assignments in __init__
                    let field_pattern = regex::Regex::new(r"self\.([a-zA-Z_][a-zA-Z0-9_]*)\s*=").unwrap();
                    let field_count = field_pattern.find_iter(init_content).count();

                    if field_count > 15 {
                        let line_num = source[..class_start].lines().count() + 1;
                        issues.push(Issue::new(
                            "PY_N7",
                            format!("Class '{}' has {} fields (recommended: max 15)", class_name.as_str(), field_count),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            line_num,
                        ).with_remediation(Remediation::quick(
                            "Consider splitting this class or grouping related fields into nested objects"
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
    fn test_n7_registered() {
        let rule = PY_N7Rule::new();
        assert_eq!(rule.id(), "PY_N7");
    }

    #[test]
    fn test_n7_detects_too_many_fields() {
        let rule = PY_N7Rule::new();
        let smelly = r#"
class MyClass:
    def __init__(self):
        self.field1 = 1
        self.field2 = 2
        self.field3 = 3
        self.field4 = 4
        self.field5 = 5
        self.field6 = 6
        self.field7 = 7
        self.field8 = 8
        self.field9 = 9
        self.field10 = 10
        self.field11 = 11
        self.field12 = 12
        self.field13 = 13
        self.field14 = 14
        self.field15 = 15
        self.field16 = 16
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect class with too many fields");
        assert_eq!(issues[0].rule_id, "PY_N7");
    }

    #[test]
    fn test_n7_allows_normal_class() {
        let rule = PY_N7Rule::new();
        let clean = r#"
class MyClass:
    def __init__(self):
        self.field1 = 1
        self.field2 = 2
        self.field3 = 3
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag class with normal number of fields");
    }

    #[test]
    fn test_n7_allows_exactly_15_fields() {
        let rule = PY_N7Rule::new();
        let clean = r#"
class MyClass:
    def __init__(self):
        self.field1 = 1
        self.field2 = 2
        self.field3 = 3
        self.field4 = 4
        self.field5 = 5
        self.field6 = 6
        self.field7 = 7
        self.field8 = 8
        self.field9 = 9
        self.field10 = 10
        self.field11 = 11
        self.field12 = 12
        self.field13 = 13
        self.field14 = 14
        self.field15 = 15
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag class with exactly 15 fields");
    }
}