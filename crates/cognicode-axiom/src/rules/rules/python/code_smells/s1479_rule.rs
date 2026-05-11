//! S1479 — Too many methods in class (>20)
//!
//! Detects classes with too many methods.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_S1479"
    name: "Class should not have too many methods"
    severity: Major
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Classes with more than 20 methods are hard to understand and maintain. Consider splitting into smaller classes.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let threshold = 20;

        let lines: Vec<&str> = ctx.source.lines().collect();

        for (line_num, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("class ") && trimmed.ends_with(':') {
                let start_line = line_num;
                let mut indent_level = line.len() - line.trim_start().len();
                let mut method_count = 0;

                for (check_line_num, check_line) in lines.iter().enumerate().skip(line_num + 1) {
                    let check_trimmed = check_line.trim();
                    if check_trimmed.is_empty() || check_trimmed.starts_with('#') {
                        continue;
                    }
                    let check_indent = check_line.len() - check_line.trim_start().len();
                    if check_indent <= indent_level && !check_trimmed.is_empty() {
                        break;
                    }

                    // Count method definitions (not nested functions)
                    if check_trimmed.starts_with("def ") && !check_trimmed.contains("    ") {
                        method_count += 1;
                    }
                }

                if method_count > threshold {
                    issues.push(Issue::new(
                        "PY_S1479",
                        format!("Class at line {} has {} methods (threshold: {})", start_line + 1, method_count, threshold),
                        Severity::Major,
                        Category::CodeSmell,
                        ctx.file_path,
                        start_line + 1,
                    ).with_remediation(Remediation::quick(
                        "Consider splitting this class into smaller, more focused classes."
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
    fn test_s1479_registered() {
        let rule = PY_S1479Rule::new();
        assert_eq!(rule.id(), "PY_S1479");
    }

    #[test]
    fn test_s1479_detects_too_many_methods() {
        let rule = PY_S1479Rule::new();
        let smelly = r#"
class Config:
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
        assert_eq!(issues[0].rule_id, "PY_S1479");
    }

    #[test]
    fn test_s1479_allows_normal_class() {
        let rule = PY_S1479Rule::new();
        let clean = r#"
class Small:
    def method1(self): pass
    def method2(self): pass
    def method3(self): pass
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag class with <= 20 methods");
    }
}
