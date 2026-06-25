//! T14 — missing cleanup for temp/monkeypatch resources
//!
//! Detects tests that create temp files or monkeypatch without proper cleanup.
use crate::{Severity, Category, Issue, Remediation, Rule, RuleContext, RuleEntry};
use crate::rules::{CleanCodeAttribute, SoftwareQuality, SoftwareQualityImpact, ImpactSeverity};
use cognicode_macros::declare_rule;
use std::path::Path;
use tree_sitter::Parser as TsParser;

declare_rule! {
    id: "PY_T14"
    name: "Missing cleanup for temp/monkeypatch resources"
    severity: Minor
    category: CodeSmell
    language: "Python"
    params: {}

    explanation: "Tests that create temporary files or use monkeypatching should clean up resources using try/finally, addCleanup, or fixture teardown.",
    clean_code: Clear,
    impacts: [Reliability: Medium],
    check: => {
        let mut issues = Vec::new();
        let source = ctx.source;

        // Find test methods
        let test_method_pattern = regex::Regex::new(r"def (test_\w+)\s*\(").unwrap();

        for cap in test_method_pattern.captures_iter(source) {
            if let Some(method_name) = cap.get(1) {
                let method_start = cap.get(0).unwrap().start();
                let method_name_str = method_name.as_str();

                // Find the method body
                let remaining = &source[method_start..];
                // Find the next method/class definition to determine body end
                // The 2 skips "de" in "def" so we start searching from after the def keyword
                let body_end = remaining[2..]
                    .find("\ndef ")
                    .or_else(|| remaining[2..].find("\nclass "))
                    .map(|pos| pos + 2)  // Add 2 to get absolute position in remaining
                    .unwrap_or(remaining.len());

                let method_body = &remaining[..body_end];

                // Check for temp file creation without cleanup
                let has_temp_creation = method_body.contains("mkstemp")
                    || method_body.contains("mkdtemp")
                    || method_body.contains("NamedTemporaryFile")
                    || method_body.contains("TemporaryFile")
                    || method_body.contains("tempfile");

                let has_monkeypatch = method_body.contains("monkeypatch")
                    || method_body.contains("mock.patch")
                    || method_body.contains("unittest.mock.patch");

                let has_cleanup = method_body.contains("finally:")
                    || method_body.contains("addCleanup")
                    || method_body.contains("yield")  // pytest fixture pattern
                    || method_body.contains("@fixture");

                if (has_temp_creation || has_monkeypatch) && !has_cleanup {
                    let line_num = source[..method_start].lines().count() + 1;
                    issues.push(Issue::new(
                        "PY_T14",
                        format!("Test '{}' creates resources without proper cleanup", method_name_str),
                        Severity::Minor,
                        Category::CodeSmell,
                        ctx.file_path,
                        line_num,
                    ).with_remediation(Remediation::quick(
                        "Use try/finally, addCleanup(), or pytest fixtures for cleanup."
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
    fn test_t14_registered() {
        let rule = PY_T14Rule::new();
        assert_eq!(rule.id(), "PY_T14");
    }

    #[test]
    fn test_t14_detects_temp_file_without_cleanup() {
        let rule = PY_T14Rule::new();
        let smelly = r#"
def test_something():
    fd, path = tempfile.mkstemp()
    # missing cleanup!
    os.close(fd)
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect temp file without cleanup");
        assert_eq!(issues[0].rule_id, "PY_T14");
    }

    #[test]
    fn test_t14_detects_monkeypatch_without_cleanup() {
        let rule = PY_T14Rule::new();
        let smelly = r#"
def test_something(monkeypatch):
    monkeypatch.setattr(obj, 'method', mock_fn)
    result = obj.method()
"#;
        let issues = with_python_context(smelly, "test.py", |ctx| rule.check(ctx));
        assert!(!issues.is_empty(), "Should detect monkeypatch without cleanup");
        assert_eq!(issues[0].rule_id, "PY_T14");
    }

    #[test]
    fn test_t14_allows_finally_cleanup() {
        let rule = PY_T14Rule::new();
        let clean = r#"
def test_something():
    fd, path = tempfile.mkstemp()
    try:
        # do something
    finally:
        os.close(fd)
        os.unlink(path)
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag try/finally cleanup");
    }

    #[test]
    fn test_t14_allows_yield_fixture() {
        let rule = PY_T14Rule::new();
        let clean = r#"
def test_something(tmp_path):
    # tmp_path is automatically cleaned up
    pass
"#;
        let issues = with_python_context(clean, "test.py", |ctx| rule.check(ctx));
        assert!(issues.is_empty(), "Should not flag pytest yield fixtures");
    }
}
