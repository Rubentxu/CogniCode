//! S1820 — Too many fields in class (>15)
//!
//! Detects classes with too many fields.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1820"
    name: "Class should not have too many fields"
    severity: Major
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Classes with more than 15 fields are hard to understand and maintain. Consider splitting into smaller classes or using composition.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let threshold = 15;

        for (line_num, line) in ctx.source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("class ") && trimmed.ends_with(':') {
                let start_line = line_num;
                let mut indent_level = line.len() - line.trim_start().len();
                let mut field_count = 0;
                let mut found_init = false;

                for (check_line_num, check_line) in ctx.source.lines().enumerate().skip(line_num + 1) {
                    let check_trimmed = check_line.trim();
                    if check_trimmed.is_empty() {
                        continue;
                    }
                    let check_indent = check_line.len() - check_line.trim_start().len();
                    if check_indent <= indent_level && !check_trimmed.is_empty() {
                        break;
                    }
                    // Count self.x = assignments as fields
                    if check_trimmed.starts_with("self.") && check_trimmed.contains("=") {
                        field_count += 1;
                    }
                }

                if field_count > threshold {
                    issues.push(Issue::new(
                        "PY_S1820",
                        format!("Class at line {} has {} fields (threshold: {})", start_line + 1, field_count, threshold),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        start_line + 1,
                    ).with_remediation(Remediation::quick(
                        "Consider splitting this class into smaller classes or using composition."
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
    fn test_s1820_registered() {
        let rule = PY_S1820Rule::new();
        assert_eq!(rule.id(), "PY_S1820");
    }

    #[test]
    fn test_s1820_detects_too_many_fields() {
        let rule = PY_S1820Rule::new();
        let smelly = r#"
class User:
    def __init__(self):
        self.name = name
        self.age = age
        self.email = email
        self.phone = phone
        self.address = address
        self.city = city
        self.country = country
        self.zipcode = zipcode
        self.username = username
        self.password = password
        self.created_at = created_at
        self.updated_at = updated_at
        self.is_active = is_active
        self.is_admin = is_admin
        self.last_login = last_login
        self.preferences = preferences
        self.settings = settings
        self.profile = profile
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect too many fields");
        assert_eq!(issues[0].rule_id, "PY_S1820");
    }

    #[test]
    fn test_s1820_allows_normal_fields() {
        let rule = PY_S1820Rule::new();
        let clean = r#"
class Point:
    def __init__(self):
        self.x = x
        self.y = y
        self.z = z
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag class with <= 15 fields");
    }
}
