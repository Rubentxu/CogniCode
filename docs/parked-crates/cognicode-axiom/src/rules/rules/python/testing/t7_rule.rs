//! T7 — Test fixture too complex (>20 lines setup)
//!
//! Detects test fixtures that have too much setup code, indicating a need for better test design.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_T7"
    name: "Test fixture too complex"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Test fixtures with more than 20 lines of setup indicate the test may be doing too much or that fixtures should be refactored.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find test methods and count their setup lines
        let test_method_pattern = regex::Regex::new(r"def (test_\w+)\s*\(").unwrap();

        for cap in test_method_pattern.captures_iter(source) {
            if let Some(method_name) = cap.get(1) {
                let method_start = cap.get(0).unwrap().start();
                let method_name_str = method_name.as_str();

                // Find the method body
                let remaining = &source[method_start..];
                // Find next method or class to determine body end
                // Search in remaining[2..] to skip "de" from "def"
                let search_in = &remaining[2..];
                let body_end_rel = search_in
                    .find("\ndef ")
                    .or_else(|| search_in.find("\nclass "));

                let body_end = match body_end_rel {
                    Some(pos) => 2 + pos,  // Absolute position
                    None => remaining.len(), // No next method found, use full string
                };
                let method_body = &remaining[..body_end];

                // Count lines before first assert or statement
                let lines: Vec<&str> = method_body.lines().collect();
                let mut setup_lines = 0;
                let mut has_assert = false;

                for line in &lines {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        continue;
                    }
                    if trimmed.starts_with("assert") || trimmed.contains("self.assert") {
                        has_assert = true;
                        break;
                    }
                    setup_lines += 1;
                }

                if setup_lines > 20 {
                    let line_num = source[..method_start].lines().count() + 1;
                    issues.push(Issue::new(
                        "PY_T7",
                        format!("Test '{}' has {} lines of setup before assertions", method_name_str, setup_lines),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Consider extracting setup to fixtures or helper methods."
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
    use cognicode_core::infrastructure::parser::Language;

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
    fn test_t7_registered() {
        let rule = PY_T7Rule::new();
        assert_eq!(rule.id(), "PY_T7");
    }

    #[test]
    fn test_t7_detects_complex_fixture() {
        let rule = PY_T7Rule::new();
        let smelly = r#"
def test_something():
    x = 1
    x = 2
    x = 3
    x = 4
    x = 5
    x = 6
    x = 7
    x = 8
    x = 9
    x = 10
    x = 11
    x = 12
    x = 13
    x = 14
    x = 15
    x = 16
    x = 17
    x = 18
    x = 19
    x = 20
    x = 21
    x = 22
    assert x == 22
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect complex fixture >20 lines");
        assert_eq!(issues[0].rule_id, "PY_T7");
    }

    #[test]
    fn test_t7_allows_simple_test() {
        let rule = PY_T7Rule::new();
        let clean = r#"
def test_something():
    x = 1
    assert x == 1
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag simple test");
    }
}
