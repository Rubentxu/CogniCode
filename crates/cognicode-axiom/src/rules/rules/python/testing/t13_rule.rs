//! T13 — hardcoded temp file path
//!
//! Detects hardcoded temporary file paths in tests, which is not portable.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_T13"
    name: "Hardcoded temp file path"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Hardcoded temp file paths like '/tmp/file.txt' are not portable across systems. Use tempfile module or tempdir fixtures.",
    clean_code: Clear,
    impacts: [Maintainability: Medium],
    check: => {
        let mut issues = Vec::new();
        let src = ctx.source;

        // Find test methods
        let test_method_re = regex::Regex::new(r"def (test_\w+)\s*\(").unwrap();

        for cap in test_method_re.captures_iter(src) {
            if let Some(method_match) = cap.get(1) {
                let method_start = cap.get(0).unwrap().start();
                let method_name = method_match.as_str();

                // Find the method body
                let remaining = &src[method_start..];
                let body_end = remaining[2..]
                    .find("\ndef ")
                    .or_else(|| remaining[2..].find("\nclass "))
                    .unwrap_or(remaining.len() - 2);

                let method_body = &remaining[2..body_end];

                // Check for hardcoded temp paths
                let temp_path_re = regex::Regex::new(r#"['"]/tmp/"#).unwrap();

                if temp_path_re.is_match(method_body) {
                    let line_num = src[..method_start].lines().count() + 1;
                    issues.push(Issue::new(
                        "PY_T13",
                        &format!("Test method {} uses hardcoded temp file path", method_name),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Use tempfile.mkstemp() or pytest tmp_path fixture instead."
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
    fn test_t13_registered() {
        let rule = PY_T13Rule::new();
        assert_eq!(rule.id(), "PY_T13");
    }

    #[test]
    fn test_t13_detects_hardcoded_temp_path() {
        let rule = PY_T13Rule::new();
        let smelly = r##"
def test_something():
    with open("/tmp/test_data.txt", "w") as f:
        f.write("data")
    assert os.path.exists("/tmp/test_data.txt")
"##;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect hardcoded /tmp path");
        assert_eq!(issues[0].rule_id, "PY_T13");
    }

    #[test]
    fn test_t13_allows_tempfile_usage() {
        let rule = PY_T13Rule::new();
        let clean = r##"
def test_something(tmp_path):
    test_file = tmp_path / "test_data.txt"
    test_file.write_text("data")
    assert test_file.exists()
"##;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag proper tempfile usage");
    }
}
