//! N5 — Commented-out code
//!
//! Detects commented lines that look like code rather than descriptions.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use cognicode_core::infrastructure::parser::Language;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_N5"
    name: "Commented-out code should be removed"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Commented lines that look like code (containing =, def, import, class, return, if, for, while) should be removed instead of commented.",
    clean_code: Clear,
    impacts: [Maintainability: Low],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Look for comment lines that contain code-like patterns
        let code_patterns = [
            r#"#.*\bdef\s+"#,
            r#"#.*\bclass\s+"#,
            r#"#.*\bimport\s+"#,
            r#"#.*\bfrom\s+\w+\s+import"#,
            r#"#.*[a-zA-Z_]\s*=\s*"#,
            r#"#.*\breturn\s+"#,
            r#"#.*\bif\s+"#,
            r#"#.*\bfor\s+"#,
            r#"#.*\bwhile\s+"#,
            r#"#.*\bprint\s*\("#,
            r#"#.*\bassert\s+"#,
        ];

        for (line_num, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                for pattern in &code_patterns {
                    let re = regex::Regex::new(pattern).unwrap();
                    if re.is_match(trimmed) {
                        issues.push(Issue::new(
                            "PY_N5",
                            format!("Commented-out code detected: {}", &trimmed[..trimmed.len().min(50)]),
                            Severity::Minor,
                            Category::CodeSmell,
                            ctx.file_path,
                            line_num + 1,
                        ).with_remediation(Remediation::quick(
                            "Remove commented-out code or replace with meaningful comment"
                        )));
                        break;
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
    fn test_n5_registered() {
        let rule = PY_N5Rule::new();
        assert_eq!(rule.id(), "PY_N5");
    }

    #[test]
    fn test_n5_detects_commented_def() {
        let rule = PY_N5Rule::new();
        let smelly = r#"
# def old_function():
#     pass
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect commented def");
        assert_eq!(issues[0].rule_id, "PY_N5");
    }

    #[test]
    fn test_n5_detects_commented_assignment() {
        let rule = PY_N5Rule::new();
        let smelly = r#"
# x = 5
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect commented assignment");
    }

    #[test]
    fn test_n5_allows_meaningful_comment() {
        let rule = PY_N5Rule::new();
        let clean = r#"
# This is a helper function that does something important
def func():
    pass
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag meaningful comments");
    }

    #[test]
    fn test_n5_allows_import_comment() {
        let rule = PY_N5Rule::new();
        let clean = r#"
# TODO: add more imports
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag TODO comments");
    }
}