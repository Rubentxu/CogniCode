//! N6 — Too many methods in class (>20)
//!
//! Detects classes with too many methods, indicating potential God class anti-pattern.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_N6"
    name: "Class has too many methods (more than 20)"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Classes with more than 20 methods may be doing too much (God class anti-pattern). Consider splitting into smaller, focused classes.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find all class definitions and count their methods
        let class_pattern = regex::Regex::new(r"(?m)^class\s+(\w+)").unwrap();
        let method_pattern = regex::Regex::new(r"\n    def\s+\w+\s*\(").unwrap();

        for class_cap in class_pattern.captures_iter(source) {
            if let Some(class_name) = class_cap.get(1) {
                let class_start = class_cap.get(0).unwrap().start();

                // Find the end of the class (next class def or end of file)
                let remaining = &source[class_start..];
                let class_end = remaining[2..]
                    .find("\nclass ")
                    .unwrap_or(remaining.len() - 2);

                let class_body = &remaining[..class_end];

                // Count methods in the class body
                let method_count = method_pattern.find_iter(class_body).count();

                if method_count > 20 {
                    let line_num = source[..class_start].lines().count() + 1;
                    issues.push(Issue::new(
                        "PY_N6",
                        format!("Class '{}' has {} methods (recommended: max 20)", class_name.as_str(), method_count),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Consider splitting this class into smaller, focused classes"
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
    fn test_n6_registered() {
        let rule = PY_N6Rule::new();
        assert_eq!(rule.id(), "PY_N6");
    }

    #[test]
    fn test_n6_detects_too_many_methods() {
        let rule = PY_N6Rule::new();
        let smelly = r#"
class MyClass:
    def method1(self): pass
    def method2(self): pass
    def method3(self): pass
    def method4(self): pass
    def method5(self): pass
    def method6(self): pass
    def method7(self): pass
    def method8(self): pass
    def method9(self): pass
    def method10(self): pass
    def method11(self): pass
    def method12(self): pass
    def method13(self): pass
    def method14(self): pass
    def method15(self): pass
    def method16(self): pass
    def method17(self): pass
    def method18(self): pass
    def method19(self): pass
    def method20(self): pass
    def method21(self): pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect class with too many methods");
        assert_eq!(issues[0].rule_id, "PY_N6");
    }

    #[test]
    fn test_n6_allows_normal_class() {
        let rule = PY_N6Rule::new();
        let clean = r#"
class MyClass:
    def method1(self): pass
    def method2(self): pass
    def method3(self): pass
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag class with normal number of methods");
    }

    #[test]
    fn test_n6_allows_exactly_20_methods() {
        let rule = PY_N6Rule::new();
        let clean = r#"
class MyClass:
    def method1(self): pass
    def method2(self): pass
    def method3(self): pass
    def method4(self): pass
    def method5(self): pass
    def method6(self): pass
    def method7(self): pass
    def method8(self): pass
    def method9(self): pass
    def method10(self): pass
    def method11(self): pass
    def method12(self): pass
    def method13(self): pass
    def method14(self): pass
    def method15(self): pass
    def method16(self): pass
    def method17(self): pass
    def method18(self): pass
    def method19(self): pass
    def method20(self): pass
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag class with exactly 20 methods");
    }
}